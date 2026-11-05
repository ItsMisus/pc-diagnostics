//! Process rules (Impact Score, docs/impact-score.md).

use models::Source;
use models::component::ComponentKind;
use models::diagnostics::{Confidence, FindingKind, Severity, SimpleExplanation};
use models::process::{IMPACT_METHODOLOGY_VERSION, ProcessRow};
use models::settings::Language;

use super::{Cov, Ctx, Outcome, ev, finding, simple, strings};
use crate::fmt;
use crate::window::SHORT_WINDOW_S;

/// Mean Impact Score over the process window considered "high".
pub const IMPACT_HIGH: f64 = 50.0;
/// Processes listed in the finding.
pub const IMPACT_LISTED_MAX: usize = 5;

pub fn impact_disclaimer(lang: Language) -> &'static str {
    lang.pick(
        "L'Impact Score stima la pressione sulle risorse; non è una prova che il processo rallenti il computer.",
        "The Impact Score estimates resource pressure; it is not proof that the process slows the computer down.",
    )
}

pub fn evaluate(ctx: &Ctx) -> Outcome {
    let mut out = Outcome::default();
    let mut cov = Cov::new(ComponentKind::Processes);
    let lang = ctx.lang;
    let Some(snap) = ctx.input.processes else {
        cov.not_evaluated(lang.pick(
            "analisi dei processi non disponibile",
            "process analysis not available",
        ));
        out.coverage.push(cov.finish());
        return out;
    };
    let w = fmt::seconds(snap.window_s);
    if snap.window_s < SHORT_WINDOW_S {
        cov.partial(lang.pick(
            format!("finestra di analisi dei processi breve ({w})"),
            format!("short process analysis window ({w})"),
        ));
    }
    if snap.methodology_version != IMPACT_METHODOLOGY_VERSION {
        let v = snap.methodology_version;
        cov.partial(lang.pick(
            format!("metodologia Impact Score v{v} diversa da quella attesa (v{IMPACT_METHODOLOGY_VERSION})"),
            format!("Impact Score methodology v{v} differs from the expected one (v{IMPACT_METHODOLOGY_VERSION})"),
        ));
    }
    let (total, inaccessible) = (snap.total_processes, snap.inaccessible_processes);
    cov.note(lang.pick(
        format!("{total} processi analizzati, {inaccessible} con percorso non leggibile (metriche comunque misurate)"),
        format!("{total} processes analyzed, {inaccessible} with an unreadable path (metrics measured anyway)"),
    ));

    let mut high: Vec<&ProcessRow> = snap
        .ranking_window
        .iter()
        .filter(|r| r.pressure.impact.is_finite() && r.pressure.impact >= IMPACT_HIGH)
        .collect();
    high.sort_by(|a, b| {
        b.pressure
            .impact
            .total_cmp(&a.pressure.impact)
            .then_with(|| a.name.cmp(&b.name))
    });
    if !high.is_empty() {
        let names: Vec<&str> = high
            .iter()
            .take(IMPACT_LISTED_MAX)
            .map(|r| r.name.as_str())
            .collect();
        let (n, threshold, listed) = (high.len(), fmt::num(lang, IMPACT_HIGH, 0), names.join(", "));
        let disclaimer = impact_disclaimer(lang);
        let mut f = finding(
            "processes.impact.high",
            ComponentKind::Processes,
            Severity::Info,
            FindingKind::Observation,
            Confidence::Medium,
            lang.pick(
                "Processi con Impact Score elevato",
                "Processes with a high Impact Score",
            ),
            lang.pick(
                format!("{n} processi hanno un Impact Score medio pari o superiore a {threshold} nella finestra di {w}: {listed}. {disclaimer}"),
                format!(
                    "{n} {} an average Impact Score at or above {threshold} in the {w} window: {listed}. {disclaimer}",
                    if n == 1 { "process has" } else { "processes have" }
                ),
            ),
            impact_simple(lang, &high, &names),
        );
        f.evidence = high
            .iter()
            .take(IMPACT_LISTED_MAX)
            .map(|r| {
                let name = &r.name;
                let n1 = |v: f64| fmt::num(lang, v, 1);
                let (mean, peak, cpu, ram, io) = (
                    n1(r.pressure.impact),
                    n1(r.impact_peak),
                    n1(r.pressure.cpu),
                    n1(r.pressure.ram),
                    n1(r.pressure.io),
                );
                ev(
                    lang.pick(
                        format!("Impact Score di {name}"),
                        format!("Impact Score of {name}"),
                    ),
                    lang.pick(
                        format!("medio {mean}, picco {peak} (CPU {cpu}, RAM {ram}, I/O {io})"),
                        format!("average {mean}, peak {peak} (CPU {cpu}, RAM {ram}, I/O {io})"),
                    ),
                    Source::Kernel,
                )
            })
            .collect();
        f.possible_causes = strings(&[lang.pick(
            "Programmi che svolgono lavoro intenso su CPU, memoria o disco",
            "Programs doing heavy work on the CPU, memory or disk",
        )]);
        f.recommendation = lang.pick(
            "Se il computer è lento mentre questi processi sono attivi, verificare se il loro lavoro è atteso. Non terminare processi di sistema.",
            "If the computer is slow while these processes are active, check whether their work is expected. Do not end system processes.",
        ).into();
        f.limitations = strings(&[disclaimer]);
        if high.iter().any(|r| r.is_self) {
            f.limitations.push(
                lang.pick(
                    "L'elenco include processi di questa applicazione (PC Diagnostics).",
                    "The list includes processes of this application (PC Diagnostics).",
                )
                .into(),
            );
        }
        out.findings.push(f);
    }
    out.coverage.push(cov.finish());
    out
}

/// Plain-language text for `processes.impact.high`, with singular/plural agreement.
fn impact_simple(lang: Language, high: &[&ProcessRow], names: &[&str]) -> SimpleExplanation {
    let has_self = high.iter().any(|r| r.is_self);
    let self_note = if has_self {
        lang.pick(
            " Nell'elenco c'è anche PC Diagnostics, cioè questa applicazione.",
            " The list also includes PC Diagnostics, which is this app.",
        )
    } else {
        ""
    };
    let action = lang.pick(
        "Se il PC è lento mentre questi programmi sono aperti, controlla se ti servono in quel momento e chiudi quelli che non stai usando. Non chiudere i programmi di sistema di Windows.",
        "If the PC is slow while these programs are open, check whether you need them at that moment and close the ones you aren't using. Don't close Windows system programs.",
    );
    if let ([one], 1) = (names, high.len()) {
        let self_one = if has_self {
            lang.pick(
                " È questa stessa applicazione (PC Diagnostics).",
                " It's this app itself (PC Diagnostics).",
            )
        } else {
            ""
        };
        return simple(
            lang.pick(
                format!("Un programma sta usando molte risorse del PC: {one}"),
                format!("A program is using a lot of the PC's resources: {one}"),
            ),
            lang.pick(
                format!("Il programma «{one}» ha usato molto processore, memoria o disco nell'ultimo periodo analizzato. Non è una prova che rallenti il PC: è solo una stima di quanto lavoro sta facendo.{self_one}"),
                format!("The program “{one}” used a lot of processor, memory, or disk in the last period analyzed. This isn't proof that it slows the PC down: it's only an estimate of how much work it's doing.{self_one}"),
            ),
            lang.pick(
                "Se il PC è lento mentre questo programma è aperto, controlla se ti serve in quel momento e chiudilo se non lo stai usando. Non chiudere i programmi di sistema di Windows.",
                "If the PC is slow while this program is open, check whether you need it at that moment and close it if you aren't using it. Don't close Windows system programs.",
            ),
        );
    }
    let listed = if high.len() > names.len() {
        fmt::join_names_and_more(lang, names, high.len() - names.len())
    } else {
        fmt::join_names(lang, names)
    };
    let n = high.len();
    simple(
        lang.pick(
            format!("{n} programmi stanno usando molte risorse del PC"),
            format!("{n} programs are using a lot of the PC's resources"),
        ),
        lang.pick(
            format!("Questi programmi hanno usato molto processore, memoria o disco nell'ultimo periodo analizzato: {listed}. Non è una prova che rallentino il PC: è solo una stima di quanto lavoro stanno facendo.{self_note}"),
            format!("These programs used a lot of processor, memory, or disk in the last period analyzed: {listed}. This isn't proof that they slow the PC down: it's only an estimate of how much work they're doing.{self_note}"),
        ),
        action,
    )
}

#[cfg(test)]
mod tests {
    use crate::evaluate;
    use crate::testdata::*;
    use models::component::ComponentKind;
    use models::diagnostics::CoverageStatus;

    #[test]
    fn high_impact_boundary_and_disclaimer() {
        let snap = snapshot(vec![
            process("heavy.exe", 50.0, 2.0, 50.0),
            process("light.exe", 5.0, 0.1, 49.9),
        ]);
        let mut inp = input(&[]);
        inp.processes = Some(&snap);
        let r = evaluate(&inp);
        let f = r
            .findings
            .iter()
            .find(|f| f.rule_id == "processes.impact.high")
            .expect("finding");
        assert!(f.description.contains("heavy.exe"));
        assert!(!f.description.contains("light.exe"));
        assert!(f.description.contains("non è una prova"));
        assert!(!serde_json::to_string(f).expect("json").contains("AppData"));
    }

    #[test]
    fn no_high_impact_no_finding_and_coverage() {
        let snap = snapshot(vec![process("a.exe", 1.0, 0.1, 10.0)]);
        let mut inp = input(&[]);
        inp.processes = Some(&snap);
        let r = evaluate(&inp);
        assert!(
            !r.findings
                .iter()
                .any(|f| f.rule_id == "processes.impact.high")
        );
        let c = r
            .coverage
            .iter()
            .find(|c| c.component == ComponentKind::Processes)
            .expect("cov");
        assert_eq!(c.status, CoverageStatus::Evaluated);

        let mut short = snapshot(vec![]);
        short.window_s = 10;
        inp.processes = Some(&short);
        let r = evaluate(&inp);
        let c = r
            .coverage
            .iter()
            .find(|c| c.component == ComponentKind::Processes)
            .expect("cov");
        assert_eq!(c.status, CoverageStatus::Partial);
    }

    #[test]
    fn english_texts() {
        let snap = snapshot(vec![process("heavy.exe", 50.0, 2.0, 60.0)]);
        let mut inp = input(&[]);
        inp.processes = Some(&snap);
        inp.language = models::settings::Language::English;
        let r = evaluate(&inp);
        let f = r
            .findings
            .iter()
            .find(|f| f.rule_id == "processes.impact.high")
            .expect("finding");
        assert_eq!(f.title, "Processes with a high Impact Score");
        assert!(
            f.description
                .starts_with("1 process has an average Impact Score at or above 50 in the "),
            "{}",
            f.description
        );
        assert_eq!(
            f.simple.headline,
            "A program is using a lot of the PC's resources: heavy.exe"
        );
        assert_eq!(
            f.limitations[0],
            super::impact_disclaimer(models::settings::Language::English)
        );
        let cov = r
            .coverage
            .iter()
            .find(|c| c.component == ComponentKind::Processes)
            .expect("cov");
        assert!(cov.note.contains(" processes analyzed, "), "{}", cov.note);
    }
}
