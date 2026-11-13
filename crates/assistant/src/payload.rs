//! The user-message content: one finding plus minimal context, redacted.
//!
//! What is included (and nothing else):
//! - the finding's text fields and evidence (the "Semplice" duplicate text is left out);
//! - the coverage note of the finding's component;
//! - OS name/version/build and CPU architecture (never the hostname or uptime);
//! - hardware model names: CPU brand, RAM total, non-virtual GPUs (name, vendor, VRAM,
//!   driver version) and, for storage findings, disk models and volumes (mount point, file
//!   system, sizes; never the volume label);
//! - for CPU/memory/process findings, up to `MAX_PROCESSES` accessible, non-system
//!   processes from the window ranking (pid, name, CPU %, memory) so `closeProgram` can
//!   reference a real pid. Executable paths are never included.
//!
//! Every string then goes through `Redactor` (device ids, paths, user names, hostname).
//! The output is pretty JSON: exactly what is shown in `FixPreview` and sent.

use serde::Serialize;

use models::component::ComponentKind;
use models::diagnostics::{CoverageItem, CoverageStatus, Finding};
use models::hardware::HardwareInventory;
use models::process::{ProcessCategory, ProcessSnapshot};
use models::settings::Language;
use models::system::SystemInfo;
use models::{AppError, Source};

use crate::redact::Redactor;

pub const MAX_PROCESSES: usize = 8;
const MAX_EVIDENCE: usize = 20;
const MAX_LIST: usize = 10;
const MAX_TEXT: usize = 1_000;
/// Hard cap on the payload, to keep cost predictable.
pub const MAX_PAYLOAD_BYTES: usize = 24_000;

/// Everything the payload may draw from. Only `finding` is required.
#[derive(Debug, Clone, Copy)]
pub struct PayloadInput<'a> {
    pub finding: &'a Finding,
    /// Coverage of the whole report; only the finding's component is used.
    pub coverage: &'a [CoverageItem],
    pub system: Option<&'a SystemInfo>,
    pub inventory: Option<&'a HardwareInventory>,
    pub processes: Option<&'a ProcessSnapshot>,
    /// Account names to scrub in addition to those detected in paths
    /// (typically `Redactor::current_user_names()`).
    pub user_names: &'a [String],
    /// Language of the user: the payload's own note, the preview texts and the plan the
    /// model is asked to write. The placeholders stay the same in every language.
    pub language: Language,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Payload {
    app: &'static str,
    finding: FindingOut,
    coverage: Option<CoverageOut>,
    system: Option<SystemOut>,
    hardware: Option<HardwareOut>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    volumes: Vec<VolumeOut>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    processes: Vec<ProcessOut>,
    privacy: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FindingOut {
    rule_id: String,
    title: String,
    component: ComponentKind,
    severity: models::diagnostics::Severity,
    kind: models::diagnostics::FindingKind,
    confidence: models::diagnostics::Confidence,
    description: String,
    evidence: Vec<EvidenceOut>,
    possible_causes: Vec<String>,
    recommendation: String,
    limitations: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct EvidenceOut {
    label: String,
    value: String,
    source: Source,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CoverageOut {
    status: CoverageStatus,
    note: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SystemOut {
    os_name: Option<String>,
    os_version: Option<String>,
    os_build: Option<String>,
    cpu_arch: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct HardwareOut {
    cpu: Option<String>,
    logical_cores: u32,
    memory_total: String,
    gpus: Vec<GpuOut>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    disks: Vec<DiskOut>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GpuOut {
    name: String,
    vendor: models::hardware::GpuVendor,
    /// `None`: the GPU has no dedicated memory of its own (unified memory, Apple
    /// silicon) or its size is not reported.
    dedicated_memory: Option<String>,
    driver_version: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DiskOut {
    model: Option<String>,
    media: models::hardware::MediaKind,
    bus: models::hardware::BusKind,
    size: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct VolumeOut {
    mount_point: String,
    file_system: Option<String>,
    total: String,
    available: String,
    removable: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProcessOut {
    pid: u32,
    name: String,
    category: ProcessCategory,
    cpu_percent: Option<f64>,
    memory: Option<String>,
}

fn privacy_note(lang: Language) -> &'static str {
    lang.pick(
        "Dati personali sostituiti: <utente>, <host>, <percorso>, <id dispositivo>.",
        "Personal data replaced: <utente>, <host>, <percorso>, <id dispositivo>.",
    )
}

fn clip(s: &str, max: usize) -> String {
    let t = s.trim();
    if t.chars().count() <= max {
        return t.to_owned();
    }
    let mut out: String = t.chars().take(max).collect();
    out.push('…');
    out
}

fn clip_list(v: &[String]) -> Vec<String> {
    v.iter().take(MAX_LIST).map(|s| clip(s, MAX_TEXT)).collect()
}

/// Binary units with one decimal, e.g. "15.8 GiB".
pub fn human_bytes(b: f64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut v = b.max(0.0);
    let mut i = 0;
    while v >= 1024.0 && i + 1 < UNITS.len() {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{v:.0} B")
    } else {
        format!("{v:.1} {}", UNITS[i])
    }
}

#[allow(clippy::cast_precision_loss)]
fn bytes_u64(b: u64) -> String {
    human_bytes(b as f64)
}

fn wants_processes(c: ComponentKind) -> bool {
    matches!(
        c,
        ComponentKind::Cpu | ComponentKind::Memory | ComponentKind::Processes
    )
}

fn build(input: &PayloadInput) -> Payload {
    let f = input.finding;
    let finding = FindingOut {
        rule_id: f.rule_id.clone(),
        title: clip(&f.title, MAX_TEXT),
        component: f.component,
        severity: f.severity,
        kind: f.kind,
        confidence: f.confidence,
        description: clip(&f.description, MAX_TEXT * 2),
        evidence: f
            .evidence
            .iter()
            .take(MAX_EVIDENCE)
            .map(|e| EvidenceOut {
                label: clip(&e.label, MAX_TEXT),
                value: clip(&e.value, MAX_TEXT),
                source: e.source,
            })
            .collect(),
        possible_causes: clip_list(&f.possible_causes),
        recommendation: clip(&f.recommendation, MAX_TEXT * 2),
        limitations: clip_list(&f.limitations),
    };
    let coverage = input
        .coverage
        .iter()
        .find(|c| c.component == f.component)
        .map(|c| CoverageOut {
            status: c.status,
            note: clip(&c.note, MAX_TEXT),
        });
    let system = input.system.map(|s| SystemOut {
        os_name: s.os_name.clone(),
        os_version: s.os_version.clone(),
        os_build: s.os_long_version.clone(),
        cpu_arch: s.cpu_arch.clone(),
    });
    let storage = f.component == ComponentKind::Storage;
    let hardware = input.inventory.map(|inv| HardwareOut {
        cpu: inv.cpu.brand.clone(),
        logical_cores: inv.cpu.logical_cores,
        memory_total: bytes_u64(inv.memory.total_bytes),
        gpus: inv
            .gpus
            .iter()
            .filter(|g| !g.is_software && !g.is_virtual)
            .take(4)
            .map(|g| GpuOut {
                name: g.name.clone(),
                vendor: g.vendor,
                dedicated_memory: g.dedicated_vram_bytes.value().map(|b| human_bytes(*b)),
                driver_version: g.driver_version.value().cloned(),
            })
            .collect(),
        disks: if storage {
            inv.physical_disks
                .value()
                .map(|d| {
                    d.iter()
                        .take(MAX_LIST)
                        .map(|d| DiskOut {
                            model: d.model.clone(),
                            media: d.media,
                            bus: d.bus,
                            size: bytes_u64(d.size_bytes),
                        })
                        .collect()
                })
                .unwrap_or_default()
        } else {
            Vec::new()
        },
    });
    let volumes = match input.inventory {
        Some(inv) if storage => inv
            .volumes
            .iter()
            .take(MAX_LIST)
            .map(|v| VolumeOut {
                mount_point: v.mount_point.clone(),
                file_system: v.file_system.clone(),
                total: bytes_u64(v.total_bytes),
                available: bytes_u64(v.available_bytes),
                removable: v.removable,
            })
            .collect(),
        _ => Vec::new(),
    };
    let processes = match input.processes {
        Some(p) if wants_processes(f.component) => p
            .ranking_window
            .iter()
            .filter(|r| r.accessible && !r.is_self && r.category != ProcessCategory::System)
            .take(MAX_PROCESSES)
            .map(|r| ProcessOut {
                pid: r.pid,
                name: clip(&r.name, 100),
                category: r.category,
                cpu_percent: r.cpu_percent.value().map(|v| (v * 10.0).round() / 10.0),
                memory: r.working_set_bytes.value().map(|b| human_bytes(*b)),
            })
            .collect(),
        _ => Vec::new(),
    };
    Payload {
        app: "PC Diagnostics",
        finding,
        coverage,
        system,
        hardware,
        volumes,
        processes,
        privacy: privacy_note(input.language),
    }
}

/// Builds the redacted, pretty-printed payload.
pub fn build_payload(input: &PayloadInput) -> Result<String, AppError> {
    let payload = build(input);
    let mut value = serde_json::to_value(&payload).map_err(|e| {
        AppError::internal("payload serialization failed").with_detail(e.to_string())
    })?;

    // Profile folder names anywhere in the source data, including fields that are never
    // sent (process executable paths), are scrubbed too.
    let mut sources: Vec<String> = Vec::new();
    collect_strings(&value, &mut sources);
    if let Some(p) = input.processes {
        for r in p.ranking_window.iter().chain(&p.ranking_instant) {
            if let Some(path) = &r.exe_path {
                sources.push(path.clone());
            }
        }
    }
    let redactor = Redactor::new(
        input.system.and_then(|s| s.hostname.as_deref()),
        input.user_names.iter().map(String::as_str),
        sources.iter().map(String::as_str),
    );
    redactor.scrub_value(&mut value);

    let text = serde_json::to_string_pretty(&value).map_err(|e| {
        AppError::internal("payload serialization failed").with_detail(e.to_string())
    })?;
    if text.len() > MAX_PAYLOAD_BYTES {
        return Err(AppError::invalid_input(
            "the data for this problem is too large for the assistant",
        )
        .with_detail(format!("{} bytes, at most {MAX_PAYLOAD_BYTES}", text.len())));
    }
    Ok(text)
}

fn collect_strings(v: &serde_json::Value, out: &mut Vec<String>) {
    match v {
        serde_json::Value::String(s) => out.push(s.clone()),
        serde_json::Value::Array(a) => a.iter().for_each(|x| collect_strings(x, out)),
        serde_json::Value::Object(o) => o.values().for_each(|x| collect_strings(x, out)),
        _ => {}
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use models::diagnostics::{Confidence, Evidence, FindingKind, Severity, SimpleExplanation};

    pub(crate) fn finding(component: ComponentKind) -> Finding {
        Finding {
            rule_id: "drivers.device.problem".into(),
            title: "Dispositivo con problemi: Cuffie di Mario".into(),
            component,
            severity: Severity::Warning,
            kind: FindingKind::Confirmed,
            confidence: Confidence::High,
            description: r"Il dispositivo USB\VID_046D&PID_0A8F\7&1F2E3D4C&0&3 segnala il codice 43 (driver C:\Users\Mario\AppData\Local\Temp\drv\usbaudio.sys)."
                .into(),
            evidence: vec![
                Evidence {
                    label: "Dispositivo".into(),
                    value: "Cuffie di Mario su DESKTOP-ABC123".into(),
                    source: Source::ConfigManager,
                },
                Evidence {
                    label: "Percorso".into(),
                    value: r"C:\Users\Mario\Documents\log.txt".into(),
                    source: Source::ConfigManager,
                },
            ],
            possible_causes: vec!["Driver danneggiato".into()],
            recommendation: "Aggiornare il driver".into(),
            limitations: vec![],
            simple: SimpleExplanation::default(),
        }
    }

    pub(crate) fn system() -> SystemInfo {
        SystemInfo {
            os_name: Some("Windows 11 Pro".into()),
            os_version: Some("24H2".into()),
            os_long_version: Some("10.0.26100".into()),
            kernel_version: None,
            hostname: Some("DESKTOP-ABC123".into()),
            cpu_arch: "x86_64".into(),
            build_arch: "x86_64".into(),
            boot_time_unix_s: Some(1),
            uptime_s: Some(2),
        }
    }

    #[test]
    fn payload_is_redacted() {
        let f = finding(ComponentKind::Drivers);
        let sys = system();
        let users = vec!["giulia".to_string()];
        let cov = [CoverageItem {
            component: ComponentKind::Drivers,
            status: CoverageStatus::Evaluated,
            note: "Dispositivi letti per giulia".into(),
        }];
        let text = build_payload(&PayloadInput {
            finding: &f,
            coverage: &cov,
            system: Some(&sys),
            inventory: None,
            processes: None,
            user_names: &users,
            language: Language::Italian,
        })
        .expect("payload");
        let lower = text.to_lowercase();
        for leaked in [
            "mario",
            "desktop-abc123",
            "giulia",
            r"c:\\users",
            "7&1f2e3d4c",
            "log.txt",
            "documents",
        ] {
            assert!(!lower.contains(leaked), "{leaked} leaked in:\n{text}");
        }
        assert!(text.contains("<utente>"));
        assert!(text.contains("<host>"));
        assert!(
            text.contains(r"USB\\VID_046D&PID_0A8F\\<id dispositivo>"),
            "{text}"
        );
        assert!(text.contains(r"<percorso>\\usbaudio.sys"), "{text}");
        assert!(text.contains("Windows 11 Pro"));
        assert!(!text.contains("uptime"));
        let v: serde_json::Value = serde_json::from_str(&text).expect("valid json");
        assert_eq!(v["finding"]["ruleId"], "drivers.device.problem");
        assert_eq!(v["coverage"]["status"], "evaluated");
        assert!(
            v["privacy"]
                .as_str()
                .is_some_and(|p| p.starts_with("Dati personali"))
        );
    }

    #[test]
    fn processes_only_for_relevant_components() {
        let f = finding(ComponentKind::Drivers);
        let text = build_payload(&PayloadInput {
            finding: &f,
            coverage: &[],
            system: None,
            inventory: None,
            processes: None,
            user_names: &[],
            language: Language::English,
        })
        .expect("payload");
        let v: serde_json::Value = serde_json::from_str(&text).expect("json");
        assert!(v.get("processes").is_none());
        assert!(v["system"].is_null());
        assert_eq!(
            v["privacy"],
            "Personal data replaced: <utente>, <host>, <percorso>, <id dispositivo>."
        );
    }

    #[test]
    fn human_bytes_formats() {
        assert_eq!(human_bytes(512.0), "512 B");
        assert_eq!(human_bytes(16.0 * 1024.0 * 1024.0 * 1024.0), "16.0 GiB");
    }
}
