//! Deterministic rules. Each module evaluates one component and returns its findings,
//! its coverage item and report-level limitations. Thresholds are `pub const` and
//! documented in docs/diagnostics-rules.md.
//!
//! Every text exists in Italian and English, written next to each other with
//! `lang.pick("…", "…")` (the language comes from `Ctx::lang`). Both texts carry the same
//! facts; only the wording and number formatting (`fmt`) change.

pub mod cpu;
pub mod drivers;
pub mod gpu;
pub mod memory;
pub mod processes;
pub mod storage;
pub mod system;

use models::Source;
use models::component::ComponentKind;
use models::diagnostics::{
    Confidence, CoverageItem, CoverageStatus, Evidence, Finding, FindingKind, Severity,
    SimpleExplanation,
};
use models::process::{ProcessRow, ProcessSnapshot};
use models::settings::Language;

use crate::DiagnosticInput;
use crate::window::{self, val};

/// Shared evaluation context.
pub struct Ctx<'a, 'b> {
    pub input: &'b DiagnosticInput<'a>,
    pub window_s: u32,
    /// Language of every text the rules produce (`input.language`).
    pub lang: Language,
}

#[derive(Default)]
pub struct Outcome {
    pub findings: Vec<Finding>,
    pub coverage: Vec<CoverageItem>,
    pub limitations: Vec<String>,
}

impl Outcome {
    pub fn extend(&mut self, other: Outcome) {
        self.findings.extend(other.findings);
        self.coverage.extend(other.coverage);
        self.limitations.extend(other.limitations);
    }
}

/// Builds a finding. `simple` (the plain-language text for the "Semplice" view) is a
/// required argument so that no rule can forget it; see "Plain-language text" in
/// docs/diagnostics-rules.md for the style rules.
#[allow(clippy::too_many_arguments)] // positional on purpose: every call reads like the finding
pub fn finding(
    rule_id: &str,
    component: ComponentKind,
    severity: Severity,
    kind: FindingKind,
    confidence: Confidence,
    title: impl Into<String>,
    description: impl Into<String>,
    simple: SimpleExplanation,
) -> Finding {
    debug_assert!(
        !simple.headline.is_empty() && !simple.meaning.is_empty() && !simple.action.is_empty(),
        "{rule_id}: incomplete plain-language text"
    );
    Finding {
        rule_id: rule_id.into(),
        title: title.into(),
        component,
        severity,
        kind,
        confidence,
        description: description.into(),
        evidence: Vec::new(),
        possible_causes: Vec::new(),
        recommendation: String::new(),
        limitations: Vec::new(),
        simple,
    }
}

/// Plain-language explanation for non-technical users (see `finding`).
pub fn simple(
    headline: impl Into<String>,
    meaning: impl Into<String>,
    action: impl Into<String>,
) -> SimpleExplanation {
    SimpleExplanation {
        headline: headline.into(),
        meaning: meaning.into(),
        action: action.into(),
    }
}

/// Plain-language action for findings that need nothing from the user.
pub fn no_action(lang: Language) -> &'static str {
    lang.pick("Nessuna azione necessaria.", "No action needed.")
}

pub fn ev(label: impl Into<String>, value: impl Into<String>, source: Source) -> Evidence {
    Evidence {
        label: label.into(),
        value: value.into(),
        source,
    }
}

pub fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| (*s).to_string()).collect()
}

/// Accumulates coverage notes; the status only ever degrades.
pub struct Cov {
    component: ComponentKind,
    status: CoverageStatus,
    notes: Vec<String>,
}

impl Cov {
    pub fn new(component: ComponentKind) -> Self {
        Self {
            component,
            status: CoverageStatus::Evaluated,
            notes: Vec::new(),
        }
    }

    pub fn note(&mut self, note: impl Into<String>) {
        self.notes.push(note.into());
    }

    pub fn partial(&mut self, note: impl Into<String>) {
        self.degrade();
        self.note(note);
    }

    /// `partial` without a note, for a reason the report states once elsewhere.
    pub fn degrade(&mut self) {
        if self.status == CoverageStatus::Evaluated {
            self.status = CoverageStatus::Partial;
        }
    }

    pub fn not_evaluated(&mut self, note: impl Into<String>) {
        self.status = CoverageStatus::NotEvaluated;
        self.note(note);
    }

    pub fn finish(self) -> CoverageItem {
        CoverageItem {
            component: self.component,
            status: self.status,
            note: self.notes.join("; "),
        }
    }
}

/// Degrades a live-window based component to `partial` when the window is shorter than the
/// sustained rules need. No note: the window and its minimum are stated once for the whole
/// report (observed window, export header), not repeated in every component.
pub fn window_notes(cov: &mut Cov, window_s: u32) {
    if window_s < window::SUSTAINED_MIN_WINDOW_S {
        cov.degrade();
    }
}

/// Coverage note when the live history is empty (shared by the live-window rules).
pub fn no_samples(lang: Language) -> &'static str {
    lang.pick(
        "nessun campione di monitoraggio disponibile",
        "no monitoring samples available",
    )
}

/// "{hits} su {available}" / "{hits} of {available}".
pub fn count_of(lang: Language, part: usize, whole: usize) -> String {
    lang.pick(format!("{part} su {whole}"), format!("{part} of {whole}"))
}

/// Up to `n` processes of the window ranking with the largest working set (name only).
pub fn top_ram_processes(snapshot: Option<&ProcessSnapshot>, n: usize) -> Vec<&ProcessRow> {
    let Some(s) = snapshot else {
        return Vec::new();
    };
    let mut rows: Vec<(&ProcessRow, f64)> = s
        .ranking_window
        .iter()
        .filter_map(|r| val(&r.working_set_bytes).map(|w| (r, w)))
        .collect();
    rows.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.name.cmp(&b.0.name)));
    rows.into_iter().take(n).map(|(r, _)| r).collect()
}

/// Kind rank used by the report ordering: confirmed first.
pub fn kind_rank(k: FindingKind) -> u8 {
    match k {
        FindingKind::Confirmed => 0,
        FindingKind::Hypothesis => 1,
        FindingKind::Observation => 2,
    }
}
