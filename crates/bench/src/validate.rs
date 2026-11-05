//! Request bounds, validation/resolution and capability descriptions.
//!
//! Every bound here is documented in docs/benchmark-methodology.md.

use std::time::Duration;

use models::benchmark::BenchmarkKind;
use models::benchmark_run::{BenchmarkCapability, BenchmarkGpuTarget, BenchmarkRequest};
use models::metric::{Metric, Source, Unavailability, Unit};
use models::settings::Language;
use models::{AppError, ErrorCode};

use crate::gpu::{self, GpuEnv, GpuPlan, Unusable, WorkerCommand};
use crate::runtime::Hooks;

pub(crate) const MIB: u64 = 1024 * 1024;
pub(crate) const GIB: u64 = 1024 * MIB;

/// Methodology version per kind. Bump when the workload, its parameters or the metric
/// definitions change: results with different versions are never compared.
pub(crate) fn methodology_version(kind: BenchmarkKind) -> u32 {
    match kind {
        BenchmarkKind::CpuSingleCore
        | BenchmarkKind::CpuMultiCore
        | BenchmarkKind::CpuSustained
        | BenchmarkKind::MemoryBandwidth
        | BenchmarkKind::MemoryLatency
        | BenchmarkKind::StorageSequential
        | BenchmarkKind::StorageRandom
        | BenchmarkKind::GpuCompute => 1,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DurationBounds {
    pub default_s: u32,
    pub min_s: u32,
    pub max_s: u32,
}

pub(crate) fn duration_bounds(kind: BenchmarkKind) -> Option<DurationBounds> {
    match kind {
        BenchmarkKind::CpuSingleCore | BenchmarkKind::CpuMultiCore => Some(DurationBounds {
            default_s: 15,
            min_s: 5,
            max_s: 60,
        }),
        BenchmarkKind::CpuSustained => Some(DurationBounds {
            default_s: 60,
            min_s: 30,
            max_s: 300,
        }),
        // Timed window; the cap drops to 30 s when the selected GPU has no usable driver
        // threshold or no readable temperature (see `gpu::Thermal::max_duration_s`).
        BenchmarkKind::GpuCompute => Some(DurationBounds {
            default_s: gpu::DEFAULT_S,
            min_s: gpu::MIN_S,
            max_s: gpu::MAX_S_OBSERVED,
        }),
        _ => None,
    }
}

pub(crate) fn supports_threads(kind: BenchmarkKind) -> bool {
    matches!(
        kind,
        BenchmarkKind::CpuMultiCore | BenchmarkKind::CpuSustained
    )
}

pub(crate) fn is_storage(kind: BenchmarkKind) -> bool {
    matches!(
        kind,
        BenchmarkKind::StorageSequential | BenchmarkKind::StorageRandom
    )
}

pub(crate) const FILE_MIB_DEFAULT: u32 = 1024;
pub(crate) const FILE_MIB_MIN: u32 = 256;
pub(crate) const FILE_MIB_MAX: u32 = 4096;
/// Upper bound of the random 4 KiB write phase (bytes), on top of the file preparation.
pub(crate) const RANDOM_WRITE_CAP_BYTES: u64 = 256 * MIB;

/// Bytes one run may write at most, given the file size. Enforced by the storage engine.
pub(crate) fn write_budget(kind: BenchmarkKind, file_mib: u32) -> u64 {
    let file = u64::from(file_mib) * MIB;
    match kind {
        BenchmarkKind::StorageSequential => file,
        BenchmarkKind::StorageRandom => file + RANDOM_WRITE_CAP_BYTES,
        _ => 0,
    }
}

/// Watchdog: the controller stops any run that is still going after this long.
pub(crate) fn hard_limit(plan: &Plan) -> Duration {
    match plan.kind {
        BenchmarkKind::CpuSingleCore | BenchmarkKind::CpuMultiCore => {
            plan.duration.unwrap_or_default() + Duration::from_secs(20)
        }
        BenchmarkKind::CpuSustained => plan.duration.unwrap_or_default() + Duration::from_secs(20),
        BenchmarkKind::MemoryBandwidth | BenchmarkKind::MemoryLatency => Duration::from_secs(120),
        // Generous for slow HDDs: 4 GiB written and read at ~10 MB/s worst case.
        BenchmarkKind::StorageSequential | BenchmarkKind::StorageRandom => {
            Duration::from_secs(1_200)
        }
        // Warm-up + process start, adapter selection and shader compilation (seconds).
        BenchmarkKind::GpuCompute => {
            plan.duration.unwrap_or_default()
                + plan.gpu.as_ref().map_or(gpu::WARMUP, |g| g.warmup)
                + Duration::from_secs(40)
        }
    }
}

/// Facts the validator needs about the machine.
#[derive(Debug, Clone)]
pub(crate) struct Env {
    pub logical_processors: u32,
    /// `X:\` mount points of present fixed volumes.
    pub fixed_volumes: Vec<String>,
    /// GPU probe result; only filled for `GpuCompute` requests.
    pub gpu: Option<GpuEnv>,
}

/// A validated request with defaults applied.
#[derive(Debug, Clone)]
pub(crate) struct Plan {
    pub kind: BenchmarkKind,
    pub request: BenchmarkRequest,
    pub duration: Option<Duration>,
    pub threads: u32,
    pub volume: Option<String>,
    pub file_mib: u32,
    pub gpu: Option<GpuPlan>,
}

fn invalid(msg: impl Into<String>) -> AppError {
    AppError::invalid_input(msg)
}

pub(crate) fn resolve(req: &BenchmarkRequest, env: &Env, hooks: &Hooks) -> Result<Plan, AppError> {
    let kind = req.kind;
    let gpu_plan = if kind == BenchmarkKind::GpuCompute {
        Some(gpu_plan(env, hooks, req.gpu_id.as_deref())?)
    } else {
        if let Some(id) = &req.gpu_id {
            return Err(invalid(format!(
                "{kind:?} does not use a GPU: gpuId is only allowed for GpuCompute"
            ))
            .with_detail(format!("received {id}")));
        }
        None
    };
    let logical = env.logical_processors.max(1);

    // Duration.
    let duration_s = match (duration_bounds(kind), req.duration_s) {
        (Some(b), None) => Some(b.default_s),
        (Some(b), Some(d)) => {
            let min = hooks.min_duration_s.unwrap_or(b.min_s).min(b.min_s);
            let max = match &gpu_plan {
                Some(g) => b.max_s.min(g.ready.thermal.max_duration_s()),
                None => b.max_s,
            };
            if d < min || d > max {
                let why = if max < b.max_s {
                    " (no usable GPU temperature threshold for the selected GPU: conservative cap)"
                } else {
                    ""
                };
                return Err(invalid(format!(
                    "durationS must be between {} and {max} for {kind:?}{why}",
                    b.min_s
                ))
                .with_detail(format!("received {d}")));
            }
            Some(d)
        }
        (None, Some(d)) => {
            return Err(invalid(format!("{kind:?} does not take durationS"))
                .with_detail(format!("received {d}")));
        }
        (None, None) => None,
    };

    // Threads.
    let threads = if supports_threads(kind) {
        match req.threads {
            None => logical,
            Some(t) if (1..=logical).contains(&t) => t,
            Some(t) => {
                return Err(invalid(format!("threads must be between 1 and {logical}"))
                    .with_detail(format!("received {t}")));
            }
        }
    } else {
        match req.threads {
            None => match kind {
                BenchmarkKind::MemoryBandwidth => logical,
                _ => 1,
            },
            Some(1) if kind == BenchmarkKind::CpuSingleCore => 1,
            Some(t) => {
                return Err(invalid(format!("{kind:?} does not take threads"))
                    .with_detail(format!("received {t}")));
            }
        }
    };

    // Storage.
    let (volume, file_mib) = if is_storage(kind) {
        if !req.storage_write_consent {
            return Err(invalid(
                "storage benchmarks write a temporary file: storageWriteConsent must be true",
            ));
        }
        let raw = req
            .storage_volume
            .as_deref()
            .ok_or_else(|| invalid("storageVolume is required for storage benchmarks"))?;
        let vol = platform::bench_env::normalize_drive_mount(raw).ok_or_else(|| {
            invalid("storageVolume must be a drive root such as C:\\").with_detail(raw.to_owned())
        })?;
        if !env
            .fixed_volumes
            .iter()
            .any(|v| v.eq_ignore_ascii_case(&vol))
        {
            return Err(invalid("storageVolume is not a present fixed volume")
                .with_detail(format!("{vol} not in {:?}", env.fixed_volumes)));
        }
        let min = hooks.min_file_mib.unwrap_or(FILE_MIB_MIN).min(FILE_MIB_MIN);
        let mib = match req.storage_file_mib {
            None => FILE_MIB_DEFAULT,
            Some(m) if (min..=FILE_MIB_MAX).contains(&m) => m,
            Some(m) => {
                return Err(invalid(format!(
                    "storageFileMib must be between {FILE_MIB_MIN} and {FILE_MIB_MAX}"
                ))
                .with_detail(format!("received {m}")));
            }
        };
        (Some(vol), mib)
    } else {
        if req.storage_volume.is_some() || req.storage_file_mib.is_some() {
            return Err(invalid(format!(
                "{kind:?} does not take storageVolume/storageFileMib"
            )));
        }
        (None, 0)
    };

    let request = BenchmarkRequest {
        kind,
        duration_s,
        threads: (supports_threads(kind) || kind == BenchmarkKind::MemoryBandwidth)
            .then_some(threads),
        storage_volume: volume.clone(),
        storage_file_mib: is_storage(kind).then_some(file_mib),
        storage_write_consent: is_storage(kind) && req.storage_write_consent,
        gpu_id: gpu_plan.as_ref().map(|g| g.ready.target.id.clone()),
    };
    Ok(Plan {
        kind,
        request,
        duration: duration_s.map(|s| Duration::from_secs(u64::from(s))),
        threads,
        volume,
        file_mib,
        gpu: gpu_plan,
    })
}

/// Resolves the target GPU: `gpu_id` must name an available physical adapter of the
/// probe; `None` = the default (primary) target.
fn gpu_plan(env: &Env, hooks: &Hooks, gpu_id: Option<&str>) -> Result<GpuPlan, AppError> {
    let gpu = env
        .gpu
        .as_ref()
        .ok_or_else(|| AppError::internal("GPU environment not probed"))?;
    // Technical detail: English.
    let why = |u: &Unusable| {
        let t = u.render(Language::English);
        format!("{} ({:?})", t.note, t.reason)
    };
    let unavailable = |u: &Unusable| {
        AppError::new(ErrorCode::NotSupported, "GPU compute benchmark unavailable")
            .with_detail(why(u))
    };
    if let GpuEnv::Unavailable(u) = gpu {
        return Err(unavailable(u));
    }
    let ready = match gpu_id {
        Some(id) => {
            let t = gpu.find(id).ok_or_else(|| {
                invalid(
                    "the requested GPU is not among the physical GPUs available for the benchmark",
                )
                .with_detail(format!(
                    "gpuId {id}; available: {}",
                    gpu.targets()
                        .iter()
                        .map(|t| format!("{} ({})", t.target.id, t.target.name))
                        .collect::<Vec<_>>()
                        .join(", ")
                ))
            })?;
            match &t.status {
                Ok(r) => (**r).clone(),
                Err(u) => {
                    return Err(invalid(format!(
                        "GPU {} cannot be used for the benchmark",
                        t.target.name
                    ))
                    .with_detail(why(u)));
                }
            }
        }
        None => match gpu.default_target().map(|t| &t.status) {
            Some(Ok(r)) => (**r).clone(),
            Some(Err(u)) => return Err(unavailable(u)),
            None => {
                return Err(AppError::internal(
                    "GPU environment without a default target",
                ));
            }
        },
    };
    let command = match &hooks.gpu_worker {
        Some(c) => c.clone(),
        None => WorkerCommand::current_exe()?,
    };
    Ok(GpuPlan {
        ready,
        command,
        warmup: hooks.gpu_warmup.unwrap_or(gpu::WARMUP),
        simulate: hooks.gpu_simulate,
    })
}

/// Upper bound of the free-space reserve kept on the tested volume.
const RESERVE_CAP: u64 = 20 * GIB;

/// Free-space rule for the storage test file: after creating it the volume must keep at
/// least `clamp(10 % of its size, 2 GiB, 20 GiB)` free. The 20 GiB cap keeps large,
/// moderately full volumes testable (10 % of 2 TB would demand 200 GiB of slack).
pub(crate) fn check_free_space(free: u64, total: u64, file_bytes: u64) -> Result<(), AppError> {
    let reserve = (total / 10).clamp(2 * GIB, RESERVE_CAP);
    let required = file_bytes.saturating_add(reserve);
    if free >= required {
        Ok(())
    } else {
        Err(
            invalid("not enough free space on the volume for the test file").with_detail(format!(
                "free {} MiB, required {} MiB (file {} MiB + reserve {} MiB)",
                free / MIB,
                required / MIB,
                file_bytes / MIB,
                reserve / MIB
            )),
        )
    }
}

pub(crate) const ALL_KINDS: [BenchmarkKind; 8] = [
    BenchmarkKind::CpuSingleCore,
    BenchmarkKind::CpuMultiCore,
    BenchmarkKind::CpuSustained,
    BenchmarkKind::MemoryBandwidth,
    BenchmarkKind::MemoryLatency,
    BenchmarkKind::StorageSequential,
    BenchmarkKind::StorageRandom,
    BenchmarkKind::GpuCompute,
];

/// Capability description and the list of what is measured.
type Texts = (&'static str, &'static [&'static str]);

fn texts(kind: BenchmarkKind, lang: Language) -> Texts {
    match kind {
        BenchmarkKind::CpuSingleCore => lang.pick::<Texts>(
            (
                "Calcolo intero, hashing e virgola mobile su un solo thread per un tempo fisso. \
                 Misura le unità di lavoro completate al secondo.",
                &["Throughput (unità/s)", "Frequenza CPU (se disponibile)"],
            ),
            (
                "Integer, hashing and floating-point work on a single thread for a fixed \
                 time. Measures the work units completed per second.",
                &["Throughput (units/s)", "CPU frequency (if available)"],
            ),
        ),
        BenchmarkKind::CpuMultiCore => lang.pick::<Texts>(
            (
                "Lo stesso carico del test single-core eseguito su più thread in parallelo. \
                 Misura le unità di lavoro completate al secondo da tutti i thread.",
                &[
                    "Throughput (unità/s)",
                    "Throughput per thread",
                    "Frequenza CPU (se disponibile)",
                ],
            ),
            (
                "The same load as the single-core test, run on several threads in parallel. \
                 Measures the work units completed per second by all threads.",
                &[
                    "Throughput (units/s)",
                    "Throughput per thread",
                    "CPU frequency (if available)",
                ],
            ),
        ),
        BenchmarkKind::CpuSustained => lang.pick::<Texts>(
            (
                "Carico multi-thread prolungato. Confronta le prestazioni iniziali e finali \
                 e la loro stabilità. La temperatura CPU non è osservabile: durata limitata.",
                &[
                    "Throughput medio (unità/s)",
                    "Variazione tra inizio e fine (%)",
                    "Coefficiente di variazione (%)",
                    "Frequenza e limite di frequenza CPU",
                ],
            ),
            (
                "Prolonged multi-threaded load. Compares the starting and ending performance \
                 and their stability. CPU temperature is not observable: limited duration.",
                &[
                    "Average throughput (units/s)",
                    "Change between start and end (%)",
                    "Coefficient of variation (%)",
                    "CPU frequency and frequency limit",
                ],
            ),
        ),
        BenchmarkKind::MemoryBandwidth => lang.pick::<Texts>(
            (
                "Lettura, scrittura e copia di un buffer in RAM (fino a 1 GiB) con tutti i \
                 thread. Non verifica l'assenza di errori della RAM.",
                &[
                    "Banda in lettura (GB/s)",
                    "Banda in scrittura (GB/s)",
                    "Banda in copia (GB/s)",
                ],
            ),
            (
                "Reads, writes and copies a RAM buffer (up to 1 GiB) with all threads. It \
                 doesn't check the RAM for errors.",
                &[
                    "Read bandwidth (GB/s)",
                    "Write bandwidth (GB/s)",
                    "Copy bandwidth (GB/s)",
                ],
            ),
        ),
        BenchmarkKind::MemoryLatency => lang.pick::<Texts>(
            (
                "Accessi dipendenti in ordine casuale su un buffer da 256 MiB. Valore \
                 indicativo; non verifica l'assenza di errori della RAM.",
                &[
                    "Latenza RAM (ns per accesso)",
                    "Latenza di riferimento in cache (32 KiB)",
                ],
            ),
            (
                "Dependent accesses in random order over a 256 MiB buffer. Indicative value; \
                 it doesn't check the RAM for errors.",
                &[
                    "RAM latency (ns per access)",
                    "Cache reference latency (32 KiB)",
                ],
            ),
        ),
        BenchmarkKind::StorageSequential => lang.pick::<Texts>(
            (
                "Scrive e poi rilegge un file temporaneo a blocchi da 1 MiB, senza cache di \
                 sistema. Il file viene eliminato al termine. Richiede il consenso alla \
                 scrittura.",
                &[
                    "Scrittura sequenziale (MB/s)",
                    "Lettura sequenziale (MB/s)",
                    "Dati scritti",
                ],
            ),
            (
                "Writes and then reads back a temporary file in 1 MiB blocks, bypassing the \
                 system cache. The file is deleted at the end. Requires consent to write.",
                &[
                    "Sequential write (MB/s)",
                    "Sequential read (MB/s)",
                    "Data written",
                ],
            ),
        ),
        BenchmarkKind::StorageRandom => lang.pick::<Texts>(
            (
                "Letture e scritture casuali da 4 KiB su un file temporaneo, senza cache di \
                 sistema. Scritture casuali limitate a 256 MiB. Il file viene eliminato al \
                 termine.",
                &[
                    "Lettura casuale 4 KiB QD1 (IOPS, MB/s, latenza)",
                    "Lettura casuale 4 KiB con 4 thread (IOPS, MB/s)",
                    "Scrittura casuale 4 KiB QD1 (IOPS, MB/s, latenza)",
                    "Dati scritti",
                ],
            ),
            (
                "Random 4 KiB reads and writes on a temporary file, bypassing the system \
                 cache. Random writes are capped at 256 MiB. The file is deleted at the end.",
                &[
                    "4 KiB random read QD1 (IOPS, MB/s, latency)",
                    "4 KiB random read with 4 threads (IOPS, MB/s)",
                    "4 KiB random write QD1 (IOPS, MB/s, latency)",
                    "Data written",
                ],
            ),
        ),
        BenchmarkKind::GpuCompute => lang.pick::<Texts>(
            (
                "Calcolo in virgola mobile FP32 (catene di FMA) con uno shader compute wgpu \
                 (DX12, in alternativa Vulkan) sulla GPU selezionata (predefinita: la GPU \
                 principale), in un processo separato. Il risultato viene verificato con un \
                 riferimento calcolato dalla CPU. Il test si arresta alla soglia termica \
                 fornita dal driver; se la soglia o la temperatura non sono disponibili la \
                 durata massima è ridotta a 30 s.",
                &[
                    "Throughput FP32 (FLOP/s)",
                    "Variazione tra inizio e fine (%)",
                    "Coefficiente di variazione (%)",
                    "Temperatura, frequenza e potenza GPU (se disponibili)",
                ],
            ),
            (
                "FP32 floating-point compute (FMA chains) with a wgpu compute shader (DX12, \
                 or Vulkan as a fallback) on the selected GPU (default: the primary GPU), in a \
                 separate process. The result is verified against a reference computed by the \
                 CPU. The test stops at the thermal threshold provided by the driver; if the \
                 threshold or the temperature is unavailable, the maximum duration is reduced \
                 to 30 s.",
                &[
                    "FP32 throughput (FLOP/s)",
                    "Change between start and end (%)",
                    "Coefficient of variation (%)",
                    "GPU temperature, clock and power (if available)",
                ],
            ),
        ),
    }
}

/// `gpu` is the probe result, used for `GpuCompute` only. Texts in `lang`.
pub(crate) fn capability(
    kind: BenchmarkKind,
    logical: u32,
    gpu: Option<&GpuEnv>,
    lang: Language,
) -> BenchmarkCapability {
    let (description, measures) = texts(kind, lang);
    let mut description = description.to_owned();
    let mut d = duration_bounds(kind);
    let mut gpu_targets = Vec::new();
    let availability = if kind == BenchmarkKind::GpuCompute {
        match gpu {
            Some(env) => {
                gpu_targets = env.targets().iter().map(|t| gpu_target(t, lang)).collect();
                match env.availability() {
                    Ok(()) => {
                        // Kind-level cap: the longest any available target allows; each
                        // target's own cap is enforced by `resolve` (30 s without
                        // `stopTemperatureC`).
                        let cap = env
                            .targets()
                            .iter()
                            .filter_map(|t| t.status.as_ref().ok())
                            .map(|r| r.thermal.max_duration_s())
                            .max();
                        if let (Some(b), Some(cap)) = (d.as_mut(), cap) {
                            b.max_s = b.max_s.min(cap);
                        }
                        Metric::available(true, Unit::None, Source::Benchmark)
                    }
                    Err(u) => {
                        let u = u.render(lang);
                        description.push(' ');
                        description.push_str(&u.note);
                        Metric::unavailable(u.reason)
                    }
                }
            }
            None => Metric::unavailable(Unavailability::NotSupported),
        }
    } else if cfg!(windows) {
        Metric::available(true, Unit::None, Source::Benchmark)
    } else {
        Metric::unavailable(Unavailability::NotSupported)
    };
    let storage = is_storage(kind);
    BenchmarkCapability {
        kind,
        availability,
        description,
        measures: measures.iter().map(|s| (*s).to_owned()).collect(),
        methodology_version: methodology_version(kind),
        default_duration_s: d.map(|b| b.default_s),
        max_duration_s: d.map(|b| b.max_s),
        supports_threads: supports_threads(kind),
        max_threads: supports_threads(kind).then_some(logical.max(1)),
        default_file_mib: storage.then_some(FILE_MIB_DEFAULT),
        max_file_mib: storage.then_some(FILE_MIB_MAX),
        max_bytes_written: storage.then(|| write_budget(kind, FILE_MIB_MAX)),
        gpu_targets,
    }
}

fn gpu_target(t: &gpu::TargetEnv, lang: Language) -> BenchmarkGpuTarget {
    BenchmarkGpuTarget {
        id: t.target.id.clone(),
        name: t.target.name.clone(),
        is_default: t.is_default,
        availability: match &t.status {
            Ok(_) => Metric::available(true, Unit::None, Source::Benchmark),
            Err(u) => Metric::unavailable(u.render(lang).reason),
        },
        stop_temperature_c: t.thermal.stop_temperature_c(),
        max_duration_s: t.thermal.max_duration_s(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gpu_worker;

    const IT: Language = Language::Italian;
    const EN: Language = Language::English;

    fn env() -> Env {
        Env {
            logical_processors: 8,
            fixed_volumes: vec!["C:\\".into(), "D:\\".into()],
            gpu: None,
        }
    }

    fn gpu_env(env: GpuEnv) -> Env {
        Env {
            gpu: Some(env),
            ..self::env()
        }
    }

    fn req(kind: BenchmarkKind) -> BenchmarkRequest {
        BenchmarkRequest {
            kind,
            duration_s: None,
            threads: None,
            storage_volume: None,
            storage_file_mib: None,
            storage_write_consent: false,
            gpu_id: None,
        }
    }

    fn storage_req(kind: BenchmarkKind) -> BenchmarkRequest {
        BenchmarkRequest {
            storage_volume: Some("c:".into()),
            storage_write_consent: true,
            ..req(kind)
        }
    }

    fn code(r: Result<Plan, AppError>) -> ErrorCode {
        r.expect_err("must reject").code
    }

    #[test]
    fn defaults_are_applied() {
        let h = Hooks::default();
        let p = resolve(&req(BenchmarkKind::CpuSingleCore), &env(), &h).expect("ok");
        assert_eq!(p.request.duration_s, Some(15));
        assert_eq!(p.threads, 1);
        assert_eq!(p.request.threads, None);
        let p = resolve(&req(BenchmarkKind::CpuMultiCore), &env(), &h).expect("ok");
        assert_eq!(p.request.threads, Some(8));
        let p = resolve(&req(BenchmarkKind::CpuSustained), &env(), &h).expect("ok");
        assert_eq!(p.request.duration_s, Some(60));
        let p = resolve(&req(BenchmarkKind::MemoryBandwidth), &env(), &h).expect("ok");
        assert_eq!(p.request.duration_s, None);
        assert_eq!(p.threads, 8);
        let p = resolve(&storage_req(BenchmarkKind::StorageSequential), &env(), &h).expect("ok");
        assert_eq!(p.request.storage_volume.as_deref(), Some("C:\\"));
        assert_eq!(p.request.storage_file_mib, Some(1024));
    }

    #[test]
    fn duration_bounds_per_kind() {
        let h = Hooks::default();
        for (kind, lo, hi) in [
            (BenchmarkKind::CpuSingleCore, 5, 60),
            (BenchmarkKind::CpuMultiCore, 5, 60),
            (BenchmarkKind::CpuSustained, 30, 300),
        ] {
            for ok in [lo, hi] {
                let r = BenchmarkRequest {
                    duration_s: Some(ok),
                    ..req(kind)
                };
                assert!(resolve(&r, &env(), &h).is_ok(), "{kind:?} {ok}");
            }
            for bad in [0, lo - 1, hi + 1] {
                let r = BenchmarkRequest {
                    duration_s: Some(bad),
                    ..req(kind)
                };
                assert_eq!(code(resolve(&r, &env(), &h)), ErrorCode::InvalidInput);
            }
        }
        for kind in [
            BenchmarkKind::MemoryBandwidth,
            BenchmarkKind::MemoryLatency,
            BenchmarkKind::StorageRandom,
        ] {
            let r = BenchmarkRequest {
                duration_s: Some(10),
                ..storage_req(kind)
            };
            assert_eq!(code(resolve(&r, &env(), &h)), ErrorCode::InvalidInput);
        }
    }

    #[test]
    fn thread_bounds() {
        let h = Hooks::default();
        for kind in [BenchmarkKind::CpuMultiCore, BenchmarkKind::CpuSustained] {
            for ok in [1, 8] {
                let r = BenchmarkRequest {
                    threads: Some(ok),
                    ..req(kind)
                };
                assert_eq!(resolve(&r, &env(), &h).expect("ok").threads, ok);
            }
            for bad in [0, 9] {
                let r = BenchmarkRequest {
                    threads: Some(bad),
                    ..req(kind)
                };
                assert_eq!(code(resolve(&r, &env(), &h)), ErrorCode::InvalidInput);
            }
        }
        let r = BenchmarkRequest {
            threads: Some(2),
            ..req(BenchmarkKind::CpuSingleCore)
        };
        assert_eq!(code(resolve(&r, &env(), &h)), ErrorCode::InvalidInput);
        let r = BenchmarkRequest {
            threads: Some(2),
            ..req(BenchmarkKind::MemoryLatency)
        };
        assert_eq!(code(resolve(&r, &env(), &h)), ErrorCode::InvalidInput);
    }

    #[test]
    fn storage_requires_consent_and_present_volume() {
        let h = Hooks::default();
        for kind in [
            BenchmarkKind::StorageSequential,
            BenchmarkKind::StorageRandom,
        ] {
            let no_consent = BenchmarkRequest {
                storage_write_consent: false,
                ..storage_req(kind)
            };
            assert_eq!(
                code(resolve(&no_consent, &env(), &h)),
                ErrorCode::InvalidInput
            );
            let no_volume = BenchmarkRequest {
                storage_volume: None,
                ..storage_req(kind)
            };
            assert_eq!(
                code(resolve(&no_volume, &env(), &h)),
                ErrorCode::InvalidInput
            );
            for bad in [
                "Z:\\",
                "C:\\Windows",
                "\\\\server\\share",
                "\\\\.\\PhysicalDrive0",
            ] {
                let r = BenchmarkRequest {
                    storage_volume: Some(bad.into()),
                    ..storage_req(kind)
                };
                assert_eq!(
                    code(resolve(&r, &env(), &h)),
                    ErrorCode::InvalidInput,
                    "{bad}"
                );
            }
            for (mib, ok) in [(255, false), (256, true), (4096, true), (4097, false)] {
                let r = BenchmarkRequest {
                    storage_file_mib: Some(mib),
                    ..storage_req(kind)
                };
                assert_eq!(resolve(&r, &env(), &h).is_ok(), ok, "{mib}");
            }
        }
        // Test hook lowers the minimum file size, never the maximum.
        let h = Hooks {
            min_file_mib: Some(16),
            ..Hooks::default()
        };
        let r = BenchmarkRequest {
            storage_file_mib: Some(16),
            ..storage_req(BenchmarkKind::StorageSequential)
        };
        assert!(resolve(&r, &env(), &h).is_ok());
        // Non-storage kinds reject storage fields.
        let r = BenchmarkRequest {
            storage_volume: Some("C:\\".into()),
            ..req(BenchmarkKind::CpuMultiCore)
        };
        assert_eq!(
            code(resolve(&r, &env(), &Hooks::default())),
            ErrorCode::InvalidInput
        );
    }

    #[test]
    fn gpu_requests_follow_the_probe_and_thermal_caps() {
        let h = Hooks::default();
        let r = req(BenchmarkKind::GpuCompute);
        // Unavailable GPU → NotSupported with the reason; not probed → Internal.
        let unavailable = GpuEnv::Unavailable(Unusable::NoUsableGpu);
        let e = resolve(&r, &gpu_env(unavailable), &h).expect_err("unavailable");
        assert_eq!(e.code, ErrorCode::NotSupported);
        assert!(
            e.detail
                .as_deref()
                .is_some_and(|d| d.contains("No usable physical GPU"))
        );
        assert_eq!(code(resolve(&r, &env(), &h)), ErrorCode::Internal);

        // Unobservable temperature: default 20 s, cap 30 s.
        let env_u = gpu_env(gpu::test_env(false));
        let p = resolve(&r, &env_u, &h).expect("ok");
        assert_eq!(p.request.duration_s, Some(20));
        assert_eq!(hard_limit(&p), Duration::from_secs(20 + 2 + 40));
        assert!(p.gpu.is_some());
        for (d, ok) in [(4, false), (5, true), (30, true), (31, false)] {
            let r = BenchmarkRequest {
                duration_s: Some(d),
                ..req(BenchmarkKind::GpuCompute)
            };
            assert_eq!(resolve(&r, &env_u, &h).is_ok(), ok, "{d}");
        }
        // Observable temperature with a driver threshold: up to 60 s.
        let env_o = gpu_env(gpu::test_env(true));
        for (d, ok) in [(60, true), (61, false)] {
            let r = BenchmarkRequest {
                duration_s: Some(d),
                ..req(BenchmarkKind::GpuCompute)
            };
            assert_eq!(resolve(&r, &env_o, &h).is_ok(), ok, "{d}");
        }
        // GPU takes no threads or storage fields.
        let r = BenchmarkRequest {
            threads: Some(2),
            ..req(BenchmarkKind::GpuCompute)
        };
        assert_eq!(code(resolve(&r, &env_o, &h)), ErrorCode::InvalidInput);
    }

    fn gpu_req(id: Option<&str>, duration_s: Option<u32>) -> BenchmarkRequest {
        BenchmarkRequest {
            gpu_id: id.map(Into::into),
            duration_s,
            ..req(BenchmarkKind::GpuCompute)
        }
    }

    #[test]
    fn gpu_id_selects_the_target() {
        let h = Hooks::default();
        let env2 = gpu_env(gpu::test_env_two(true));
        // None → the default target, and the resolved request names it.
        let p = resolve(&gpu_req(None, None), &env2, &h).expect("default");
        assert_eq!(p.request.gpu_id.as_deref(), Some(gpu::TEST_GPU_ID));
        assert_eq!(
            p.gpu.as_ref().map(|g| g.ready.target.name.as_str()),
            Some("Test GPU")
        );
        // Explicit id (case-insensitive) → that target.
        let p = resolve(
            &gpu_req(Some(&gpu::TEST_IGPU_ID.to_uppercase()), None),
            &env2,
            &h,
        )
        .expect("igpu");
        assert_eq!(p.request.gpu_id.as_deref(), Some(gpu::TEST_IGPU_ID));
        let g = p.gpu.expect("plan");
        assert_eq!(g.ready.target.name, "Test iGPU");
        assert_eq!(g.ready.thermal.stop_temperature_c(), None);
        // Per-target duration cap: 60 s on the observable GPU, 30 s on the iGPU.
        assert!(resolve(&gpu_req(Some(gpu::TEST_GPU_ID), Some(60)), &env2, &h).is_ok());
        assert!(resolve(&gpu_req(Some(gpu::TEST_IGPU_ID), Some(30)), &env2, &h).is_ok());
        assert_eq!(
            code(resolve(
                &gpu_req(Some(gpu::TEST_IGPU_ID), Some(31)),
                &env2,
                &h
            )),
            ErrorCode::InvalidInput
        );
        // Unknown id → InvalidInput naming the available ones.
        let e = resolve(&gpu_req(Some("luid:00000000-DEADBEEF"), None), &env2, &h)
            .expect_err("unknown");
        assert_eq!(e.code, ErrorCode::InvalidInput);
        assert!(e.message.contains("the requested GPU"), "{}", e.message);
        assert!(
            e.detail.as_deref().is_some_and(|d| d.contains("Test iGPU")),
            "{:?}",
            e.detail
        );
        // Known but unavailable id → InvalidInput with the GPU name and the reason.
        let env_u = gpu_env(gpu::test_env_two(false));
        let e =
            resolve(&gpu_req(Some(gpu::TEST_IGPU_ID), None), &env_u, &h).expect_err("unavailable");
        assert_eq!(e.code, ErrorCode::InvalidInput);
        assert!(e.message.contains("Test iGPU"), "{}", e.message);
        assert!(
            e.detail
                .as_deref()
                .is_some_and(|d| d.contains("no physical DX12/Vulkan adapter")),
            "{:?}",
            e.detail
        );
        // The default still works.
        assert!(resolve(&gpu_req(None, None), &env_u, &h).is_ok());
        // gpuId on a non-GPU kind → InvalidInput.
        for kind in [BenchmarkKind::CpuSingleCore, BenchmarkKind::MemoryLatency] {
            let r = BenchmarkRequest {
                gpu_id: Some(gpu::TEST_GPU_ID.into()),
                ..req(kind)
            };
            assert_eq!(
                code(resolve(&r, &env2, &h)),
                ErrorCode::InvalidInput,
                "{kind:?}"
            );
        }
        // Non-GPU kinds resolve with no gpuId.
        let p = resolve(&req(BenchmarkKind::CpuSingleCore), &env(), &h).expect("cpu");
        assert_eq!(p.request.gpu_id, None);
    }

    #[test]
    fn free_space_rule() {
        let total = 100 * GIB;
        // reserve = 10 GiB (10 %) on a 100 GiB volume.
        assert!(check_free_space(11 * GIB, total, GIB).is_ok());
        assert!(check_free_space(11 * GIB - 1, total, GIB).is_err());
        // Large volume: the 20 GiB cap applies instead of 10 % (100 GiB).
        let total = 1000 * GIB;
        assert!(check_free_space(21 * GIB, total, GIB).is_ok());
        assert!(check_free_space(21 * GIB - 1, total, GIB).is_err());
        // Small volume: the 2 GiB floor applies.
        let total = 10 * GIB;
        assert!(check_free_space(3 * GIB, total, GIB).is_ok());
        assert!(check_free_space(3 * GIB - 1, total, GIB).is_err());
        assert_eq!(
            check_free_space(0, total, GIB).expect_err("full").code,
            ErrorCode::InvalidInput
        );
    }

    #[test]
    fn capabilities_are_honest() {
        let unavailable = GpuEnv::Unavailable(Unusable::NoComputeAdapter {
            gpu: "X".into(),
            why: gpu_worker::select::NoMatch::NoAdapter {
                vendor_id: 0x10DE,
                device_id: 0x2783,
                luid: None,
            },
        });
        for kind in ALL_KINDS {
            let c = capability(kind, 24, Some(&unavailable), IT);
            assert_eq!(c.kind, kind);
            assert!(!c.description.is_empty());
            if kind == BenchmarkKind::GpuCompute {
                assert_eq!(
                    c.availability,
                    Metric::unavailable(Unavailability::ProviderMissing(
                        "adattatore wgpu DX12/Vulkan per X".into()
                    ))
                );
                assert!(c.description.contains("nessun adattatore"));
                let en = capability(kind, 24, Some(&unavailable), EN);
                assert_eq!(
                    en.availability,
                    Metric::unavailable(Unavailability::ProviderMissing(
                        "wgpu DX12/Vulkan adapter for X".into()
                    ))
                );
                assert!(en.description.ends_with(
                    "the maximum duration is reduced to 30 s. GPU X: no physical DX12/Vulkan \
                     adapter matches the requested GPU (PCI 10DE:2783)."
                ));
            } else if cfg!(windows) {
                assert_eq!(c.availability.value(), Some(&true));
            }
            assert_eq!(c.max_bytes_written.is_some(), is_storage(kind));
        }
        let c = capability(BenchmarkKind::StorageRandom, 24, None, IT);
        assert_eq!(c.max_bytes_written, Some(4096 * MIB + 256 * MIB));
        assert_eq!(
            capability(BenchmarkKind::CpuMultiCore, 24, None, IT).max_threads,
            Some(24)
        );
        // Non-GPU kinds list no GPU targets.
        for kind in ALL_KINDS {
            if kind != BenchmarkKind::GpuCompute {
                let c = capability(kind, 24, Some(&gpu::test_env_two(true)), IT);
                assert!(c.gpu_targets.is_empty(), "{kind:?}");
            }
        }
        // Ready GPU: available, generic description (no GPU name, no temperature), cap
        // follows temperature observability.
        let c = capability(
            BenchmarkKind::GpuCompute,
            24,
            Some(&gpu::test_env(false)),
            IT,
        );
        assert_eq!(c.availability.value(), Some(&true));
        assert!(!c.description.contains("Test GPU"), "{}", c.description);
        assert!(!c.description.contains("°C"), "{}", c.description);
        assert_eq!(
            (c.default_duration_s, c.max_duration_s),
            (Some(20), Some(30))
        );
        assert_eq!(c.gpu_targets.len(), 1);
        assert_eq!(c.gpu_targets[0].stop_temperature_c, None);
        let c = capability(
            BenchmarkKind::GpuCompute,
            24,
            Some(&gpu::test_env(true)),
            IT,
        );
        assert_eq!(c.max_duration_s, Some(60));
        assert_eq!(c.gpu_targets[0].stop_temperature_c, Some(89.0));
        // Two targets: default first, per-target availability and stop temperature.
        let c = capability(
            BenchmarkKind::GpuCompute,
            24,
            Some(&gpu::test_env_two(false)),
            IT,
        );
        assert_eq!(c.availability.value(), Some(&true));
        let t = &c.gpu_targets;
        assert_eq!(t.len(), 2);
        assert_eq!(
            (t[0].id.as_str(), t[0].name.as_str(), t[0].is_default),
            (gpu::TEST_GPU_ID, "Test GPU", true)
        );
        assert_eq!(t[0].availability.value(), Some(&true));
        assert_eq!(
            (t[1].id.as_str(), t[1].name.as_str(), t[1].is_default),
            (gpu::TEST_IGPU_ID, "Test iGPU", false)
        );
        assert!(!t[1].availability.is_available());
        assert_eq!(t[1].stop_temperature_c, None);
        // Never probed: not offered.
        let c = capability(BenchmarkKind::GpuCompute, 24, None, IT);
        assert!(!c.availability.is_available());
        assert!(c.gpu_targets.is_empty());
    }

    #[test]
    fn capability_texts_in_english() {
        let c = capability(BenchmarkKind::MemoryLatency, 24, None, EN);
        assert_eq!(
            c.description,
            "Dependent accesses in random order over a 256 MiB buffer. Indicative value; it \
             doesn't check the RAM for errors."
        );
        assert_eq!(
            c.measures,
            [
                "RAM latency (ns per access)",
                "Cache reference latency (32 KiB)"
            ]
        );
        let it = capability(BenchmarkKind::MemoryLatency, 24, None, IT);
        assert_eq!(it.measures[0], "Latenza RAM (ns per accesso)");
        for kind in ALL_KINDS {
            let (it, en) = (texts(kind, IT), texts(kind, EN));
            assert_eq!(it.1.len(), en.1.len(), "{kind:?}");
            assert_ne!(it.0, en.0, "{kind:?}");
        }
    }
}
