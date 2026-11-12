//! Deterministic diagnostics engine, benchmark comparison and report rendering.
//!
//! Pure functions: no I/O, no clock, no randomness, no network. The same input always
//! yields the same report. Rules and thresholds: docs/diagnostics-rules.md.
//!
//! Every finding carries an epistemic status (observation / hypothesis / confirmed).
//! There is no global health score: each component has its own findings plus a coverage
//! item, and missing data yields `Partial`/`NotEvaluated` coverage, never "all good".

#![forbid(unsafe_code)]

mod compare;
pub mod fmt;
mod report;
pub mod rules;
mod window;

#[cfg(test)]
mod macos_tests;
#[cfg(test)]
mod plain_tests;
#[cfg(test)]
mod testdata;

use models::app::Platform;
use models::benchmark_run::BenchmarkResult;
use models::component::ComponentKind;
use models::diagnostics::DiagnosticReport;
use models::drivers::DriverInventory;
use models::hardware::HardwareInventory;
use models::monitor::LiveSample;
use models::process::ProcessSnapshot;
use models::settings::Language;
use models::system::SystemInfo;

pub use compare::{
    COMPARE_AVAILABLE_MEMORY_DIFF_PERCENT, COMPARE_BACKGROUND_CPU_PERCENT, COMPARE_CPU_LIMIT_RATIO,
    compare, compare_in, find_previous_comparable,
};
pub use report::{ReportBundle, render_html, render_json, report_file_name};
pub use window::{SHORT_WINDOW_S, SUSTAINED_MIN_WINDOW_S, SUSTAINED_SHARE_PERCENT};

/// Version of the rule set. Bump whenever a rule, threshold or text meaning changes.
pub const RULES_VERSION: u32 = 1;

/// Everything the rules may look at. Missing parts reduce coverage, they never fail.
#[derive(Debug, Clone, Copy)]
pub struct DiagnosticInput<'a> {
    pub app_version: &'a str,
    /// Report time (Unix ms), supplied by the caller: the engine never reads the clock.
    pub now_ms: u64,
    pub system: Option<&'a SystemInfo>,
    pub inventory: Option<&'a HardwareInventory>,
    /// Live samples, oldest first (~1 s cadence, up to 5 min).
    pub live: &'a [LiveSample],
    pub processes: Option<&'a ProcessSnapshot>,
    pub drivers: Option<&'a DriverInventory>,
    /// Benchmark history, newest first.
    pub benchmarks: &'a [BenchmarkResult],
    /// Language of every text in the report (the effective UI language at generation
    /// time). A stored report keeps it: texts are never re-translated.
    pub language: Language,
    /// Platform the data was collected on (`Platform::current()` in the app). It selects
    /// the OS-specific wording (tool names, "Windows" vs "macOS"); thresholds and rules
    /// are the same everywhere. Texts for platforms other than Windows are written for
    /// macOS, the only other platform with real providers.
    pub platform: Platform,
}

const COMPONENT_ORDER: [ComponentKind; 7] = [
    ComponentKind::Cpu,
    ComponentKind::Gpu,
    ComponentKind::Memory,
    ComponentKind::Storage,
    ComponentKind::Drivers,
    ComponentKind::Processes,
    ComponentKind::System,
];

fn component_rank(c: ComponentKind) -> usize {
    COMPONENT_ORDER
        .iter()
        .position(|x| *x == c)
        .unwrap_or(usize::MAX)
}

/// Runs every rule. Deterministic: the id is `diag-<now_ms>`.
pub fn evaluate(input: &DiagnosticInput) -> DiagnosticReport {
    let window_s = window::observed_window_s(input.live);
    let lang = input.language;
    let ctx = rules::Ctx {
        input,
        window_s,
        lang,
        platform: input.platform,
    };

    let mut out = rules::Outcome::default();
    out.extend(rules::cpu::evaluate(&ctx));
    out.extend(rules::gpu::evaluate(&ctx));
    out.extend(rules::memory::evaluate(&ctx));
    out.extend(rules::storage::evaluate(&ctx));
    out.extend(rules::drivers::evaluate(&ctx));
    out.extend(rules::processes::evaluate(&ctx));
    out.extend(rules::system::evaluate(&ctx));

    let mut findings = out.findings;
    // Stable sort: ties keep rule-evaluation order, which is itself deterministic.
    findings.sort_by(|a, b| {
        b.severity
            .cmp(&a.severity)
            .then_with(|| rules::kind_rank(a.kind).cmp(&rules::kind_rank(b.kind)))
            .then_with(|| a.rule_id.cmp(&b.rule_id))
    });

    let mut coverage = out.coverage;
    coverage.sort_by_key(|c| component_rank(c.component));

    let mut limitations = vec![lang.pick(
        format!("Diagnosi basata su regole deterministiche (versione {RULES_VERSION}) applicate ai soli dati disponibili; non è un'analisi completa del sistema."),
        format!("Diagnosis based on deterministic rules (version {RULES_VERSION}) applied only to the available data; it is not a complete analysis of the system."),
    )];
    if input.live.is_empty() {
        limitations.push(
            lang.pick(
                "Nessuno storico di monitoraggio disponibile: le regole su CPU, memoria e GPU non sono state valutate.",
                "No monitoring history available: the CPU, memory and GPU rules were not evaluated.",
            )
            .into(),
        );
    } else {
        let w = fmt::seconds(window_s);
        limitations.push(lang.pick(
            format!("Finestra di monitoraggio osservata: {w}. I risultati descrivono solo questo intervallo."),
            format!("Observed monitoring window: {w}. The results describe only this period."),
        ));
    }
    for l in out.limitations {
        if !limitations.contains(&l) {
            limitations.push(l);
        }
    }
    if input.benchmarks.is_empty() {
        limitations.push(
            lang.pick(
                "Nessun benchmark disponibile: nessuna valutazione delle prestazioni misurate.",
                "No benchmarks available: measured performance was not assessed.",
            )
            .into(),
        );
    }
    limitations.push(
        lang.pick(
            "L'assenza di risultati per un componente non significa che sia tutto in ordine: consultare la copertura.",
            "No findings for a component does not mean everything is fine: check the coverage.",
        )
        .into(),
    );

    DiagnosticReport {
        id: format!("diag-{}", input.now_ms),
        generated_at: input.now_ms,
        app_version: input.app_version.to_string(),
        rules_version: RULES_VERSION,
        observed_window_s: window_s,
        findings,
        coverage,
        limitations,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testdata::*;
    use models::diagnostics::{CoverageStatus, FindingKind, Severity};
    use models::drivers::{DeviceCategory, DeviceHealth};
    use models::{Source, Unit};

    #[test]
    fn empty_input_is_partial_never_all_good() {
        let r = evaluate(&input(&[]));
        assert_eq!(r.id, format!("diag-{}", T0 + 600_000));
        assert_eq!(r.rules_version, RULES_VERSION);
        assert!(r.findings.is_empty());
        assert_eq!(r.coverage.len(), 7);
        let order: Vec<_> = r.coverage.iter().map(|c| c.component).collect();
        assert_eq!(order, COMPONENT_ORDER);
        assert!(
            r.coverage
                .iter()
                .all(|c| c.status == CoverageStatus::NotEvaluated),
            "{:?}",
            r.coverage
        );
        assert!(r.limitations.iter().any(|l| l.contains("Nessuno storico")));
    }

    #[test]
    fn every_component_has_one_coverage_item_with_full_input() {
        let samples = live(120, |_, _| {});
        let sys = system();
        let inv = inventory();
        let snap = snapshot(vec![process("a.exe", 1.0, 0.1, 1.0)]);
        let drv = drivers(
            vec![device("x", DeviceCategory::Usb, DeviceHealth::Working)],
            true,
        );
        let mut inp = input(&samples);
        inp.system = Some(&sys);
        inp.inventory = Some(&inv);
        inp.processes = Some(&snap);
        inp.drivers = Some(&drv);
        let r = evaluate(&inp);
        for c in COMPONENT_ORDER {
            assert_eq!(r.coverage.iter().filter(|x| x.component == c).count(), 1);
        }
        assert!(r.coverage.iter().all(|c| !c.note.is_empty()));
        assert_eq!(r.observed_window_s, 120);
    }

    #[test]
    fn findings_are_sorted_by_severity_kind_rule() {
        let samples = live(60, |_, s| {
            s.cpu.usage_percent = avail(99.0, Unit::Percent, Source::Pdh); // warning observation
            s.cpu.limit_mhz = avail(1000.0, Unit::Megahertz, Source::PowerApi); // info obs + hyp
        });
        let mut inv = inventory();
        inv.volumes = vec![volume("C:\\", 100.0, 1.0)]; // critical confirmed
        let drv = drivers(
            vec![device(
                "Cam",
                DeviceCategory::Other,
                DeviceHealth::ProblemDetected,
            )], // warning confirmed
            true,
        );
        let mut inp = input(&samples);
        inp.inventory = Some(&inv);
        inp.drivers = Some(&drv);
        let r = evaluate(&inp);
        let got: Vec<_> = r
            .findings
            .iter()
            .map(|f| (f.severity, f.kind, f.rule_id.as_str()))
            .collect();
        assert_eq!(
            got,
            vec![
                (
                    Severity::Critical,
                    FindingKind::Confirmed,
                    "storage.volume.low_space"
                ),
                (
                    Severity::Warning,
                    FindingKind::Confirmed,
                    "drivers.device.problem"
                ),
                (
                    Severity::Warning,
                    FindingKind::Observation,
                    "cpu.utilization.sustained"
                ),
                (
                    Severity::Info,
                    FindingKind::Hypothesis,
                    "cpu.frequency.limit_cause"
                ),
                (
                    Severity::Info,
                    FindingKind::Observation,
                    "cpu.frequency.limit_reported"
                ),
            ]
        );
    }

    #[test]
    fn evaluation_is_deterministic() {
        let samples = live(90, |i, s| {
            s.cpu.usage_percent = avail((i % 100) as f64, Unit::Percent, Source::Pdh);
        });
        let inv = inventory();
        let mut inp = input(&samples);
        inp.inventory = Some(&inv);
        assert_eq!(evaluate(&inp), evaluate(&inp));
    }

    #[test]
    fn no_finding_suggests_buying_or_disabling() {
        let samples = live(60, |_, s| {
            s.cpu.usage_percent = avail(99.0, Unit::Percent, Source::Pdh);
            s.memory.available_bytes = avail(0.1 * GIB, Unit::Bytes, Source::Sysinfo);
            s.memory.pages_per_sec = avail(9000.0, Unit::Count, Source::Pdh);
        });
        let r = evaluate(&input(&samples));
        let text = serde_json::to_string(&r).expect("json").to_lowercase();
        for banned in ["acquist", "compra", "sostitu", "disattiva", "disabilita"] {
            assert!(!text.contains(banned), "{banned}");
        }
    }

    #[test]
    fn report_limitations_follow_the_language() {
        let mut inp = input(&[]);
        inp.language = Language::English;
        let r = evaluate(&inp);
        assert!(
            r.limitations[0].starts_with("Diagnosis based on deterministic rules (version "),
            "{:?}",
            r.limitations
        );
        assert!(
            r.limitations
                .iter()
                .any(|l| l.starts_with("No monitoring history available"))
        );
        assert!(
            r.limitations
                .last()
                .is_some_and(|l| l.ends_with("check the coverage."))
        );
        assert!(r.coverage.iter().all(|c| !c.note.is_empty()));
        // Same structure as the Italian report.
        let it = evaluate(&input(&[]));
        assert_eq!(it.limitations.len(), r.limitations.len());
        assert_eq!(it.coverage.len(), r.coverage.len());
    }
}
