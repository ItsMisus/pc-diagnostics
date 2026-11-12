//! End-to-end checks with macOS-shaped data: no driver inventory, no commit metrics, no
//! CPU frequency limit, an Apple GPU without dedicated memory, mount-point volumes and
//! POSIX paths. The diagnosis must complete, report coverage honestly and never tell a
//! Mac user about Windows tools.

use models::app::Platform;
use models::component::ComponentKind;
use models::diagnostics::{CoverageStatus, DiagnosticReport};
use models::hardware::{DiskHealthStatus, GpuVendor};
use models::settings::Language;
use models::{Metric, Source, Unavailability, Unit};

use crate::testdata::*;
use crate::{ReportBundle, evaluate, render_html, render_json};

fn not_supported<T>() -> Metric<T> {
    Metric::unavailable(Unavailability::NotSupported)
}

/// Strings that may legitimately name Windows on a Mac: "only on Windows".
fn strip_allowed(s: &str) -> String {
    s.replace("solo su Windows", "")
        .replace("only on Windows", "")
}

fn texts(r: &DiagnosticReport) -> String {
    serde_json::to_string(r).expect("json")
}

struct Mac {
    samples: Vec<models::monitor::LiveSample>,
    system: models::system::SystemInfo,
    inventory: models::hardware::HardwareInventory,
    processes: models::process::ProcessSnapshot,
}

fn mac() -> Mac {
    let samples = live(120, |_, s| {
        s.cpu.usage_percent = avail(99.0, Unit::Percent, Source::Sysinfo);
        s.cpu.current_mhz = not_supported();
        s.cpu.max_mhz = not_supported();
        s.cpu.limit_mhz = not_supported();
        s.memory.available_bytes = avail(0.5 * GIB, Unit::Bytes, Source::Sysinfo);
        s.memory.commit_used_bytes = not_supported();
        s.memory.commit_limit_bytes = not_supported();
        s.memory.pages_per_sec = avail(5000.0, Unit::Count, Source::Mach);
        s.memory.swap_used_bytes = avail(3.0 * GIB, Unit::Bytes, Source::Mach);
        s.memory.swap_total_bytes = avail(4.0 * GIB, Unit::Bytes, Source::Mach);
        s.memory.pressure = pressure(models::monitor::MemoryPressure::Critical);
        s.gpus[0].usage_percent = avail(99.0, Unit::Percent, Source::IoKit);
        s.gpus[0].dedicated_used_bytes = not_supported();
    });
    let mut system = system();
    system.os_name = Some("macOS".into());
    system.os_version = Some("27.2.1".into());
    system.os_long_version = Some("macOS 27.2.1".into());
    system.hostname = Some("MBP-SECRET1.local".into());
    let mut inventory = inventory();
    inventory.gpus[0].name = "Apple M5".into();
    inventory.gpus[0].vendor = GpuVendor::Apple;
    inventory.gpus[0].dedicated_vram_bytes = not_supported();
    inventory.gpus[0].device_id = not_supported();
    inventory.gpus[0].shared_memory_bytes = avail(12.0 * GIB, Unit::Bytes, Source::Metal);
    inventory.gpus[0].driver_version = not_supported();
    inventory.volumes = vec![
        volume("/", 460.0, 4.0),
        volume("/Volumes/Archivio", 1000.0, 80.0),
    ];
    inventory.physical_disks = Metric::Available {
        value: vec![models::hardware::PhysicalDiskInfo {
            health: Metric::Available {
                value: DiskHealthStatus::Warning,
                unit: Unit::None,
                source: Source::IoKit,
                at: T0,
            },
            ..disk(DiskHealthStatus::Warning)
        }],
        unit: Unit::None,
        source: Source::IoKit,
        at: T0,
    };
    let mut big = process("Safari", 90.0, 6.0, 80.0);
    big.exe_path = Some("/Applications/Safari.app/Contents/MacOS/Safari".into());
    let mut tool = process("Editor", 2.0, 0.5, 60.0);
    tool.exe_path = Some("/Users/mario.rossi/Applications/Editor.app/Contents/MacOS/Editor".into());
    Mac {
        samples,
        system,
        inventory,
        processes: snapshot(vec![big, tool]),
    }
}

fn run(m: &Mac, language: Language) -> DiagnosticReport {
    let mut inp = input(&m.samples);
    inp.platform = Platform::Macos;
    inp.language = language;
    inp.system = Some(&m.system);
    inp.inventory = Some(&m.inventory);
    inp.processes = Some(&m.processes);
    inp.drivers = None; // driver inventory is NotSupported on macOS
    evaluate(&inp)
}

#[test]
fn macos_diagnosis_completes_with_honest_coverage() {
    let m = mac();
    for lang in [Language::Italian, Language::English] {
        let r = run(&m, lang);
        assert_eq!(r.coverage.len(), 7);
        let cov = |c: ComponentKind| {
            r.coverage
                .iter()
                .find(|x| x.component == c)
                .expect("coverage")
        };
        let drivers = cov(ComponentKind::Drivers);
        assert_eq!(drivers.status, CoverageStatus::NotEvaluated);
        assert!(
            drivers
                .note
                .contains(lang.pick("solo su Windows", "only on Windows")),
            "{}",
            drivers.note
        );
        // Commit does not exist on macOS: noted, not counted as missing data.
        let memory = cov(ComponentKind::Memory);
        assert!(
            memory.note.contains(lang.pick(
                "esiste solo su Windows, regola non applicabile",
                "exists only on Windows, rule not applicable"
            )),
            "{}",
            memory.note
        );
        assert!(
            memory.note.starts_with(lang.pick(
                "memoria disponibile, pressione della memoria e paginazione",
                "available memory, memory pressure and paging"
            )),
            "{}",
            memory.note
        );
        let gpu = cov(ComponentKind::Gpu);
        assert!(
            gpu.note
                .contains(lang.pick("memoria unificata", "unified memory")),
            "{}",
            gpu.note
        );
        let ids: Vec<&str> = r.findings.iter().map(|f| f.rule_id.as_str()).collect();
        for id in [
            "cpu.utilization.sustained",
            "cpu.process.dominant",
            "memory.available.low",
            "memory.pressure.high",
            "memory.paging.high",
            "gpu.utilization.sustained",
            "storage.volume.low_space",
            "storage.disk.health",
            "processes.impact.high",
        ] {
            assert!(ids.contains(&id), "{id} missing from {ids:?}");
        }
        assert!(!ids.iter().any(|i| i.starts_with("drivers.")), "{ids:?}");
        assert!(!ids.contains(&"memory.commit.high"));
        assert!(
            !ids.iter().any(|i| i.starts_with("cpu.frequency.")),
            "{ids:?}"
        );
        let text = strip_allowed(&texts(&r));
        for windows_only in [
            "Windows",
            "Pulizia disco",
            "Disk Cleanup",
            "Sensore memoria",
            "Storage Sense",
            "Gestione attività",
            "Task Manager",
            "Recycle Bin",
            "MSFT_PhysicalDisk",
            "Pages/sec",
            "file di paging",
            "paging file",
        ] {
            assert!(!text.contains(windows_only), "{windows_only} in {text}");
        }
    }
}

#[test]
fn macos_texts_use_macos_names() {
    let m = mac();
    let it = run(&m, Language::Italian);
    let en = run(&m, Language::English);
    let find = |r: &DiagnosticReport, id: &str| {
        r.findings
            .iter()
            .filter(|f| f.rule_id == id)
            .cloned()
            .collect::<Vec<_>>()
    };
    let low_it = find(&it, "storage.volume.low_space");
    let heads: Vec<&str> = low_it.iter().map(|f| f.simple.headline.as_str()).collect();
    assert!(
        heads.contains(&"Il disco di avvio è quasi pieno"),
        "{heads:?}"
    );
    assert!(
        heads.contains(&"Sul disco /Volumes/Archivio resta poco spazio libero"),
        "{heads:?}"
    );
    assert!(
        low_it[0]
            .simple
            .action
            .contains("Impostazioni di Sistema > Generali > Spazio")
    );
    // Sizes in plain text follow Finder (powers of 1000): 4 GiB of 460 GiB.
    let boot = low_it
        .iter()
        .find(|f| f.simple.headline == "Il disco di avvio è quasi pieno")
        .expect("boot volume");
    assert!(
        boot.simple.meaning.contains("4,3 GB su 493,9 GB"),
        "{}",
        boot.simple.meaning
    );
    // The technical description keeps the exact IEC units.
    assert!(boot.description.contains("4,0 GiB"), "{}", boot.description);
    let low_en = find(&en, "storage.volume.low_space");
    assert!(low_en[0].simple.action.contains("empty the Trash"));
    assert!(
        low_en[0]
            .simple
            .meaning
            .contains("macOS may fail to install updates")
    );
    let disk = &find(&en, "storage.disk.health")[0];
    assert!(
        disk.title
            .starts_with("macOS reports a disk health problem")
    );
    assert_eq!(disk.evidence[0].label, "Health status");
    assert_eq!(disk.evidence[0].source, Source::IoKit);
    let paging = &find(&en, "memory.paging.high")[0];
    assert!(paging.simple.meaning.starts_with("macOS often read data"));
    assert!(paging.limitations[0].contains("macOS pageins"));
    let procs = &find(&it, "processes.impact.high")[0];
    assert!(
        procs
            .simple
            .action
            .ends_with("Non chiudere i programmi di sistema di macOS.")
    );
    let sys = it
        .coverage
        .iter()
        .find(|c| c.component == ComponentKind::System)
        .expect("system");
    assert!(
        sys.note
            .starts_with("sistema operativo rilevato (macOS 27.2.1)")
    );
}

#[test]
fn macos_report_export_is_clean_and_redacted() {
    let m = mac();
    for lang in [Language::Italian, Language::English] {
        let report = run(&m, lang);
        let bundle = ReportBundle {
            report: &report,
            system: Some(&m.system),
            inventory: Some(&m.inventory),
            benchmarks: &[],
            drivers: None,
            processes: Some(&m.processes),
            utc_offset_minutes: Some(60),
            language: lang,
            platform: Platform::Macos,
        };
        let html = render_html(&bundle, true).expect("html");
        let json = render_json(&bundle, true).expect("json");
        for out in [&html, &json] {
            let lower = out.to_ascii_lowercase();
            assert!(!lower.contains("mario"), "user name leaked");
            assert!(!lower.contains("mbp-secret1"), "hostname leaked");
            assert!(!out.contains("Personal Stuff"), "volume label leaked");
        }
        let html_clean = strip_allowed(&html);
        assert!(!html_clean.contains("Windows"), "{html_clean}");
        assert!(html.contains(lang.pick("Stato riportato da macOS", "Status reported by macOS")));
        // Unredacted export keeps the real POSIX paths out of the document anyway:
        // process paths are never exported.
        let plain = render_json(&bundle, false).expect("json");
        assert!(!plain.contains("/Users/mario.rossi/Applications"));
    }
}
