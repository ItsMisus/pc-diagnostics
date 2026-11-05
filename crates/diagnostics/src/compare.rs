//! Benchmark comparison.

use models::benchmark::BenchmarkState;
use models::benchmark_run::{BenchmarkResult, PowerSource};
use models::diagnostics::{BenchmarkComparison, MetricDelta};
use models::settings::Language;

use crate::fmt;
use crate::window::val;

/// Background CPU at start above this (percent of the machine) weakens a comparison.
pub const COMPARE_BACKGROUND_CPU_PERCENT: f64 = 10.0;
/// OS frequency limit counts as "below max" when limit < max × this ratio.
pub const COMPARE_CPU_LIMIT_RATIO: f64 = 0.99;
/// Relative difference of available memory at start (vs the larger value) that weakens a comparison.
pub const COMPARE_AVAILABLE_MEMORY_DIFF_PERCENT: f64 = 25.0;

/// Same as `compare_in` with Italian caveats. Kept for callers that do not pass the
/// language yet; new code should call `compare_in`.
pub fn compare(current: &BenchmarkResult, previous: &BenchmarkResult) -> BenchmarkComparison {
    compare_in(Language::Italian, current, previous)
}

/// Compares two runs; the caveats are written in `lang`.
pub fn compare_in(
    lang: Language,
    current: &BenchmarkResult,
    previous: &BenchmarkResult,
) -> BenchmarkComparison {
    let mut out = BenchmarkComparison {
        current_run_id: current.run_id.clone(),
        previous_run_id: previous.run_id.clone(),
        comparable: current.comparable_with(previous),
        caveats: Vec::new(),
        deltas: Vec::new(),
    };
    if !out.comparable {
        out.caveats = incompatibilities(lang, current, previous);
        return out;
    }

    for m in &current.metrics {
        if let Some(p) = previous.metrics.iter().find(|p| p.id == m.id) {
            let change_percent = (p.value != 0.0 && p.value.is_finite() && m.value.is_finite())
                .then(|| (m.value - p.value) / p.value * 100.0);
            out.deltas.push(MetricDelta {
                id: m.id.clone(),
                label: m.label.clone(),
                unit: m.unit,
                current: m.value,
                previous: p.value,
                change_percent,
                higher_is_better: m.higher_is_better,
            });
        }
    }
    out.caveats = context_caveats(lang, current, previous);
    out
}

/// ("attuale", "precedente") / ("current", "previous"): the run a caveat is about.
fn run_labels(lang: Language) -> (&'static str, &'static str) {
    lang.pick(("attuale", "precedente"), ("current", "previous"))
}

fn not_detected(lang: Language) -> &'static str {
    lang.pick("non rilevata", "not detected")
}

fn incompatibilities(lang: Language, c: &BenchmarkResult, p: &BenchmarkResult) -> Vec<String> {
    let mut v = Vec::new();
    let (cur, prev) = run_labels(lang);
    let vs = lang.pick("contro", "vs");
    if c.kind != p.kind {
        v.push(
            lang.pick("Tipo di benchmark diverso.", "Different benchmark type.")
                .into(),
        );
    }
    for (label, r) in [(cur, c), (prev, p)] {
        if r.state != BenchmarkState::Completed {
            let state = crate::report::state_label(lang, r.state);
            v.push(lang.pick(
                format!("L'esecuzione {label} non è stata completata ({state})."),
                format!("The {label} run was not completed ({state})."),
            ));
        }
    }
    if c.methodology_version != p.methodology_version {
        let (a, b) = (c.methodology_version, p.methodology_version);
        v.push(lang.pick(
            format!("Versione della metodologia diversa ({a} {vs} {b})."),
            format!("Different methodology version ({a} {vs} {b})."),
        ));
    }
    if c.build_profile != p.build_profile {
        let (a, b) = (&c.build_profile, &p.build_profile);
        v.push(lang.pick(
            format!("Profilo di compilazione diverso ({a} {vs} {b})."),
            format!("Different build profile ({a} {vs} {b})."),
        ));
    }
    // The GPU id (a LUID) changes across reboots; the GPU is matched by name and driver.
    let (a, b) = (&c.request, &p.request);
    if a.kind != b.kind
        || a.duration_s != b.duration_s
        || a.threads != b.threads
        || a.storage_volume != b.storage_volume
        || a.storage_file_mib != b.storage_file_mib
        || a.storage_write_consent != b.storage_write_consent
    {
        let mut fields = Vec::new();
        if a.kind != b.kind {
            fields.push(lang.pick("tipo", "type"));
        }
        if a.duration_s != b.duration_s {
            fields.push(lang.pick("durata", "duration"));
        }
        if a.threads != b.threads {
            fields.push(lang.pick("thread", "threads"));
        }
        if a.storage_volume != b.storage_volume {
            fields.push("volume");
        }
        if a.storage_file_mib != b.storage_file_mib {
            fields.push(lang.pick("dimensione del file di test", "test file size"));
        }
        if a.storage_write_consent != b.storage_write_consent {
            fields.push(lang.pick("consenso alla scrittura", "write consent"));
        }
        let fields = fields.join(", ");
        v.push(lang.pick(
            format!("Parametri del test diversi: {fields}."),
            format!("Different test parameters: {fields}."),
        ));
    }
    if c.context.cpu_brand != p.context.cpu_brand {
        v.push(
            lang.pick("Modello di CPU diverso.", "Different CPU model.")
                .into(),
        );
    }
    if c.context.logical_processors != p.context.logical_processors {
        let (a, b) = (c.context.logical_processors, p.context.logical_processors);
        v.push(lang.pick(
            format!("Numero di processori logici diverso ({a} {vs} {b})."),
            format!("Different number of logical processors ({a} {vs} {b})."),
        ));
    }
    if c.context.total_memory_bytes != p.context.total_memory_bytes {
        let a = fmt::bytes_u64(lang, c.context.total_memory_bytes);
        let b = fmt::bytes_u64(lang, p.context.total_memory_bytes);
        v.push(lang.pick(
            format!("Memoria totale diversa ({a} {vs} {b})."),
            format!("Different total memory ({a} {vs} {b})."),
        ));
    }
    if c.context.storage_model != p.context.storage_model {
        v.push(
            lang.pick(
                "Modello del disco testato diverso.",
                "Different tested disk model.",
            )
            .into(),
        );
    }
    if c.context.gpu_name != p.context.gpu_name {
        v.push(
            lang.pick("GPU testata diversa.", "Different tested GPU.")
                .into(),
        );
    } else if c.context.gpu_driver_version != p.context.gpu_driver_version {
        let nd = not_detected(lang);
        let a = c.context.gpu_driver_version.as_deref().unwrap_or(nd);
        let b = p.context.gpu_driver_version.as_deref().unwrap_or(nd);
        v.push(lang.pick(
            format!("Versione del driver GPU diversa ({a} {vs} {b})."),
            format!("Different GPU driver version ({a} {vs} {b})."),
        ));
    }
    v
}

fn power_label(lang: Language, p: PowerSource) -> &'static str {
    match p {
        PowerSource::Ac => lang.pick("rete elettrica", "AC power"),
        PowerSource::Battery => lang.pick("batteria", "battery"),
        PowerSource::Unknown => lang.pick("sconosciuta", "unknown"),
    }
}

fn context_caveats(lang: Language, c: &BenchmarkResult, p: &BenchmarkResult) -> Vec<String> {
    let mut v = Vec::new();
    let (cc, pc) = (&c.context, &p.context);
    let (cur, prev) = run_labels(lang);
    if cc.power_source != pc.power_source {
        let (now, before) = (
            power_label(lang, cc.power_source),
            power_label(lang, pc.power_source),
        );
        v.push(lang.pick(
            format!("Alimentazione diversa: {now} ora, {before} prima."),
            format!("Different power source: {now} now, {before} before."),
        ));
    }
    if cc.power_plan != pc.power_plan {
        let nd = not_detected(lang);
        let now = cc.power_plan.as_deref().unwrap_or(nd);
        let before = pc.power_plan.as_deref().unwrap_or(nd);
        v.push(lang.pick(
            format!("Modalità di alimentazione diversa: {now} ora, {before} prima."),
            format!("Different power mode: {now} now, {before} before."),
        ));
    }
    for (label, ctx) in [(cur, cc), (prev, pc)] {
        if let Some(bg) = val(&ctx.background_cpu_percent)
            && bg > COMPARE_BACKGROUND_CPU_PERCENT
        {
            let bg = fmt::pct(lang, bg);
            v.push(lang.pick(
                format!(
                    "Carico CPU in background elevato all'avvio dell'esecuzione {label}: {bg}."
                ),
                format!("High background CPU load at the start of the {label} run: {bg}."),
            ));
        }
    }
    for (label, ctx) in [(cur, cc), (prev, pc)] {
        if let (Some(lim), Some(max)) = (val(&ctx.cpu_limit_mhz), val(&ctx.cpu_max_mhz))
            && max > 0.0
            && lim < max * COMPARE_CPU_LIMIT_RATIO
        {
            let (lim, max) = (fmt::mhz(lang, lim), fmt::mhz(lang, max));
            v.push(lang.pick(
                format!("Limite di frequenza CPU attivo all'avvio dell'esecuzione {label}: {lim} su {max}."),
                format!("CPU frequency limit active at the start of the {label} run: {lim} of {max}."),
            ));
        }
    }
    if let (Some(a), Some(b)) = (
        val(&cc.available_memory_bytes),
        val(&pc.available_memory_bytes),
    ) {
        let larger = a.max(b);
        if larger > 0.0 && (a - b).abs() / larger * 100.0 > COMPARE_AVAILABLE_MEMORY_DIFF_PERCENT {
            let (now, before) = (fmt::bytes(lang, a), fmt::bytes(lang, b));
            v.push(lang.pick(
                format!("Memoria disponibile all'avvio molto diversa: {now} ora, {before} prima."),
                format!("Very different available memory at start: {now} now, {before} before."),
            ));
        }
    }
    // A warning both runs share (e.g. "CPU temperature not observable") is a condition of
    // the machine, not a difference between the runs: only one-sided warnings weaken it.
    for (label, r, other) in [(cur, c, p), (prev, p, c)] {
        let own: Vec<&str> = r
            .warnings
            .iter()
            .filter(|w| !other.warnings.contains(w))
            .map(String::as_str)
            .collect();
        if !own.is_empty() {
            let own = own.join("; ");
            v.push(lang.pick(
                format!("Solo l'esecuzione {label} ha avvisi: {own}"),
                format!("Only the {label} run has warnings: {own}"),
            ));
        }
    }
    v
}

/// The newest run in `history` that started before `current` and is comparable with it.
/// `history` is expected newest first, but the result does not depend on its order.
pub fn find_previous_comparable<'a>(
    current: &BenchmarkResult,
    history: &'a [BenchmarkResult],
) -> Option<&'a BenchmarkResult> {
    history
        .iter()
        .filter(|h| {
            h.run_id != current.run_id
                && h.started_at < current.started_at
                && current.comparable_with(h)
        })
        .max_by(|a, b| {
            a.started_at
                .cmp(&b.started_at)
                .then_with(|| b.run_id.cmp(&a.run_id))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testdata::*;
    use models::benchmark::BenchmarkKind;
    use models::{Source, Unit};

    #[test]
    fn comparable_runs_produce_deltas() {
        let prev = bench("p", BenchmarkKind::CpuMultiCore, T0);
        let mut cur = bench("c", BenchmarkKind::CpuMultiCore, T0 + 1000);
        cur.metrics[0].value = 900.0;
        cur.metrics
            .push(metric("only_current", 1.0, Unit::Count, true));
        let cmp = compare(&cur, &prev);
        assert!(cmp.comparable);
        assert_eq!(cmp.deltas.len(), 1);
        let d = &cmp.deltas[0];
        assert_eq!((d.current, d.previous), (900.0, 1000.0));
        let pct = d.change_percent.expect("pct");
        assert!((pct - -10.0).abs() < 1e-9);
        assert!(cmp.caveats.is_empty(), "{:?}", cmp.caveats);
    }

    #[test]
    fn zero_previous_has_no_percent() {
        let mut prev = bench("p", BenchmarkKind::CpuMultiCore, T0);
        prev.metrics[0].value = 0.0;
        let cur = bench("c", BenchmarkKind::CpuMultiCore, T0 + 1);
        assert_eq!(compare(&cur, &prev).deltas[0].change_percent, None);
    }

    #[test]
    fn not_comparable_explains_fields() {
        let prev = bench("p", BenchmarkKind::CpuMultiCore, T0);
        let mut cur = bench("c", BenchmarkKind::CpuMultiCore, T0 + 1);
        cur.request.threads = Some(4);
        cur.build_profile = "debug".into();
        let cmp = compare(&cur, &prev);
        assert!(!cmp.comparable);
        assert!(cmp.deltas.is_empty());
        assert!(cmp.caveats.iter().any(|c| c.contains("thread")));
        assert!(cmp.caveats.iter().any(|c| c.contains("compilazione")));

        let mut cur = bench("c", BenchmarkKind::CpuMultiCore, T0 + 1);
        cur.state = BenchmarkState::Failed;
        let cmp = compare(&cur, &prev);
        assert!(!cmp.comparable);
        assert!(
            cmp.caveats
                .iter()
                .any(|c| c.contains("non è stata completata"))
        );

        let cur = bench("c", BenchmarkKind::CpuSingleCore, T0 + 1);
        assert!(
            compare(&cur, &prev)
                .caveats
                .iter()
                .any(|c| c.contains("Tipo"))
        );
    }

    #[test]
    fn gpu_identity_decides_comparability_not_the_adapter_id() {
        let mut prev = bench("p", BenchmarkKind::GpuCompute, T0);
        prev.context.gpu_name = Some("GPU A".into());
        prev.context.gpu_driver_version = Some("1.0".into());
        prev.request.gpu_id = Some("luid:00000000-00000001".into());
        let mut cur = prev.clone();
        cur.run_id = "c".into();
        cur.started_at = T0 + 1;
        // Same GPU after a reboot: new LUID, still comparable.
        cur.request.gpu_id = Some("luid:00000000-00000002".into());
        assert!(compare(&cur, &prev).comparable);

        cur.context.gpu_driver_version = Some("2.0".into());
        let cmp = compare(&cur, &prev);
        assert!(!cmp.comparable);
        assert!(cmp.caveats.iter().any(|c| c.contains("driver GPU")));

        cur.context.gpu_name = Some("GPU B".into());
        let cmp = compare(&cur, &prev);
        assert!(
            cmp.caveats
                .iter()
                .any(|c| c.contains("GPU testata diversa"))
        );
    }

    #[test]
    fn shared_warnings_are_not_context_differences() {
        let shared = "Temperatura CPU non osservabile: applicati limiti di durata conservativi";
        let mut prev = bench("p", BenchmarkKind::CpuSingleCore, T0);
        prev.warnings = vec![shared.into()];
        let mut cur = bench("c", BenchmarkKind::CpuSingleCore, T0 + 1);
        cur.warnings = vec![shared.into()];
        assert!(compare(&cur, &prev).caveats.is_empty());

        cur.warnings.push("Alimentazione a batteria".into());
        let caveats = compare(&cur, &prev).caveats;
        assert_eq!(caveats.len(), 1);
        assert!(caveats[0].contains("Solo l'esecuzione attuale"));
        assert!(caveats[0].contains("batteria"));
        assert!(!caveats[0].contains(shared));
    }

    #[test]
    fn context_differences_become_caveats() {
        let prev = bench("p", BenchmarkKind::CpuMultiCore, T0);
        let mut cur = bench("c", BenchmarkKind::CpuMultiCore, T0 + 1);
        cur.context.power_source = PowerSource::Battery;
        cur.context.power_plan = Some("Risparmio energia".into());
        cur.context.background_cpu_percent = avail(10.1, Unit::Percent, Source::Pdh);
        cur.context.cpu_limit_mhz = avail(3000.0, Unit::Megahertz, Source::PowerApi);
        cur.context.available_memory_bytes = avail(5.9 * GIB, Unit::Bytes, Source::Sysinfo);
        cur.warnings.push("In esecuzione a batteria".into());
        let cmp = compare(&cur, &prev);
        assert!(cmp.comparable);
        assert_eq!(cmp.caveats.len(), 6, "{:?}", cmp.caveats);
    }

    #[test]
    fn context_caveat_boundaries() {
        let prev = bench("p", BenchmarkKind::CpuMultiCore, T0);
        let mut cur = bench("c", BenchmarkKind::CpuMultiCore, T0 + 1);
        cur.context.background_cpu_percent = avail(10.0, Unit::Percent, Source::Pdh);
        cur.context.available_memory_bytes = avail(6.0 * GIB, Unit::Bytes, Source::Sysinfo); // exactly 25 %
        cur.context.cpu_limit_mhz = avail(4700.0 * 0.99, Unit::Megahertz, Source::PowerApi);
        assert!(compare(&cur, &prev).caveats.is_empty());
    }

    #[test]
    fn find_previous_picks_newest_older_comparable() {
        let cur = bench("c", BenchmarkKind::CpuMultiCore, T0 + 100);
        let newer = bench("n", BenchmarkKind::CpuMultiCore, T0 + 200);
        let mut other_params = bench("x", BenchmarkKind::CpuMultiCore, T0 + 90);
        other_params.request.duration_s = Some(60);
        let good = bench("g", BenchmarkKind::CpuMultiCore, T0 + 50);
        let older = bench("o", BenchmarkKind::CpuMultiCore, T0 + 10);
        let other_kind = bench("k", BenchmarkKind::CpuSingleCore, T0 + 80);
        let history = vec![newer, cur.clone(), other_params, other_kind, good, older];
        let found = find_previous_comparable(&cur, &history).expect("found");
        assert_eq!(found.run_id, "g");
        assert!(find_previous_comparable(&history[5], &history).is_none());
    }

    #[test]
    fn english_caveats() {
        let prev = bench("p", BenchmarkKind::CpuMultiCore, T0);
        let mut cur = bench("c", BenchmarkKind::CpuMultiCore, T0 + 1);
        cur.context.power_source = PowerSource::Battery;
        cur.context.background_cpu_percent = avail(10.1, Unit::Percent, Source::Pdh);
        let caveats = compare_in(Language::English, &cur, &prev).caveats;
        assert_eq!(
            caveats,
            vec![
                "Different power source: battery now, AC power before.".to_string(),
                "High background CPU load at the start of the current run: 10.1%.".to_string(),
            ]
        );
        // The Italian wrapper is unchanged.
        assert_eq!(
            compare(&cur, &prev),
            compare_in(Language::Italian, &cur, &prev)
        );

        let mut other = bench("o", BenchmarkKind::CpuSingleCore, T0 + 2);
        other.context.logical_processors = 4;
        let cmp = compare_in(Language::English, &other, &prev);
        assert!(!cmp.comparable);
        assert!(
            cmp.caveats
                .contains(&"Different benchmark type.".to_string())
        );
        assert!(
            cmp.caveats
                .iter()
                .any(|c| c.starts_with("Different number of logical processors (4 vs ")),
            "{:?}",
            cmp.caveats
        );
    }
}
