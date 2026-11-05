//! Diagnostic finding contract, produced by the deterministic rule engine (phase 6).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::component::ComponentKind;
use crate::metric::Source;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Severity {
    Info,
    Warning,
    Critical,
}

/// Epistemic status of a finding. The UI must render these differently.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum FindingKind {
    /// A measured fact, no interpretation.
    Observation,
    /// A plausible explanation consistent with the evidence, not proven.
    Hypothesis,
    /// Confirmed by an authoritative signal (e.g. Windows device problem code).
    Confirmed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Confidence {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Evidence {
    pub label: String,
    /// Already formatted with unit, e.g. "1.2 GiB available of 16 GiB".
    pub value: String,
    pub source: Source,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Finding {
    /// Stable rule identifier, e.g. `memory.pressure.sustained`.
    pub rule_id: String,
    pub title: String,
    pub component: ComponentKind,
    pub severity: Severity,
    pub kind: FindingKind,
    pub confidence: Confidence,
    pub description: String,
    pub evidence: Vec<Evidence>,
    pub possible_causes: Vec<String>,
    pub recommendation: String,
    pub limitations: Vec<String>,
    /// Plain-language version for the "Semplice" view: no jargon, no numbers without
    /// context, written for someone who is not technical. Empty in reports saved before
    /// it existed; the UI then falls back to `title`/`description`/`recommendation`.
    #[serde(default)]
    pub simple: SimpleExplanation,
}

/// A finding explained for non-technical users. Same facts as the technical text, never
/// stronger claims (a hypothesis stays "potrebbe").
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SimpleExplanation {
    /// One short sentence: what is happening, e.g. "Il PC sta finendo la memoria".
    pub headline: String,
    /// Why it matters for the user, in everyday terms.
    pub meaning: String,
    /// What the user can do now (or "Nessuna azione necessaria").
    pub action: String,
}

/// How thoroughly a component could be evaluated. Missing data yields a partial diagnosis,
/// never a misleading "all good".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum CoverageStatus {
    Evaluated,
    Partial,
    NotEvaluated,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CoverageItem {
    pub component: ComponentKind,
    pub status: CoverageStatus,
    /// What was (or could not be) checked, e.g. "temperatura CPU non disponibile".
    pub note: String,
}

/// Output of the deterministic rule engine. There is deliberately no global health score:
/// each component has its own findings and coverage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DiagnosticReport {
    pub id: String,
    #[ts(type = "number")]
    pub generated_at: u64,
    pub app_version: String,
    /// Rule set version; bump when rules change.
    pub rules_version: u32,
    /// Seconds of live monitoring history the rules could look at.
    pub observed_window_s: u32,
    /// Most severe first.
    pub findings: Vec<Finding>,
    pub coverage: Vec<CoverageItem>,
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MetricDelta {
    pub id: String,
    pub label: String,
    pub unit: crate::metric::Unit,
    pub current: f64,
    pub previous: f64,
    /// (current - previous) / previous * 100. `None` if previous is 0.
    pub change_percent: Option<f64>,
    pub higher_is_better: bool,
}

/// Comparison of two benchmark runs. `deltas` is empty when the runs are not comparable.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BenchmarkComparison {
    pub current_run_id: String,
    pub previous_run_id: String,
    pub comparable: bool,
    /// Why not comparable, or context differences that weaken the comparison
    /// (power source/plan, background load, frequency limit, available memory).
    pub caveats: Vec<String>,
    pub deltas: Vec<MetricDelta>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ReportFormat {
    Json,
    Html,
}

/// Result of writing a report file to the user's Documents folder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ExportedReport {
    pub path: String,
    #[ts(type = "number")]
    pub bytes: u64,
    /// Personal data (hostname, paths, device instance ids) removed.
    pub redacted: bool,
}
