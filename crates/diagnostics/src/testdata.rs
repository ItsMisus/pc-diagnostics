//! Synthetic test fixtures. TEST DATA ONLY: compiled only under `cfg(test)`, never shipped.

use models::benchmark::{BenchmarkKind, BenchmarkState};
use models::benchmark_run::{
    BenchmarkContext, BenchmarkRequest, BenchmarkResult, PowerSource, ResultMetric,
};
use models::drivers::{
    DeviceCategory, DeviceHealth, DeviceRecord, DriverInfo, DriverInventory, DriverSummary,
    UpdateCheckInfo, UpdateSource, UpdateStatus,
};
use models::hardware::{
    BusKind, CpuInfo, DiskHealthStatus, GpuInfo, GpuVendor, HardwareInventory, MediaKind,
    MemoryInfo, PhysicalDiskInfo, VolumeInfo,
};
use models::monitor::{CpuSample, GpuSample, LiveSample, MemorySample, StorageSample};
use models::process::{
    IMPACT_METHODOLOGY_VERSION, ImpactWeights, Pressure, ProcessCategory, ProcessContext,
    ProcessRow, ProcessSnapshot,
};
use models::settings::Language;
use models::system::SystemInfo;
use models::{Metric, Source, Unavailability, Unit};

use crate::DiagnosticInput;

pub const T0: u64 = 1_793_755_260_000; // 2026-11-04 01:21:00 UTC
pub const GIB: f64 = 1024.0 * 1024.0 * 1024.0;
pub const GPU_ID: &str = "luid:00000000-0000ABCD";

pub fn avail(v: f64, unit: Unit, source: Source) -> Metric<f64> {
    Metric::Available {
        value: v,
        unit,
        source,
        at: T0,
    }
}

pub fn missing<T>() -> Metric<T> {
    Metric::unavailable(Unavailability::ProviderMissing("test".into()))
}

pub fn base_sample(i: usize) -> LiveSample {
    LiveSample {
        seq: i as u64 + 1,
        at: T0 + i as u64 * 1000,
        cpu: CpuSample {
            usage_percent: avail(10.0, Unit::Percent, Source::Pdh),
            per_core_percent: missing(),
            current_mhz: avail(4000.0, Unit::Megahertz, Source::Pdh),
            max_mhz: avail(4700.0, Unit::Megahertz, Source::PowerApi),
            limit_mhz: avail(4700.0, Unit::Megahertz, Source::PowerApi),
            temperature_c: missing(),
            power_w: missing(),
        },
        memory: MemorySample {
            total_bytes: avail(16.0 * GIB, Unit::Bytes, Source::Sysinfo),
            used_bytes: avail(8.0 * GIB, Unit::Bytes, Source::Sysinfo),
            available_bytes: avail(8.0 * GIB, Unit::Bytes, Source::Sysinfo),
            commit_used_bytes: avail(12.0 * GIB, Unit::Bytes, Source::Pdh),
            commit_limit_bytes: avail(32.0 * GIB, Unit::Bytes, Source::Pdh),
            pages_per_sec: avail(5.0, Unit::Count, Source::Pdh),
        },
        gpus: vec![GpuSample {
            id: GPU_ID.into(),
            usage_percent: avail(5.0, Unit::Percent, Source::Pdh),
            dedicated_used_bytes: avail(1.0 * GIB, Unit::Bytes, Source::Pdh),
            temperature_c: missing(),
            core_clock_mhz: missing(),
            power_w: missing(),
            power_percent: missing(),
            fan_rpm: missing(),
        }],
        storage: StorageSample {
            read_bytes_per_sec: avail(0.0, Unit::BytesPerSecond, Source::Pdh),
            write_bytes_per_sec: avail(0.0, Unit::BytesPerSecond, Source::Pdh),
            active_time_percent: avail(1.0, Unit::Percent, Source::Pdh),
        },
    }
}

/// `n` samples at 1 s cadence; `f(i, sample)` customizes each.
pub fn live(n: usize, f: impl Fn(usize, &mut LiveSample)) -> Vec<LiveSample> {
    (0..n)
        .map(|i| {
            let mut s = base_sample(i);
            f(i, &mut s);
            s
        })
        .collect()
}

pub fn system() -> SystemInfo {
    SystemInfo {
        os_name: Some("Windows".into()),
        os_version: Some("11 (26200)".into()),
        os_long_version: Some("Windows 11 Pro".into()),
        kernel_version: Some("26200".into()),
        hostname: Some("DESKTOP-SECRET1".into()),
        cpu_arch: "x86_64".into(),
        build_arch: "x86_64".into(),
        boot_time_unix_s: Some(1_793_700_000),
        uptime_s: Some(55_260),
    }
}

pub fn volume(mount: &str, total_gib: f64, free_gib: f64) -> VolumeInfo {
    VolumeInfo {
        mount_point: mount.into(),
        label: Some("Personal Stuff".into()),
        file_system: Some("NTFS".into()),
        total_bytes: (total_gib * GIB) as u64,
        available_bytes: (free_gib * GIB) as u64,
        removable: false,
        media: MediaKind::Ssd,
    }
}

pub fn disk(health: DiskHealthStatus) -> PhysicalDiskInfo {
    PhysicalDiskInfo {
        id: "0".into(),
        model: Some("Test NVMe 1TB".into()),
        firmware: Some("1.0".into()),
        media: MediaKind::Ssd,
        bus: BusKind::Nvme,
        size_bytes: (1000.0 * GIB) as u64,
        health: Metric::Available {
            value: health,
            unit: Unit::None,
            source: Source::Wmi,
            at: T0,
        },
    }
}

pub fn inventory() -> HardwareInventory {
    HardwareInventory {
        collected_at: T0,
        cpu: CpuInfo {
            vendor: Some("TestVendor".into()),
            brand: Some("Test CPU 8-Core".into()),
            arch: "x86_64".into(),
            physical_cores: Some(8),
            logical_cores: 16,
            max_mhz: avail(4700.0, Unit::Megahertz, Source::PowerApi),
        },
        gpus: vec![GpuInfo {
            id: GPU_ID.into(),
            name: "Test GPU 8GB".into(),
            vendor: GpuVendor::Nvidia,
            vendor_id: 0x10DE,
            device_id: 1,
            dedicated_vram_bytes: (8.0 * GIB) as u64,
            shared_memory_bytes: (8.0 * GIB) as u64,
            is_software: false,
            is_virtual: false,
            driver_version: Metric::Available {
                value: "32.0.15.1".into(),
                unit: Unit::None,
                source: Source::Dxgi,
                at: T0,
            },
            telemetry_sources: vec![Source::Pdh],
        }],
        memory: MemoryInfo {
            total_bytes: (16.0 * GIB) as u64,
            modules: missing(),
        },
        physical_disks: Metric::Available {
            value: vec![disk(DiskHealthStatus::Healthy)],
            unit: Unit::None,
            source: Source::Wmi,
            at: T0,
        },
        volumes: vec![volume("C:\\", 500.0, 200.0)],
    }
}

pub fn process(name: &str, cpu: f64, ws_gib: f64, impact: f64) -> ProcessRow {
    ProcessRow {
        key: format!("{}:1", name.len()),
        pid: 1000 + name.len() as u32,
        name: name.into(),
        category: ProcessCategory::Application,
        accessible: true,
        is_self: false,
        exe_path: Some(format!("C:\\Users\\mario.rossi\\AppData\\Local\\{name}")),
        cpu_percent: avail(cpu, Unit::Percent, Source::Pdh),
        working_set_bytes: avail(ws_gib * GIB, Unit::Bytes, Source::Pdh),
        private_bytes: avail(ws_gib * GIB, Unit::Bytes, Source::Pdh),
        io_read_bytes_per_sec: avail(0.0, Unit::BytesPerSecond, Source::Pdh),
        io_write_bytes_per_sec: avail(0.0, Unit::BytesPerSecond, Source::Pdh),
        pressure: Pressure {
            cpu,
            ram: 0.0,
            io: 0.0,
            impact,
        },
        impact_peak: impact,
    }
}

pub fn snapshot(rows: Vec<ProcessRow>) -> ProcessSnapshot {
    ProcessSnapshot {
        at: T0,
        interval_ms: 2000,
        window_s: 60,
        methodology_version: IMPACT_METHODOLOGY_VERSION,
        weights: ImpactWeights::DEFAULT,
        context: ProcessContext {
            logical_processors: 16,
            total_memory_bytes: 16.0 * GIB,
            available_memory_bytes: 8.0 * GIB,
            io_reference_bytes_per_sec: 100.0 * 1024.0 * 1024.0,
        },
        total_processes: 200,
        inaccessible_processes: 80,
        ranking_instant: rows.clone(),
        ranking_window: rows,
    }
}

pub fn device(name: &str, category: DeviceCategory, health: DeviceHealth) -> DeviceRecord {
    let code = match health {
        DeviceHealth::ProblemDetected => Some(43),
        DeviceHealth::DriverMissing => Some(28),
        DeviceHealth::Disabled => Some(22),
        DeviceHealth::Working => None,
    };
    DeviceRecord {
        instance_id: format!("USB\\VID_1234&PID_5678\\SERIALXYZ{}", name.len()),
        name: name.into(),
        manufacturer: Some("Maker".into()),
        class_name: None,
        class_guid: None,
        category,
        hardware_ids: vec!["USB\\VID_1234&PID_5678".into()],
        driver: Some(DriverInfo {
            version: Some("1.0".into()),
            date: Some("2024-01-01".into()),
            provider: Some("Maker".into()),
            inf_name: Some("oem1.inf".into()),
            signed: missing(),
            signer: None,
            is_inbox: false,
        }),
        health,
        problem_code: code,
        problem_description: code.map(|c| format!("Code {c}: test description.")),
        update: UpdateStatus::NotChecked,
    }
}

pub fn drivers(devices: Vec<DeviceRecord>, checked: bool) -> DriverInventory {
    DriverInventory {
        collected_at: T0,
        summary: DriverSummary::from_devices(&devices),
        devices,
        update_check: checked.then_some(UpdateCheckInfo {
            checked_at: T0,
            source: UpdateSource::WindowsUpdate,
            duration_ms: 1000,
            offered: 0,
            error: None,
        }),
    }
}

pub fn update_available() -> UpdateStatus {
    UpdateStatus::UpdateAvailable {
        source: UpdateSource::WindowsUpdate,
        title: "Driver update".into(),
        version: Some("2.0".into()),
        date: None,
        hardware_id: "USB\\VID_1234&PID_5678".into(),
    }
}

pub fn metric(id: &str, value: f64, unit: Unit, higher_is_better: bool) -> ResultMetric {
    ResultMetric {
        id: id.into(),
        label: id.into(),
        value,
        unit,
        higher_is_better,
    }
}

pub fn bench(run_id: &str, kind: BenchmarkKind, started_at: u64) -> BenchmarkResult {
    BenchmarkResult {
        run_id: run_id.into(),
        kind,
        state: BenchmarkState::Completed,
        methodology_version: 1,
        app_version: "0.1.0".into(),
        build_profile: "release".into(),
        started_at,
        finished_at: started_at + 30_000,
        duration_ms: 30_000,
        request: BenchmarkRequest {
            kind,
            duration_s: Some(30),
            threads: None,
            storage_volume: None,
            storage_file_mib: None,
            storage_write_consent: false,
            gpu_id: None,
        },
        context: BenchmarkContext {
            os_version: Some("11".into()),
            cpu_brand: Some("Test CPU 8-Core".into()),
            logical_processors: 16,
            total_memory_bytes: (16.0 * GIB) as u64,
            power_source: PowerSource::Ac,
            power_plan: Some("Bilanciato".into()),
            background_cpu_percent: avail(2.0, Unit::Percent, Source::Pdh),
            available_memory_bytes: avail(8.0 * GIB, Unit::Bytes, Source::Sysinfo),
            cpu_limit_mhz: avail(4700.0, Unit::Megahertz, Source::PowerApi),
            cpu_max_mhz: avail(4700.0, Unit::Megahertz, Source::PowerApi),
            storage_model: None,
            gpu_name: None,
            gpu_driver_version: None,
        },
        metrics: vec![metric(
            "throughput",
            1000.0,
            Unit::OperationsPerSecond,
            true,
        )],
        series: vec![],
        warnings: vec![],
        notes: Vec::new(),
        safety_stop: None,
        error: None,
    }
}

pub fn input<'a>(live: &'a [LiveSample]) -> DiagnosticInput<'a> {
    DiagnosticInput {
        app_version: "0.1.0",
        now_ms: T0 + 600_000,
        system: None,
        inventory: None,
        live,
        processes: None,
        drivers: None,
        benchmarks: &[],
        language: Language::Italian,
    }
}
