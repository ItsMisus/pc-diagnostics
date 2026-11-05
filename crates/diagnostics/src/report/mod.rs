//! Report rendering (JSON and HTML) with optional redaction. Pure: returns strings; the
//! shell writes the file.

mod html;
mod redact;

use serde::{Deserialize, Serialize};

use models::AppError;
use models::benchmark::{BenchmarkKind, BenchmarkState};
use models::benchmark_run::BenchmarkResult;
use models::component::ComponentKind;
use models::diagnostics::{
    Confidence, CoverageStatus, DiagnosticReport, FindingKind, ReportFormat, Severity,
};
use models::drivers::{
    DeviceCategory, DeviceHealth, DriverInventory, DriverSummary, UpdateCheckInfo,
};
use models::hardware::HardwareInventory;
use models::process::{ProcessCategory, ProcessSnapshot};
use models::settings::Language;
use models::system::SystemInfo;

use crate::fmt;
use crate::window::val;

pub use redact::Redactor;

/// Identifies the export document format.
pub const REPORT_FORMAT_NAME: &str = "pc-diagnostics-report";
/// Bumped when the export document layout changes.
pub const REPORT_FORMAT_VERSION: u32 = 1;
/// Processes listed in a report (window ranking).
pub const REPORT_TOP_PROCESSES: usize = 10;

/// Everything a report may contain. Only `report` is required.
#[derive(Debug, Clone, Copy)]
pub struct ReportBundle<'a> {
    pub report: &'a DiagnosticReport,
    pub system: Option<&'a SystemInfo>,
    pub inventory: Option<&'a HardwareInventory>,
    pub benchmarks: &'a [BenchmarkResult],
    pub drivers: Option<&'a DriverInventory>,
    pub processes: Option<&'a ProcessSnapshot>,
    /// Local UTC offset at generation time, for local times in the HTML and file name.
    /// `None` = show UTC.
    pub utc_offset_minutes: Option<i32>,
    /// Language of the texts the export adds (headings, labels, dates). The findings keep
    /// the language they were generated in.
    pub language: Language,
}

/// One device with a problem. `instance_id` is omitted when redacted.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProblemDevice {
    pub name: String,
    pub category: DeviceCategory,
    pub manufacturer: Option<String>,
    pub health: DeviceHealth,
    pub problem_code: Option<u32>,
    pub problem_description: Option<String>,
    pub driver_version: Option<String>,
    pub driver_provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instance_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DriversSection {
    pub collected_at: u64,
    pub summary: DriverSummary,
    pub update_check: Option<UpdateCheckInfo>,
    pub problem_devices: Vec<ProblemDevice>,
}

/// A process of the window ranking: name and scores only, never the path.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessEntry {
    pub name: String,
    pub category: ProcessCategory,
    pub impact: f64,
    pub impact_peak: f64,
    pub cpu_percent: Option<f64>,
    pub working_set_bytes: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessesSection {
    pub at: u64,
    pub window_s: u32,
    pub methodology_version: u32,
    pub total_processes: u32,
    pub top: Vec<ProcessEntry>,
}

/// The exported document (JSON layout; HTML renders the same data).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportDocument {
    pub format: String,
    pub format_version: u32,
    pub redacted: bool,
    /// Language of the labels the export added (`it` / `en`).
    #[serde(default)]
    pub language: Language,
    pub generated_at_utc: String,
    /// Local UTC offset when the report was exported (times in the HTML use it).
    #[serde(default)]
    pub utc_offset_minutes: Option<i32>,
    pub report: DiagnosticReport,
    pub system: Option<SystemInfo>,
    pub hardware: Option<HardwareInventory>,
    pub benchmarks: Vec<BenchmarkResult>,
    pub drivers: Option<DriversSection>,
    pub processes: Option<ProcessesSection>,
}

fn internal(context: &str, e: impl std::fmt::Display) -> AppError {
    AppError::internal(format!("report rendering failed: {context}")).with_detail(e.to_string())
}

/// Builds the document, applying structural and string redaction when requested.
pub fn prepare(bundle: &ReportBundle, redact: bool) -> Result<ExportDocument, AppError> {
    let drivers = bundle.drivers.map(|d| DriversSection {
        collected_at: d.collected_at,
        summary: d.summary.clone(),
        update_check: d.update_check.clone(),
        problem_devices: d
            .devices
            .iter()
            .filter(|x| {
                matches!(
                    x.health,
                    DeviceHealth::ProblemDetected | DeviceHealth::DriverMissing
                )
            })
            .map(|x| ProblemDevice {
                name: x.name.clone(),
                category: x.category,
                manufacturer: x.manufacturer.clone(),
                health: x.health,
                problem_code: x.problem_code,
                problem_description: x.problem_description.clone(),
                driver_version: x.driver.as_ref().and_then(|v| v.version.clone()),
                driver_provider: x.driver.as_ref().and_then(|v| v.provider.clone()),
                instance_id: (!redact).then(|| x.instance_id.clone()),
            })
            .collect(),
    });
    let processes = bundle.processes.map(|p| ProcessesSection {
        at: p.at,
        window_s: p.window_s,
        methodology_version: p.methodology_version,
        total_processes: p.total_processes,
        top: p
            .ranking_window
            .iter()
            .take(REPORT_TOP_PROCESSES)
            .map(|r| ProcessEntry {
                name: r.name.clone(),
                category: r.category,
                impact: r.pressure.impact,
                impact_peak: r.impact_peak,
                cpu_percent: val(&r.cpu_percent),
                working_set_bytes: val(&r.working_set_bytes),
            })
            .collect(),
    });
    let mut system = bundle.system.cloned();
    let mut hardware = bundle.inventory.cloned();
    if redact {
        if let Some(s) = system.as_mut() {
            s.hostname = None;
        }
        if let Some(h) = hardware.as_mut() {
            for v in &mut h.volumes {
                v.label = None;
            }
        }
    }
    let doc = ExportDocument {
        format: REPORT_FORMAT_NAME.into(),
        format_version: REPORT_FORMAT_VERSION,
        redacted: redact,
        language: bundle.language,
        generated_at_utc: fmt::iso_utc(bundle.report.generated_at),
        utc_offset_minutes: bundle.utc_offset_minutes,
        report: bundle.report.clone(),
        system,
        hardware,
        benchmarks: bundle.benchmarks.to_vec(),
        drivers,
        processes,
    };
    if !redact {
        return Ok(doc);
    }
    let redactor = Redactor::from_bundle(bundle);
    let mut value = serde_json::to_value(&doc).map_err(|e| internal("serialize", e))?;
    redactor.scrub_value(&mut value);
    serde_json::from_value(value).map_err(|e| internal("redact", e))
}

pub fn render_json(bundle: &ReportBundle, redact: bool) -> Result<String, AppError> {
    let doc = prepare(bundle, redact)?;
    serde_json::to_string_pretty(&doc).map_err(|e| internal("json", e))
}

pub fn render_html(bundle: &ReportBundle, redact: bool) -> Result<String, AppError> {
    let doc = prepare(bundle, redact)?;
    Ok(html::render(&doc))
}

/// `pc-diagnostics-2026-11-04-1021.html`: local time of the report when the offset is
/// known (UTC otherwise), no hostname.
pub fn report_file_name(
    report: &DiagnosticReport,
    format: ReportFormat,
    utc_offset_minutes: Option<i32>,
) -> String {
    let (y, mo, d, h, mi, _) = fmt::civil_utc(fmt::shift(report.generated_at, utc_offset_minutes));
    let ext = match format {
        ReportFormat::Json => "json",
        ReportFormat::Html => "html",
    };
    format!("pc-diagnostics-{y:04}-{mo:02}-{d:02}-{h:02}{mi:02}.{ext}")
}

pub fn kind_label(lang: Language, k: BenchmarkKind) -> &'static str {
    match k {
        BenchmarkKind::CpuSingleCore => "CPU single-core",
        BenchmarkKind::CpuMultiCore => "CPU multi-core",
        BenchmarkKind::CpuSustained => {
            lang.pick("CPU sotto carico prolungato", "CPU under sustained load")
        }
        BenchmarkKind::MemoryBandwidth => lang.pick("Banda della memoria", "Memory bandwidth"),
        BenchmarkKind::MemoryLatency => lang.pick("Latenza della memoria", "Memory latency"),
        BenchmarkKind::StorageSequential => lang.pick("Disco sequenziale", "Sequential disk"),
        BenchmarkKind::StorageRandom => lang.pick("Disco casuale", "Random disk"),
        BenchmarkKind::GpuCompute => lang.pick("GPU calcolo", "GPU compute"),
    }
}

pub fn state_label(lang: Language, s: BenchmarkState) -> &'static str {
    match s {
        BenchmarkState::Idle => lang.pick("inattivo", "idle"),
        BenchmarkState::Preparing => lang.pick("in preparazione", "preparing"),
        BenchmarkState::Running => lang.pick("in esecuzione", "running"),
        BenchmarkState::Cancelling => lang.pick("annullamento in corso", "cancelling"),
        BenchmarkState::Completed => lang.pick("completato", "completed"),
        BenchmarkState::Cancelled => lang.pick("annullato", "cancelled"),
        BenchmarkState::Failed => lang.pick("non riuscito", "failed"),
    }
}

pub fn severity_label(lang: Language, s: Severity) -> &'static str {
    match s {
        Severity::Critical => lang.pick("Critico", "Critical"),
        Severity::Warning => lang.pick("Avviso", "Warning"),
        Severity::Info => lang.pick("Informazione", "Info"),
    }
}

pub fn finding_kind_label(lang: Language, k: FindingKind) -> &'static str {
    match k {
        FindingKind::Observation => lang.pick("Osservazione", "Observation"),
        FindingKind::Hypothesis => lang.pick("Ipotesi", "Hypothesis"),
        FindingKind::Confirmed => lang.pick("Confermato", "Confirmed"),
    }
}

pub fn confidence_label(lang: Language, c: Confidence) -> &'static str {
    match c {
        Confidence::High => lang.pick("alta", "high"),
        Confidence::Medium => lang.pick("media", "medium"),
        Confidence::Low => lang.pick("bassa", "low"),
    }
}

pub fn coverage_label(lang: Language, c: CoverageStatus) -> &'static str {
    match c {
        CoverageStatus::Evaluated => lang.pick("Valutato", "Evaluated"),
        CoverageStatus::Partial => lang.pick("Parziale", "Partial"),
        CoverageStatus::NotEvaluated => lang.pick("Non valutato", "Not evaluated"),
    }
}

pub fn component_label(lang: Language, c: ComponentKind) -> &'static str {
    match c {
        ComponentKind::Cpu => "CPU",
        ComponentKind::Gpu => "GPU",
        ComponentKind::Memory => lang.pick("Memoria", "Memory"),
        ComponentKind::Storage => lang.pick("Archiviazione", "Storage"),
        ComponentKind::Drivers => lang.pick("Driver e dispositivi", "Drivers and devices"),
        ComponentKind::Processes => lang.pick("Processi", "Processes"),
        ComponentKind::System => lang.pick("Sistema", "System"),
    }
}

#[cfg(test)]
mod tests;
