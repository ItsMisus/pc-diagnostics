//! The wgpu workload. Runs only inside the worker process.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use models::settings::Language;
use models::{AppError, ErrorCode};

use super::kernel::{self, WORKGROUP_SIZE};
use super::protocol::{AdapterReport, Msg, Point, WorkerResult, phase};
use super::select::{self, Target};

/// Wall-clock length a batch (one submit + wait) aims for. Short enough for fast
/// cancellation and far below the Windows TDR limit (2 s per GPU packet).
const TARGET_BATCH: Duration = Duration::from_millis(40);
/// A single dispatch never grows beyond this (TDR safety on slow GPUs).
const MAX_DISPATCH_TIME: Duration = Duration::from_millis(20);
/// Largest dispatch: 16 384 workgroups × 256 = 4 Mi invocations (16 MiB output).
const MAX_WORKGROUPS: u32 = 16_384;
const MIN_WORKGROUPS: u32 = 16;
const MAX_DISPATCHES_PER_BATCH: u32 = 256;
/// A wait longer than this means the GPU stopped responding.
const POLL_TIMEOUT: Duration = Duration::from_secs(10);
const PROGRESS_EVERY: Duration = Duration::from_millis(250);

pub(crate) enum WorkErr {
    Cancelled,
    Failed(AppError),
}

fn failed(msg: &str, detail: impl Into<String>) -> WorkErr {
    WorkErr::Failed(AppError::new(ErrorCode::Platform, msg).with_detail(detail))
}

/// Windows (and other non-Apple targets): DX12, Vulkan as fallback. macOS: Metal only
/// (the only native GPU API there).
fn backends() -> wgpu::Backends {
    if cfg!(target_os = "macos") {
        wgpu::Backends::METAL
    } else {
        wgpu::Backends::DX12 | wgpu::Backends::VULKAN
    }
}

fn instance() -> wgpu::Instance {
    wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: backends(),
        // No validation/debug layers (they are on by default in debug builds).
        flags: wgpu::InstanceFlags::empty(),
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    })
}

fn backend_name(b: wgpu::Backend) -> &'static str {
    match b {
        wgpu::Backend::Dx12 => "dx12",
        wgpu::Backend::Vulkan => "vulkan",
        wgpu::Backend::Metal => "metal",
        _ => "other",
    }
}

fn device_type_name(t: wgpu::DeviceType) -> &'static str {
    match t {
        wgpu::DeviceType::DiscreteGpu => "discrete",
        wgpu::DeviceType::IntegratedGpu => "integrated",
        wgpu::DeviceType::VirtualGpu => "virtual",
        wgpu::DeviceType::Cpu => "cpu",
        wgpu::DeviceType::Other => "other",
    }
}

/// The adapter's id in `GpuInfo::id` format: the DXGI LUID of a DX12 adapter
/// (`luid:<high>-<low>`), or the I/O Kit registry id of a Metal device (`ioreg:<16 hex>`,
/// `MTLDevice.registryID`, which is the `IOAccelerator` entry the inventory lists). `None`
/// for other backends (Vulkan).
fn luid_of(adapter: &wgpu::Adapter) -> Option<String> {
    #[cfg(windows)]
    {
        if adapter.get_info().backend != wgpu::Backend::Dx12 {
            return None;
        }
        // SAFETY: only reads the adapter description through the guard, which is dropped
        // at the end of this block; nothing is destroyed or mutated.
        let hal = unsafe { adapter.as_hal::<wgpu::hal::api::Dx12>() }?;
        // SAFETY: `raw_adapter()` is a live IDXGIAdapter3 owned by the guard;
        // `GetDesc2` only fills a plain struct.
        let desc = unsafe { hal.raw_adapter().GetDesc2() }.ok()?;
        Some(format!(
            "luid:{:08X}-{:08X}",
            desc.AdapterLuid.HighPart as u32, desc.AdapterLuid.LowPart
        ))
    }
    #[cfg(target_os = "macos")]
    {
        use objc2_metal::MTLDevice;
        if adapter.get_info().backend != wgpu::Backend::Metal {
            return None;
        }
        // SAFETY: only reads the device's registry id through the guard, which is dropped
        // at the end of this block; nothing is destroyed or mutated.
        let hal = unsafe { adapter.as_hal::<wgpu::hal::api::Metal>() }?;
        let id = hal.raw_device().registryID();
        (id != 0).then(|| platform::gpu_bench::registry_gpu_id(id))
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let _ = adapter;
        None
    }
}

fn report(adapter: &wgpu::Adapter) -> AdapterReport {
    let info = adapter.get_info();
    AdapterReport {
        name: info.name.clone(),
        vendor_id: info.vendor,
        device_id: info.device,
        backend: backend_name(info.backend).to_owned(),
        device_type: device_type_name(info.device_type).to_owned(),
        luid: luid_of(adapter),
        driver: info.driver.clone(),
        driver_info: info.driver_info.clone(),
    }
}

fn enumerate() -> (wgpu::Instance, Vec<wgpu::Adapter>) {
    let instance = instance();
    let adapters = pollster::block_on(instance.enumerate_adapters(backends()));
    (instance, adapters)
}

/// Every adapter wgpu sees (the parent filters).
pub(crate) fn probe() -> Vec<AdapterReport> {
    let (_instance, adapters) = enumerate();
    adapters.iter().map(report).collect()
}

/// Errors raised by wgpu outside an error scope, and device loss.
#[derive(Default)]
struct DeviceFault(Mutex<Option<String>>);

impl DeviceFault {
    fn set(&self, s: String) {
        let mut g = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        if g.is_none() {
            *g = Some(s);
        }
    }
    fn get(&self) -> Option<String> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::ComputePipeline,
    bind_group: wgpu::BindGroup,
    out: wgpu::Buffer,
    fault: Arc<DeviceFault>,
}

impl Gpu {
    fn new(adapter: &wgpu::Adapter) -> Result<Self, WorkErr> {
        let limits = adapter.limits();
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("pcdiag-gpu-bench"),
            required_limits: limits.clone(),
            ..Default::default()
        }))
        .map_err(|e| failed("GPU device creation failed", e.to_string()))?;
        let fault = Arc::new(DeviceFault::default());
        let f = Arc::clone(&fault);
        device.on_uncaptured_error(Arc::new(move |e| f.set(format!("wgpu error: {e}"))));
        let f = Arc::clone(&fault);
        device.set_device_lost_callback(move |reason, msg| {
            f.set(format!("device lost ({reason:?}): {msg}"));
        });

        let out_bytes = u64::from(MAX_WORKGROUPS) * u64::from(WORKGROUP_SIZE) * 4;
        if out_bytes > limits.max_storage_buffer_binding_size
            || out_bytes > limits.max_buffer_size
            || limits.max_compute_workgroups_per_dimension < MAX_WORKGROUPS
            || limits.max_compute_invocations_per_workgroup < WORKGROUP_SIZE
        {
            return Err(WorkErr::Failed(
                AppError::new(
                    ErrorCode::NotSupported,
                    "GPU limits too low for the benchmark",
                )
                .with_detail(format!("{limits:?}")),
            ));
        }

        let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("pcdiag-fma"),
            source: wgpu::ShaderSource::Wgsl(kernel::wgsl().into()),
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("pcdiag-fma"),
            layout: None,
            module: &module,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
        let out = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("pcdiag-out"),
            size: out_bytes,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("pcdiag-bg"),
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: out.as_entire_binding(),
            }],
        });
        if let Some(e) = pollster::block_on(scope.pop()) {
            return Err(failed("GPU pipeline creation failed", e.to_string()));
        }
        Ok(Self {
            device,
            queue,
            pipeline,
            bind_group,
            out,
            fault,
        })
    }

    fn check_fault(&self) -> Result<(), WorkErr> {
        match self.fault.get() {
            Some(f) => Err(failed("GPU error during the benchmark", f)),
            None => Ok(()),
        }
    }

    /// One submit of `dispatches` dispatches, waited for. Returns the wall time.
    fn batch(&self, workgroups: u32, dispatches: u32) -> Result<Duration, WorkErr> {
        let t0 = Instant::now();
        let mut enc = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        {
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor::default());
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            for _ in 0..dispatches {
                pass.dispatch_workgroups(workgroups, 1, 1);
            }
        }
        let idx = self.queue.submit([enc.finish()]);
        self.device
            .poll(wgpu::PollType::Wait {
                submission_index: Some(idx),
                timeout: Some(POLL_TIMEOUT),
            })
            .map_err(|e| failed("GPU did not complete the work", e.to_string()))?;
        self.check_fault()?;
        Ok(t0.elapsed())
    }

    /// Reads `out[i]` for each index.
    fn read_back(&self, indices: &[u32]) -> Result<Vec<f32>, WorkErr> {
        let size = (indices.len() as u64 * 4).max(4);
        let staging = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("pcdiag-readback"),
            size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut enc = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        for (j, &i) in indices.iter().enumerate() {
            enc.copy_buffer_to_buffer(&self.out, u64::from(i) * 4, &staging, j as u64 * 4, 4);
        }
        let idx = self.queue.submit([enc.finish()]);
        let mapped: Arc<Mutex<Option<Result<(), String>>>> = Arc::default();
        let m2 = Arc::clone(&mapped);
        staging.map_async(wgpu::MapMode::Read, .., move |r| {
            *m2.lock().unwrap_or_else(PoisonError::into_inner) = Some(r.map_err(|e| e.to_string()));
        });
        self.device
            .poll(wgpu::PollType::Wait {
                submission_index: Some(idx),
                timeout: Some(POLL_TIMEOUT),
            })
            .map_err(|e| failed("GPU read-back did not complete", e.to_string()))?;
        // `poll(Wait)` also runs the map callback.
        let state = mapped
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        match state {
            Some(Ok(())) => {}
            Some(Err(e)) => return Err(failed("GPU read-back mapping failed", e)),
            None => {
                return Err(failed(
                    "GPU read-back mapping failed",
                    "callback not called",
                ));
            }
        }
        let values = {
            let view = staging
                .get_mapped_range(..)
                .map_err(|e| failed("GPU read-back mapping failed", e.to_string()))?;
            view.as_chunks::<4>()
                .0
                .iter()
                .take(indices.len())
                .map(|b| f32::from_le_bytes(*b))
                .collect()
        };
        staging.unmap();
        self.check_fault()?;
        Ok(values)
    }
}

pub(crate) struct RunParams {
    pub target: Target,
    pub exclude_luids: Vec<String>,
    pub warmup: Duration,
    pub duration: Duration,
}

fn check(cancel: &AtomicBool) -> Result<(), WorkErr> {
    if cancel.load(Ordering::Relaxed) {
        Err(WorkErr::Cancelled)
    } else {
        Ok(())
    }
}

pub(crate) fn run(
    p: &RunParams,
    cancel: &AtomicBool,
    emit: &mut dyn FnMut(&Msg),
) -> Result<WorkerResult, WorkErr> {
    let progress = |emit: &mut dyn FnMut(&Msg), phase: &str, fraction: f64, f: Option<f64>| {
        emit(&Msg::Progress {
            phase: phase.to_owned(),
            fraction,
            flops_per_s: f,
        });
    };
    progress(emit, phase::SELECT, 0.0, None);
    let (_instance, adapters) = enumerate();
    let reports: Vec<AdapterReport> = adapters.iter().map(report).collect();
    let (index, matched_by) =
        select::select(&reports, &p.target, &p.exclude_luids).map_err(|reason| {
            WorkErr::Failed(
                AppError::new(ErrorCode::NotSupported, "no suitable GPU adapter")
                    .with_detail(reason.text(Language::English)),
            )
        })?;
    emit(&Msg::Selected {
        adapter: reports[index].clone(),
        matched_by,
    });
    check(cancel)?;
    progress(emit, phase::COMPILE, 0.0, None);
    let gpu = Gpu::new(&adapters[index])?;
    check(cancel)?;

    let total = p.warmup + p.duration;
    let frac = |t: Duration| (t.as_secs_f64() / total.as_secs_f64().max(1e-9)).clamp(0.0, 1.0);

    // Warm-up and calibration: grow the dispatch, then the dispatches per batch, until a
    // batch takes ~TARGET_BATCH. Frozen when the timed window starts.
    let t0 = Instant::now();
    let mut wg = MIN_WORKGROUPS;
    let mut k = 1u32;
    let mut last_progress = Instant::now();
    progress(emit, phase::WARMUP, 0.0, None);
    while t0.elapsed() < p.warmup {
        check(cancel)?;
        let d = gpu.batch(wg, k)?;
        let per_dispatch = d / k;
        if d < TARGET_BATCH / 2 {
            if wg < MAX_WORKGROUPS && per_dispatch * 2 < MAX_DISPATCH_TIME {
                wg *= 2;
            } else if k < MAX_DISPATCHES_PER_BATCH {
                k *= 2;
            }
        } else if per_dispatch > MAX_DISPATCH_TIME && wg > MIN_WORKGROUPS {
            wg /= 2;
        } else if d > TARGET_BATCH * 2 && k > 1 {
            k /= 2;
        }
        if last_progress.elapsed() >= PROGRESS_EVERY {
            last_progress = Instant::now();
            let rate = kernel::flops_for(wg, u64::from(k)) / d.as_secs_f64().max(1e-9);
            progress(emit, phase::WARMUP, frac(t0.elapsed()), Some(rate));
        }
    }
    let warmup_ms = u64::try_from(t0.elapsed().as_millis()).unwrap_or(u64::MAX);

    // Timed window.
    emit(&Msg::Measuring);
    let batch_flops = kernel::flops_for(wg, u64::from(k));
    let start = Instant::now();
    let mut total_flops = 0.0;
    let mut batches = 0u64;
    let mut points = Vec::new();
    let mut last_point = Duration::ZERO;
    let mut since_point = 0.0;
    let mut last_rate = None;
    while start.elapsed() < p.duration {
        check(cancel)?;
        gpu.batch(wg, k)?;
        batches += 1;
        total_flops += batch_flops;
        since_point += batch_flops;
        let now = start.elapsed();
        let dt = now - last_point;
        if dt >= Duration::from_secs(1) {
            let rate = since_point / dt.as_secs_f64();
            points.push(Point {
                elapsed_ms: u64::try_from(now.as_millis()).unwrap_or(u64::MAX),
                flops_per_s: rate,
            });
            last_rate = Some(rate);
            last_point = now;
            since_point = 0.0;
        }
        if last_progress.elapsed() >= PROGRESS_EVERY {
            last_progress = Instant::now();
            progress(emit, phase::MEASURE, frac(p.warmup + now), last_rate);
        }
    }
    let timed = start.elapsed();
    let tail = timed - last_point;
    if tail >= Duration::from_millis(500) && since_point > 0.0 {
        points.push(Point {
            elapsed_ms: u64::try_from(timed.as_millis()).unwrap_or(u64::MAX),
            flops_per_s: since_point / tail.as_secs_f64(),
        });
    }

    progress(emit, phase::VERIFY, 1.0, last_rate);
    let invocations = wg * WORKGROUP_SIZE;
    let indices = kernel::check_indices(invocations);
    let values = gpu.read_back(&indices)?;
    let max_rel_error = kernel::verify(&indices, &values).map_err(|m| {
        WorkErr::Failed(
            AppError::new(
                ErrorCode::Internal,
                "GPU result verification failed: the output does not match the CPU reference",
            )
            .with_detail(format!(
                "out[{}] = {} on the GPU, {} expected",
                m.index, m.gpu, m.cpu
            )),
        )
    })?;

    Ok(WorkerResult {
        flops_per_invocation: kernel::flops_per_invocation(),
        invocations_per_dispatch: invocations,
        dispatches_per_batch: k,
        batches,
        total_flops,
        timed_ms: u64::try_from(timed.as_millis()).unwrap_or(u64::MAX),
        warmup_ms,
        points,
        verified_elements: u32::try_from(indices.len()).unwrap_or(u32::MAX),
        max_rel_error,
    })
}
