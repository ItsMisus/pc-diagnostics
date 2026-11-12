//! Tests for the plain-language text (`Finding.simple`) of every rule, in Italian and
//! English. Style rules: docs/diagnostics-rules.md, "Plain-language text".

use models::benchmark::BenchmarkKind;
use models::benchmark_run::{BenchmarkResult, PowerSource};
use models::diagnostics::{Confidence, DiagnosticReport, Finding, FindingKind, Severity};
use models::drivers::{DeviceCategory as C, DeviceHealth as H, DeviceRecord, DriverInventory};
use models::hardware::{DiskHealthStatus, HardwareInventory};
use models::monitor::LiveSample;
use models::process::ProcessSnapshot;
use models::settings::Language;
use models::{Metric, Source, Unit};

use crate::evaluate;
use crate::rules::cpu::SUSTAINED_DROP_METRIC_ID;
use crate::testdata::*;

/// Technical tokens that must never reach the "Semplice" view (checked case-insensitively).
pub const BANNED_JARGON: [&str; 26] = [
    "pdh",
    "cm_prob",
    "commit",
    "nvml",
    "wddm",
    "luid",
    "tdr",
    "throttl",
    "vram",
    "smart",
    "paging",
    "pages/sec",
    "working set",
    "impact score",
    "throughput",
    "configuration manager",
    "msft_",
    "healthstatus",
    "code ",
    "codice ",
    "benchmark",
    "mhz",
    "gib",
    "mib",
    "kib",
    "dxgi",
];

/// Advice the product never gives (buying, disabling, registry, firmware, tuning).
pub const BANNED_ADVICE: [&str; 11] = [
    "acquist",
    "compra",
    "sostitu",
    "disattiva",
    "disabilita",
    "registro di sistema",
    "regedit",
    "bios",
    "uefi",
    "overclock",
    "undervolt",
];

/// At least one must appear in the headline or meaning of a hypothesis or of any
/// low-confidence finding.
pub const HEDGES: [&str; 6] = [
    "potrebb",
    "probabil",
    "forse",
    "possibil",
    "non è detto",
    "non è certo",
];

/// An observation states a fact: its plain text must not claim a cause.
pub const CAUSAL_WORDS: [&str; 5] = ["perché", "a causa d", "è causato", "è dovuto", "colpa d"];

/// English counterparts of `BANNED_ADVICE`, `HEDGES` and `CAUSAL_WORDS`. `BANNED_JARGON`
/// is shared: its tokens are technical terms in both languages.
pub const BANNED_ADVICE_EN: [&str; 10] = [
    "buy",
    "purchase",
    "replace",
    "disable",
    "registry",
    "regedit",
    "bios",
    "uefi",
    "overclock",
    "undervolt",
];
pub const HEDGES_EN: [&str; 9] = [
    "might",
    "may ",
    "could",
    "probabl",
    "possibl",
    "perhaps",
    "not necessarily",
    "not certain",
    "no way to know",
];
pub const CAUSAL_WORDS_EN: [&str; 5] = [
    "because",
    "due to",
    "is caused",
    "caused by",
    "the fault of",
];

/// Italian words that must never appear in an English report (checked case-insensitively
/// on the whole serialized report, so device names of the fixtures avoid them).
const ITALIAN_MARKERS: [&str; 12] = [
    " il ",
    " della ",
    " dei ",
    " non ",
    " è ",
    " perché",
    " sono ",
    " nessun",
    " per ",
    " con ",
    "campioni",
    "disponibil",
];

const COMPONENTS: [&str; 7] = [
    "cpu",
    "gpu",
    "memory",
    "storage",
    "drivers",
    "processes",
    "system",
];

fn is_rule_id(s: &str) -> bool {
    let parts: Vec<&str> = s.split('.').collect();
    parts.len() == 3
        && COMPONENTS.contains(&parts[0])
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_lowercase() || c == '_'))
}

/// Every rule id literal in the rule sources (outside their test modules). A new rule is
/// therefore covered by `every_rule_has_valid_plain_text` without editing this file,
/// unless no scenario below triggers it, in which case the test fails and names it.
fn rule_ids_in_sources() -> Vec<String> {
    let sources = [
        include_str!("rules/cpu.rs"),
        include_str!("rules/gpu.rs"),
        include_str!("rules/memory.rs"),
        include_str!("rules/storage.rs"),
        include_str!("rules/drivers.rs"),
        include_str!("rules/processes.rs"),
        include_str!("rules/system.rs"),
    ];
    let mut ids = Vec::new();
    for src in sources {
        let code = src.split("#[cfg(test)]").next().unwrap_or(src);
        for (i, lit) in code.split('"').enumerate() {
            if i % 2 == 1 && is_rule_id(lit) && !ids.iter().any(|x| x == lit) {
                ids.push(lit.to_string());
            }
        }
    }
    ids.sort();
    ids
}

struct Scenario {
    live: Vec<LiveSample>,
    inventory: Option<HardwareInventory>,
    processes: Option<ProcessSnapshot>,
    drivers: Option<DriverInventory>,
    benchmarks: Vec<BenchmarkResult>,
}

impl Scenario {
    fn run(&self) -> DiagnosticReport {
        self.run_in(Language::Italian)
    }

    fn run_in(&self, language: Language) -> DiagnosticReport {
        let mut inp = input(&self.live);
        inp.inventory = self.inventory.as_ref();
        inp.processes = self.processes.as_ref();
        inp.drivers = self.drivers.as_ref();
        inp.benchmarks = &self.benchmarks;
        inp.language = language;
        evaluate(&inp)
    }
}

fn coded(name: &str, cat: C, code: Option<u32>) -> DeviceRecord {
    let mut d = device(name, cat, H::ProblemDetected);
    d.problem_code = code;
    d.problem_description = code.map(|c| format!("Code {c}: test description."));
    d
}

fn storage_pair(kind: BenchmarkKind, id: &str, caveat: bool) -> [BenchmarkResult; 2] {
    let mut prev = bench(&format!("{id}-prev"), kind, T0);
    prev.metrics = vec![metric("read", 3000.0, Unit::BytesPerSecond, true)];
    let mut cur = prev.clone();
    cur.run_id = format!("{id}-cur");
    cur.started_at = T0 + 1000;
    cur.metrics[0].value = 1500.0;
    if caveat {
        cur.context.power_source = PowerSource::Battery;
    }
    [cur, prev]
}

/// Scenario that triggers almost every rule at once.
fn everything() -> Scenario {
    let live = live(60, |_, s| {
        s.cpu.usage_percent = avail(99.0, Unit::Percent, Source::Pdh);
        s.cpu.limit_mhz = avail(2000.0, Unit::Megahertz, Source::PowerApi);
        s.memory.available_bytes = avail(0.5 * GIB, Unit::Bytes, Source::Sysinfo);
        s.memory.commit_used_bytes = avail(31.0 * GIB, Unit::Bytes, Source::Pdh);
        s.memory.pages_per_sec = avail(5000.0, Unit::Count, Source::Pdh);
        s.memory.pressure = pressure(models::monitor::MemoryPressure::Critical);
        s.gpus[0].usage_percent = avail(99.0, Unit::Percent, Source::Pdh);
        s.gpus[0].dedicated_used_bytes = avail(7.8 * GIB, Unit::Bytes, Source::Pdh);
    });
    let mut inventory = inventory();
    inventory.volumes = vec![
        volume("C:\\", 500.0, 200.0),
        volume("D:\\", 100.0, 1.0),
        volume("E:\\", 1000.0, 99.0),
    ];
    let mut warn = disk(DiskHealthStatus::Warning);
    warn.id = "1".into();
    inventory.physical_disks = Metric::Available {
        value: vec![disk(DiskHealthStatus::Unhealthy), warn],
        unit: Unit::None,
        source: Source::Wmi,
        at: T0,
    };
    let processes = snapshot(vec![
        process("render.exe", 60.0, 6.0, 80.0),
        process("heavy.exe", 10.0, 2.0, 60.0),
        process("idle.exe", 1.0, 0.1, 1.0),
    ]);
    let mut devices = vec![
        device("Scheda Wi-Fi", C::Wifi, H::ProblemDetected),
        device("Webcam", C::Other, H::DriverMissing),
        device("Scheda video", C::Gpu, H::ProblemDetected),
        coded("Lettore impronte", C::Input, None),
        coded("Scheda audio", C::Audio, Some(14)),
        coded("Stampante", C::Usb, Some(45)),
        coded("Chiavetta", C::Usb, Some(47)),
        coded("Adattatore Bluetooth", C::Bluetooth, Some(48)),
        coded("Controller", C::StorageController, Some(52)),
        coded("Monitor", C::Display, Some(10)),
        coded("Ethernet", C::Ethernet, Some(44)),
        coded("Sensore", C::Chipset, Some(12)),
    ];
    for i in 0..12 {
        let mut d = device(&format!("Componente {i:02}"), C::Disk, H::Working);
        d.update = update_available();
        devices.push(d);
    }
    let drivers = drivers(devices, true);
    let mut sustained = bench("sus", BenchmarkKind::CpuSustained, T0 + 5000);
    sustained
        .metrics
        .push(metric(SUSTAINED_DROP_METRIC_ID, 20.0, Unit::Percent, false));
    sustained.context.power_source = PowerSource::Battery;
    let mut benchmarks = vec![sustained];
    benchmarks.extend(storage_pair(BenchmarkKind::StorageSequential, "seq", false));
    benchmarks.extend(storage_pair(BenchmarkKind::StorageRandom, "rnd", true));
    Scenario {
        live,
        inventory: Some(inventory),
        processes: Some(processes),
        drivers: Some(drivers),
        benchmarks,
    }
}

/// Variants: warning-level memory, paging without memory pressure, single process and
/// single update, this app as the dominant process, CPU temperature available.
fn variants() -> Vec<Scenario> {
    let warning_memory = Scenario {
        live: live(60, |_, s| {
            s.memory.available_bytes = avail(1.4 * GIB, Unit::Bytes, Source::Sysinfo);
            s.memory.pressure = pressure(models::monitor::MemoryPressure::Warning);
            s.cpu.temperature_c = avail(70.0, Unit::Celsius, Source::Wmi);
        }),
        inventory: None,
        processes: None,
        drivers: None,
        benchmarks: vec![{
            let mut b = bench("sus", BenchmarkKind::CpuSustained, T0);
            b.metrics
                .push(metric(SUSTAINED_DROP_METRIC_ID, 30.0, Unit::Percent, false));
            b
        }],
    };
    let paging_only = Scenario {
        live: live(60, |_, s| {
            s.memory.pages_per_sec = avail(2000.0, Unit::Count, Source::Pdh);
        }),
        inventory: None,
        processes: Some(snapshot(vec![process("big.exe", 1.0, 3.0, 10.0)])),
        drivers: None,
        benchmarks: Vec::new(),
    };
    let mut me = process("pc-diagnostics.exe", 40.0, 1.0, 70.0);
    me.is_self = true;
    let mut one_update = device("Touchpad", C::Input, H::Working);
    one_update.update = update_available();
    let single = Scenario {
        live: live(60, |_, s| {
            s.cpu.usage_percent = avail(50.0, Unit::Percent, Source::Pdh);
        }),
        inventory: None,
        processes: Some(snapshot(vec![me])),
        drivers: Some(drivers(vec![one_update], true)),
        benchmarks: vec![bench("r", BenchmarkKind::StorageRandom, T0)],
    };
    vec![warning_memory, paging_only, single]
}

fn all_reports(language: Language) -> Vec<DiagnosticReport> {
    let mut out = vec![everything().run_in(language)];
    out.extend(variants().iter().map(|s| s.run_in(language)));
    out
}

fn all_findings_in(language: Language) -> Vec<Finding> {
    all_reports(language)
        .into_iter()
        .flat_map(|r| r.findings)
        .collect()
}

fn all_findings() -> Vec<Finding> {
    all_findings_in(Language::Italian)
}

fn contains_any(text: &str, needles: &[&str]) -> Option<String> {
    let lower = text.to_lowercase();
    needles
        .iter()
        .find(|n| lower.contains(*n))
        .map(|n| (*n).to_string())
}

/// Word lists of one language for `check_plain_texts`.
struct Style {
    advice: &'static [&'static str],
    hedges: &'static [&'static str],
    causal: &'static [&'static str],
}

const ITALIAN: Style = Style {
    advice: &BANNED_ADVICE,
    hedges: &HEDGES,
    causal: &CAUSAL_WORDS,
};

const ENGLISH: Style = Style {
    advice: &BANNED_ADVICE_EN,
    hedges: &HEDGES_EN,
    causal: &CAUSAL_WORDS_EN,
};

#[test]
fn every_rule_has_valid_plain_text() {
    check_plain_texts(&all_findings(), &ITALIAN);
}

#[test]
fn every_rule_has_valid_english_plain_text() {
    check_plain_texts(&all_findings_in(Language::English), &ENGLISH);
}

fn check_plain_texts(findings: &[Finding], style: &Style) {
    let mut seen: Vec<String> = findings.iter().map(|f| f.rule_id.clone()).collect();
    seen.sort();
    seen.dedup();
    let expected = rule_ids_in_sources();
    assert!(expected.len() >= 19, "rule id scan broken: {expected:?}");
    for id in &expected {
        assert!(
            seen.contains(id),
            "no scenario triggers {id}: extend the scenarios"
        );
    }

    for f in findings {
        let p = &f.simple;
        let id = &f.rule_id;
        assert!(!p.headline.trim().is_empty(), "{id}: empty headline");
        assert!(!p.meaning.trim().is_empty(), "{id}: empty meaning");
        assert!(!p.action.trim().is_empty(), "{id}: empty action");
        assert!(
            !p.headline.ends_with('.') && !p.headline.contains(". "),
            "{id}: headline must be one short sentence: {}",
            p.headline
        );
        assert!(p.headline.chars().count() <= 110, "{id}: headline too long");
        let all = format!("{} {} {}", p.headline, p.meaning, p.action);
        if let Some(t) = contains_any(&all, &BANNED_JARGON) {
            panic!("{id}: jargon {t:?} in plain text: {all}");
        }
        if let Some(t) = contains_any(&all, style.advice) {
            panic!("{id}: banned advice {t:?} in plain text: {all}");
        }
        let claim = format!("{} {}", p.headline, p.meaning);
        if f.kind == FindingKind::Hypothesis || f.confidence == Confidence::Low {
            assert!(
                contains_any(&claim, style.hedges).is_some(),
                "{id}: hypothesis / low confidence must be hedged: {claim}"
            );
        }
        if f.kind == FindingKind::Observation
            && let Some(t) = contains_any(&claim, style.causal)
        {
            panic!("{id}: observation claims a cause ({t:?}): {claim}");
        }
    }
}

/// The rules do the same work in both languages: same findings, same order, same facts;
/// every text differs and the English report carries no Italian.
#[test]
fn english_reports_mirror_italian_ones() {
    let (it, en) = (
        all_reports(Language::Italian),
        all_reports(Language::English),
    );
    assert_eq!(it.len(), en.len());
    for (a, b) in it.iter().zip(&en) {
        assert_eq!(a.findings.len(), b.findings.len());
        assert_eq!(a.coverage.len(), b.coverage.len());
        assert_eq!(a.limitations.len(), b.limitations.len());
        for (x, y) in a.findings.iter().zip(&b.findings) {
            let id = &x.rule_id;
            assert_eq!(
                (id, x.severity, x.kind, x.confidence, x.component),
                (&y.rule_id, y.severity, y.kind, y.confidence, y.component)
            );
            assert_eq!(x.evidence.len(), y.evidence.len(), "{id}");
            assert_eq!(
                x.evidence.iter().map(|e| e.source).collect::<Vec<_>>(),
                y.evidence.iter().map(|e| e.source).collect::<Vec<_>>(),
                "{id}"
            );
            assert_eq!(x.possible_causes.len(), y.possible_causes.len(), "{id}");
            assert_eq!(x.limitations.len(), y.limitations.len(), "{id}");
            for (it_text, en_text) in [
                (&x.title, &y.title),
                (&x.description, &y.description),
                (&x.recommendation, &y.recommendation),
                (&x.simple.headline, &y.simple.headline),
                (&x.simple.meaning, &y.simple.meaning),
                (&x.simple.action, &y.simple.action),
            ] {
                assert_ne!(it_text, en_text, "{id}: text not translated");
            }
        }
        for (x, y) in a.coverage.iter().zip(&b.coverage) {
            assert_eq!((x.component, x.status), (y.component, y.status));
            assert_ne!(
                x.note, y.note,
                "{:?}: coverage note not translated",
                x.component
            );
        }
        let json = serde_json::to_string(b).expect("json").to_lowercase();
        if let Some(t) = contains_any(&json, &ITALIAN_MARKERS) {
            panic!("Italian {t:?} in an English report: {json}");
        }
    }
}

#[test]
fn english_plain_texts_use_english_formatting() {
    let findings = all_findings_in(Language::English);
    let low = get(&findings, "memory.available.low", |f| {
        f.severity == Severity::Critical
    });
    assert_eq!(low.simple.headline, "The PC almost ran out of free memory");
    assert!(
        low.simple.action.ends_with(
            "The programs using the most memory were: render.exe, heavy.exe, and idle.exe."
        ),
        "{}",
        low.simple.action
    );
    let space = get(&findings, "storage.volume.low_space", |f| {
        f.severity == Severity::Critical
    });
    assert_eq!(space.simple.headline, "Drive D: is almost full");
    assert!(
        space
            .description
            .contains("1.0 GiB free of 100.0 GiB (1.0%)")
    );
    assert!(
        space
            .simple
            .meaning
            .starts_with("Only 1.0 GB of 100.0 GB is free")
    );
    let result = get(&findings, "storage.benchmark.result", |_| true);
    assert_eq!(result.simple.action, "No action needed.");
    assert!(
        result
            .description
            .starts_with("Values measured on Nov 4, 2026, ")
    );
    let single = get(&findings, "drivers.updates.available", |f| {
        f.simple.headline == "A driver update is available"
    });
    assert_eq!(
        single.description,
        "Windows Update offers an applicable driver for 1 device."
    );
}

fn get<'a>(findings: &'a [Finding], id: &str, pred: impl Fn(&Finding) -> bool) -> &'a Finding {
    findings
        .iter()
        .find(|f| f.rule_id == id && pred(f))
        .unwrap_or_else(|| panic!("{id} not found"))
}

#[test]
fn memory_low_plain_text_names_top_processes() {
    let s = live(60, |_, s| {
        s.memory.available_bytes = avail(1.28 * GIB, Unit::Bytes, Source::Sysinfo); // 8 %
    });
    let snap = snapshot(vec![
        process("small.exe", 1.0, 0.5, 5.0),
        process("big.exe", 1.0, 6.0, 30.0),
    ]);
    let mut inp = input(&s);
    inp.processes = Some(&snap);
    let r = evaluate(&inp);
    let f = get(&r.findings, "memory.available.low", |_| true);
    assert_eq!(f.simple.headline, "Il PC sta usando quasi tutta la memoria");
    assert_eq!(
        f.simple.meaning,
        "La memoria (RAM, dove il PC tiene i programmi aperti) è stata quasi piena per gran parte del tempo osservato: in media era occupato il 92 % del totale. Quando la memoria è quasi piena il PC può diventare lento e i programmi possono bloccarsi per qualche istante."
    );
    assert_eq!(
        f.simple.action,
        "Chiudi i programmi e le schede del browser che non ti servono, poi ripeti la misurazione mentre il problema si presenta. I programmi che usavano più memoria erano: big.exe e small.exe."
    );
}

#[test]
fn device_plain_text_depends_on_problem_code_and_category() {
    let findings = everything().run().findings;
    let wifi = get(&findings, "drivers.device.problem", |f| {
        f.title.contains("Scheda Wi-Fi")
    });
    assert_eq!(
        wifi.simple.headline,
        "Windows ha fermato un dispositivo perché ha dato errori: Scheda Wi-Fi"
    );
    assert_eq!(
        wifi.simple.meaning,
        "Il dispositivo ha segnalato un problema e Windows lo ha bloccato, quindi al momento non funziona. La connessione Wi-Fi potrebbe non funzionare."
    );
    assert_eq!(
        wifi.simple.action,
        "Riavvia il PC. Se il problema resta, reinstalla il driver scaricandolo dal sito del produttore del dispositivo."
    );
    let cam = get(&findings, "drivers.device.driver_missing", |_| true);
    assert_eq!(
        cam.simple.headline,
        "Manca il driver di un dispositivo: Webcam"
    );
    assert!(cam.simple.action.contains("Windows Update"));
    let audio = get(&findings, "drivers.device.problem", |f| {
        f.title.contains("Scheda audio")
    });
    assert_eq!(
        audio.simple.headline,
        "Un dispositivo ha bisogno di un riavvio del PC: Scheda audio"
    );
    assert_eq!(audio.simple.action, "Riavvia il PC.");
    let generic = get(&findings, "drivers.device.problem", |f| {
        f.title.contains("Sensore")
    });
    assert_eq!(
        generic.simple.headline,
        "Un dispositivo non funziona correttamente: Sensore"
    );
    let gpu = get(&findings, "gpu.driver.problem", |_| true);
    assert!(
        gpu.simple.meaning.ends_with(
            "Lo schermo, i giochi e i video potrebbero funzionare male o più lentamente."
        )
    );
}

#[test]
fn storage_and_cpu_plain_texts() {
    let findings = everything().run().findings;
    let d = get(&findings, "storage.volume.low_space", |f| {
        f.severity == Severity::Critical
    });
    assert_eq!(d.simple.headline, "Il disco D: è quasi pieno");
    assert!(
        d.simple
            .meaning
            .starts_with("Restano liberi solo 1,0 GB su 100,0 GB, cioè l'1 % dello spazio."),
        "{}",
        d.simple.meaning
    );
    let e = get(&findings, "storage.volume.low_space", |f| {
        f.severity == Severity::Warning
    });
    assert_eq!(e.simple.headline, "Sul disco E: resta poco spazio libero");

    let cause = get(&findings, "cpu.frequency.limit_cause", |_| true);
    assert_eq!(
        cause.simple.headline,
        "Il processore potrebbe essere rallentato per risparmiare energia o per il calore"
    );
    assert!(
        cause
            .simple
            .meaning
            .ends_with("Durante l'ultimo test il PC era alimentato a batteria.")
    );

    let rnd = get(&findings, "storage.benchmark.drop", |f| {
        f.confidence == Confidence::Low
    });
    assert!(
        rnd.simple
            .meaning
            .contains("il confronto è poco affidabile")
    );
    let seq = get(&findings, "storage.benchmark.drop", |f| {
        f.confidence == Confidence::Medium
    });
    assert!(!seq.simple.meaning.contains("poco affidabile"));
    assert!(seq.simple.meaning.contains("peggiorata di circa il 50 %"));

    let result = get(&findings, "storage.benchmark.result", |_| true);
    assert_eq!(
        result.simple.action,
        crate::rules::no_action(models::settings::Language::Italian)
    );
}

#[test]
fn processes_and_updates_plain_text_singular_and_plural() {
    let findings = everything().run().findings;
    let many = get(&findings, "processes.impact.high", |_| true);
    assert_eq!(
        many.simple.headline,
        "2 programmi stanno usando molte risorse del PC"
    );
    assert!(many.simple.meaning.contains("render.exe e heavy.exe"));
    let upd = get(&findings, "drivers.updates.available", |_| true);
    assert_eq!(
        upd.simple.headline,
        "Sono disponibili aggiornamenti per 12 driver"
    );
    assert!(upd.simple.meaning.contains("Componente 09 e altri 2."));

    let single = variants().remove(2).run().findings;
    let one = get(&single, "processes.impact.high", |_| true);
    assert_eq!(
        one.simple.headline,
        "Un programma sta usando molte risorse del PC: pc-diagnostics.exe"
    );
    assert!(
        one.simple
            .meaning
            .ends_with("È questa stessa applicazione (PC Diagnostics).")
    );
    let dom = get(&single, "cpu.process.dominant", |_| true);
    assert!(dom.simple.meaning.contains("È questa stessa applicazione"));
    let upd = get(&single, "drivers.updates.available", |_| true);
    assert_eq!(
        upd.simple.headline,
        "È disponibile un aggiornamento per un driver"
    );
}

#[test]
fn rule_id_scan_finds_known_rules() {
    let ids = rule_ids_in_sources();
    for id in [
        "cpu.utilization.sustained",
        "gpu.driver.problem",
        "drivers.device.driver_missing",
        "storage.benchmark.drop",
        "processes.impact.high",
    ] {
        assert!(ids.iter().any(|x| x == id), "{id}");
    }
    assert!(!is_rule_id("throughput_drop_percent"));
    assert!(!is_rule_id("Code 43: x."));
}

/// `cargo test -p diagnostics dump_plain_texts -- --ignored --nocapture` prints every
/// plain-language text of the scenarios in both languages, for proofreading.
#[test]
#[ignore = "manual proofreading aid"]
fn dump_plain_texts() {
    let mut findings = all_findings();
    findings.extend(all_findings_in(Language::English));
    for f in findings {
        println!(
            "[{}] {:?}/{:?}\n  H: {}\n  M: {}\n  A: {}\n",
            f.rule_id, f.kind, f.confidence, f.simple.headline, f.simple.meaning, f.simple.action
        );
    }
}
