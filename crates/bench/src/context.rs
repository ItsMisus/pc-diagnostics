//! Run context (conditions at start) and the warnings derived from it.

use std::time::Duration;

use models::benchmark::BenchmarkKind;
use models::benchmark_run::{BenchmarkContext, PowerSource};
use models::metric::{Metric, Source, Unavailability, Unit};
use models::monitor::LiveSample;
use models::settings::Language;
use models::time::now_unix_ms;

/// Pre-start window over which background CPU load is averaged.
pub(crate) const PREP_WINDOW: Duration = Duration::from_secs(3);
/// Samples older than this at start are not used for the background average.
const SAMPLE_MAX_AGE_MS: u64 = 3_500;
pub(crate) const BACKGROUND_CPU_WARN_PERCENT: f64 = 10.0;

pub(crate) fn warn_temperature(lang: Language) -> &'static str {
    lang.pick(
        "Temperatura CPU non osservabile: applicati limiti di durata conservativi",
        "CPU temperature not observable: conservative duration limits applied",
    )
}

pub(crate) fn warn_debug(lang: Language) -> &'static str {
    lang.pick(
        "Build di debug: risultati non confrontabili con build di release",
        "Debug build: results not comparable with release builds",
    )
}

fn warn_battery(lang: Language) -> &'static str {
    lang.pick(
        "Alimentazione a batteria: le prestazioni possono essere ridotte e i risultati non \
         sono confrontabili con quelli da rete elettrica.",
        "Running on battery: performance may be reduced and results are not comparable \
         with runs on AC power.",
    )
}

fn warn_no_monitor(lang: Language) -> &'static str {
    lang.pick(
        "Monitoraggio live non attivo: carico di fondo e frequenza CPU durante il test non \
         disponibili.",
        "Live monitoring not running: background load and CPU frequency during the test \
         are unavailable.",
    )
}

const NO_SAMPLER: &str = "live sampler not running";

/// Slow-ish facts (WMI for the disk model), collected off the supervisor thread.
#[derive(Debug, Clone, Default)]
pub(crate) struct StaticFacts {
    pub os_version: Option<String>,
    pub cpu_brand: Option<String>,
    pub total_memory_bytes: u64,
    pub power_source: Option<PowerSource>,
    pub power_plan: Option<String>,
    pub storage_model: Option<String>,
    pub gpu_name: Option<String>,
    pub gpu_driver_version: Option<String>,
}

/// `gpu_id`: the `GpuInfo::id` of the adapter the GPU benchmark runs on (resolved by the
/// validator); `None` for other kinds.
pub(crate) fn static_facts(storage_volume: Option<&str>, gpu_id: Option<&str>) -> StaticFacts {
    let tested_gpu = gpu_id.and_then(|id| {
        platform::gpu_bench::gpu_adapters()
            .ok()
            .and_then(|all| all.into_iter().find(|g| g.id.eq_ignore_ascii_case(id)))
    });
    let sys = platform::system::system_info();
    StaticFacts {
        os_version: sys.os_long_version.or(sys.os_version),
        cpu_brand: platform::bench_env::cpu_brand(),
        total_memory_bytes: platform::bench_env::total_memory_bytes(),
        power_source: Some(platform::bench_env::power_source()),
        power_plan: platform::bench_env::active_power_plan(),
        storage_model: storage_volume.and_then(platform::bench_env::volume_disk_model),
        gpu_driver_version: tested_gpu
            .as_ref()
            .and_then(|g| g.driver_version.value().cloned()),
        gpu_name: tested_gpu.map(|g| g.name),
    }
}

/// Keeps distinct, recent samples (by `seq`) for the background-load average.
#[derive(Debug, Default)]
pub(crate) struct SampleWindow {
    samples: Vec<LiveSample>,
    pub sampler_seen: bool,
}

impl SampleWindow {
    pub fn offer(&mut self, s: Option<LiveSample>) {
        let Some(s) = s else { return };
        self.sampler_seen = true;
        if now_unix_ms().saturating_sub(s.at) > SAMPLE_MAX_AGE_MS {
            return;
        }
        if self.samples.iter().all(|x| x.seq != s.seq) {
            self.samples.push(s);
        }
    }

    pub fn latest(&self) -> Option<&LiveSample> {
        self.samples.iter().max_by_key(|s| s.seq)
    }

    pub fn background_cpu(&self) -> Metric<f64> {
        background_cpu(&self.samples, self.sampler_seen)
    }
}

/// Mean whole-machine CPU use of the samples that carry it.
pub(crate) fn background_cpu(samples: &[LiveSample], sampler_seen: bool) -> Metric<f64> {
    let values: Vec<f64> = samples
        .iter()
        .filter_map(|s| s.cpu.usage_percent.value().copied())
        .collect();
    match crate::stats::mean(&values) {
        Some(v) => Metric::available(v, Unit::Percent, Source::Pdh),
        None if sampler_seen => Metric::unavailable(Unavailability::NotSupported),
        None => Metric::unavailable(Unavailability::ProviderMissing(NO_SAMPLER.into())),
    }
}

/// Assembles the context and the start-condition warnings (in `lang`).
pub(crate) fn build(
    kind: BenchmarkKind,
    facts: StaticFacts,
    window: &SampleWindow,
    logical_processors: u32,
    build_profile: &str,
    lang: Language,
) -> (BenchmarkContext, Vec<String>) {
    let available_memory_bytes = match window.latest().map(|s| s.memory.available_bytes.clone()) {
        Some(m @ Metric::Available { .. }) => m,
        _ => match platform::process::memory_status() {
            // Read directly by the engine (GlobalMemoryStatusEx).
            Ok(m) => Metric::available(m.available_bytes as f64, Unit::Bytes, Source::Benchmark),
            Err(e) => Metric::unavailable(Unavailability::Failed(e)),
        },
    };
    let (cpu_max_mhz, cpu_limit_mhz) = platform::bench_env::cpu_frequency_limits();
    let context = BenchmarkContext {
        os_version: facts.os_version,
        cpu_brand: facts.cpu_brand,
        logical_processors,
        total_memory_bytes: facts.total_memory_bytes,
        power_source: facts.power_source.unwrap_or(PowerSource::Unknown),
        power_plan: facts.power_plan,
        background_cpu_percent: window.background_cpu(),
        available_memory_bytes,
        cpu_limit_mhz,
        cpu_max_mhz,
        storage_model: facts.storage_model,
        gpu_name: facts.gpu_name,
        gpu_driver_version: facts.gpu_driver_version,
    };
    let warnings = start_warnings(kind, &context, window.sampler_seen, build_profile, lang);
    (context, warnings)
}

/// Pure: warnings implied by the start conditions.
pub(crate) fn start_warnings(
    kind: BenchmarkKind,
    c: &BenchmarkContext,
    sampler_seen: bool,
    build_profile: &str,
    lang: Language,
) -> Vec<String> {
    let mut w = Vec::new();
    if build_profile == "debug" {
        w.push(warn_debug(lang).to_owned());
    }
    if c.power_source == PowerSource::Battery {
        w.push(warn_battery(lang).to_owned());
    }
    if let Some(bg) = c.background_cpu_percent.value()
        && *bg > BACKGROUND_CPU_WARN_PERCENT
    {
        w.push(lang.pick(
            format!(
                "Carico di fondo elevato prima dell'avvio (CPU {bg:.0}%): i risultati possono \
                 essere inferiori al normale."
            ),
            format!(
                "High background load before the start (CPU {bg:.0}%): results may be lower \
                 than usual."
            ),
        ));
    }
    if let (Some(limit), Some(max)) = (c.cpu_limit_mhz.value(), c.cpu_max_mhz.value())
        && *limit < *max - 1.0
    {
        w.push(lang.pick(
            format!(
                "Limite di frequenza del sistema operativo inferiore al massimo all'avvio \
                 ({limit:.0} MHz su {max:.0} MHz): possibile limitazione da piano energetico o \
                 termica, causa non determinabile."
            ),
            format!(
                "Operating system frequency limit below the maximum at the start \
                 ({limit:.0} MHz of {max:.0} MHz): possible power-plan or thermal limiting, \
                 cause undetermined."
            ),
        ));
    }
    if kind.component() == models::component::ComponentKind::Cpu {
        w.push(warn_temperature(lang).to_owned());
    }
    if !sampler_seen {
        w.push(warn_no_monitor(lang).to_owned());
    }
    w
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> BenchmarkContext {
        BenchmarkContext {
            os_version: None,
            cpu_brand: None,
            logical_processors: 8,
            total_memory_bytes: 1,
            power_source: PowerSource::Ac,
            power_plan: None,
            background_cpu_percent: Metric::available(2.0, Unit::Percent, Source::Pdh),
            available_memory_bytes: Metric::unavailable(Unavailability::NotSupported),
            cpu_limit_mhz: Metric::available(4700.0, Unit::Megahertz, Source::PowerApi),
            cpu_max_mhz: Metric::available(4700.0, Unit::Megahertz, Source::PowerApi),
            storage_model: None,
            gpu_name: None,
            gpu_driver_version: None,
        }
    }

    #[test]
    fn clean_start_has_no_condition_warnings() {
        let w = start_warnings(
            BenchmarkKind::MemoryLatency,
            &ctx(),
            true,
            "release",
            Language::Italian,
        );
        assert!(w.is_empty(), "{w:?}");
    }

    #[test]
    fn each_condition_warns() {
        let mut c = ctx();
        c.power_source = PowerSource::Battery;
        c.background_cpu_percent = Metric::available(25.0, Unit::Percent, Source::Pdh);
        c.cpu_limit_mhz = Metric::available(3000.0, Unit::Megahertz, Source::PowerApi);
        let w = start_warnings(
            BenchmarkKind::CpuSustained,
            &c,
            false,
            "debug",
            Language::Italian,
        );
        assert_eq!(w.len(), 6, "{w:?}");
        assert!(w.contains(&warn_debug(Language::Italian).to_owned()));
        assert!(w.contains(&warn_temperature(Language::Italian).to_owned()));
        assert!(w.iter().any(|s| s.contains("batteria")));
        assert!(w.iter().any(|s| s.contains("25%")));
        assert!(w.iter().any(|s| s.contains("3000 MHz su 4700 MHz")));

        let en = start_warnings(
            BenchmarkKind::CpuSustained,
            &c,
            false,
            "debug",
            Language::English,
        );
        assert_eq!(
            en,
            vec![
                "Debug build: results not comparable with release builds",
                "Running on battery: performance may be reduced and results are not \
                 comparable with runs on AC power.",
                "High background load before the start (CPU 25%): results may be lower than \
                 usual.",
                "Operating system frequency limit below the maximum at the start (3000 MHz of \
                 4700 MHz): possible power-plan or thermal limiting, cause undetermined.",
                "CPU temperature not observable: conservative duration limits applied",
                "Live monitoring not running: background load and CPU frequency during the \
                 test are unavailable.",
            ]
        );
    }

    #[test]
    fn background_average_requires_samples() {
        let m = background_cpu(&[], false);
        assert!(matches!(
            m,
            Metric::Unavailable {
                reason: Unavailability::ProviderMissing(_)
            }
        ));
    }
}
