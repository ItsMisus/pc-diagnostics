use super::*;
use crate::testdata::*;
use crate::{DiagnosticInput, evaluate};
use models::diagnostics::ReportFormat;
use models::drivers::{DeviceCategory, DeviceHealth};
use models::{AppError, ErrorCode, Source, Unit};

struct Fixture {
    system: SystemInfo,
    inventory: HardwareInventory,
    drivers: DriverInventory,
    processes: ProcessSnapshot,
    benchmarks: Vec<BenchmarkResult>,
    report: DiagnosticReport,
}

const HOST: &str = "DESKTOP-SECRET1";
const USER: &str = "mario.rossi";
const INSTANCE: &str = "USB\\VID_1234&PID_5678\\SERIALXYZ6";

fn fixture() -> Fixture {
    let system = system();
    let mut inventory = inventory();
    inventory.volumes.push(volume("D:\\", 100.0, 1.0));
    let drivers = drivers(
        vec![
            device(
                "Webcam",
                DeviceCategory::Other,
                DeviceHealth::ProblemDetected,
            ),
            device("Mouse", DeviceCategory::Input, DeviceHealth::Working),
        ],
        true,
    );
    let mut evil = process("<script>alert(1)</script>.exe", 40.0, 2.0, 60.0);
    evil.exe_path = Some(format!("C:\\Users\\{USER}\\Downloads\\evil.exe"));
    let mut rows = vec![evil];
    for i in 0..12 {
        rows.push(process(
            &format!("p{i:02}.exe"),
            1.0,
            0.1,
            10.0 - i as f64 * 0.5,
        ));
    }
    let processes = snapshot(rows);
    let mut b = bench(
        "run-1",
        models::benchmark::BenchmarkKind::StorageSequential,
        T0,
    );
    b.request.storage_volume = Some("D:\\".into());
    b.warnings.push(format!(
        "File di test in C:\\Users\\{USER}\\AppData\\Local\\Temp su {HOST}"
    ));
    b.error = Some(
        AppError::new(ErrorCode::Platform, "write failed").with_detail(format!(
            "path c:\\users\\{USER}\\x; host {HOST}; user {USER}"
        )),
    );
    let benchmarks = vec![b];
    let samples = live(60, |_, s| {
        s.cpu.usage_percent = avail(99.0, Unit::Percent, Source::Pdh);
    });
    let input = DiagnosticInput {
        app_version: "0.1.0",
        now_ms: T0,
        system: Some(&system),
        inventory: Some(&inventory),
        live: &samples,
        processes: Some(&processes),
        drivers: Some(&drivers),
        benchmarks: &benchmarks,
        language: Language::Italian,
    };
    let report = evaluate(&input);
    Fixture {
        system,
        inventory,
        drivers,
        processes,
        benchmarks,
        report,
    }
}

fn bundle(f: &Fixture) -> ReportBundle<'_> {
    ReportBundle {
        report: &f.report,
        system: Some(&f.system),
        inventory: Some(&f.inventory),
        benchmarks: &f.benchmarks,
        drivers: Some(&f.drivers),
        processes: Some(&f.processes),
        utc_offset_minutes: None,
        language: Language::Italian,
    }
}

fn assert_clean(out: &str) {
    let lower = out.to_ascii_lowercase();
    for secret in [
        HOST,
        USER,
        "SERIALXYZ",
        "VID_1234&PID_5678\\",
        "Personal Stuff",
        "AppData\\Local\\p",
    ] {
        assert!(
            !lower.contains(&secret.to_ascii_lowercase()),
            "leaked {secret:?}"
        );
    }
    // JSON escapes backslashes; check that form too.
    assert!(!lower.contains("serialxyz"));
}

#[test]
fn json_roundtrips_main_fields() {
    let f = fixture();
    let json = render_json(&bundle(&f), false).expect("json");
    let doc: ExportDocument = serde_json::from_str(&json).expect("parse");
    assert_eq!(doc.format, REPORT_FORMAT_NAME);
    assert_eq!(doc.format_version, REPORT_FORMAT_VERSION);
    assert!(!doc.redacted);
    assert_eq!(doc.report, f.report);
    assert_eq!(doc.system.as_ref(), Some(&f.system));
    assert_eq!(doc.hardware.as_ref(), Some(&f.inventory));
    assert_eq!(doc.benchmarks, f.benchmarks);
    let d = doc.drivers.expect("drivers");
    assert_eq!(d.problem_devices.len(), 1, "problem devices only");
    assert_eq!(d.problem_devices[0].instance_id.as_deref(), Some(INSTANCE));
    let p = doc.processes.expect("processes");
    assert_eq!(p.top.len(), REPORT_TOP_PROCESSES);
    assert_eq!(doc.generated_at_utc, "2026-11-04T01:21:00Z");
    // Never paths, even unredacted.
    assert!(!json.contains("Downloads"));
}

#[test]
fn redacted_json_has_no_personal_data() {
    let f = fixture();
    let json = render_json(&bundle(&f), true).expect("json");
    assert_clean(&json);
    let doc: ExportDocument = serde_json::from_str(&json).expect("parse");
    assert!(doc.redacted);
    assert_eq!(doc.system.as_ref().and_then(|s| s.hostname.clone()), None);
    assert!(
        doc.hardware
            .as_ref()
            .expect("hw")
            .volumes
            .iter()
            .all(|v| v.label.is_none())
    );
    let d = doc.drivers.expect("drivers");
    assert_eq!(d.problem_devices[0].instance_id, None);
    // Model names and mount points are kept.
    assert!(json.contains("Test CPU 8-Core"));
    assert!(json.contains("Test NVMe 1TB"));
    assert!(json.contains("D:\\\\"));
    assert!(json.contains("Users\\\\<utente>"));
    // Findings survive redaction.
    assert_eq!(doc.report.findings.len(), f.report.findings.len());
}

#[test]
fn redacted_html_has_no_personal_data() {
    let f = fixture();
    let html = render_html(&bundle(&f), true).expect("html");
    assert_clean(&html);
    assert!(html.contains("Users\\&lt;utente&gt;"));
    assert!(html.contains("Test CPU 8-Core"));
}

#[test]
fn unredacted_html_contains_hostname() {
    let f = fixture();
    let html = render_html(&bundle(&f), false).expect("html");
    assert!(html.contains(HOST));
    assert!(html.contains("SERIALXYZ6"));
}

#[test]
fn html_escapes_injection_and_is_self_contained() {
    let f = fixture();
    let html = render_html(&bundle(&f), false).expect("html");
    assert!(!html.contains("<script"));
    assert!(html.contains("&lt;script&gt;alert(1)&lt;/script&gt;.exe"));
    let lower = html.to_ascii_lowercase();
    for banned in [
        "<script", "<link", "<img", "<iframe", "src=", "href=", "http://", "https://", "@import",
        "url(",
    ] {
        assert!(!lower.contains(banned), "found {banned}");
    }
    assert!(html.starts_with("<!DOCTYPE html>"));
    assert!(html.contains("prefers-color-scheme:dark"));
    // Spec content.
    for section in [
        "Copertura",
        "Risultati",
        "Configurazione hardware",
        "Test eseguiti",
        "Driver e dispositivi",
        "Processi",
        "Limitazioni del risultato",
    ] {
        assert!(html.contains(section), "{section}");
    }
    assert!(html.contains("Osservazione") || html.contains("Confermato"));
}

#[test]
fn minimal_bundle_renders() {
    let report = evaluate(&input(&[]));
    let b = ReportBundle {
        report: &report,
        system: None,
        inventory: None,
        benchmarks: &[],
        drivers: None,
        processes: None,
        utc_offset_minutes: None,
        language: Language::Italian,
    };
    let html = render_html(&b, true).expect("html");
    assert!(html.contains("Nessun risultato"));
    let json = render_json(&b, true).expect("json");
    let doc: ExportDocument = serde_json::from_str(&json).expect("parse");
    assert_eq!(doc.report, report);
}

#[test]
fn file_names_have_no_hostname() {
    let f = fixture();
    assert_eq!(
        report_file_name(&f.report, ReportFormat::Html, None),
        "pc-diagnostics-2026-11-04-0121.html"
    );
    assert_eq!(
        report_file_name(&f.report, ReportFormat::Json, None),
        "pc-diagnostics-2026-11-04-0121.json"
    );
    // Local time (Italy, CET) when the offset is known.
    assert_eq!(
        report_file_name(&f.report, ReportFormat::Html, Some(60)),
        "pc-diagnostics-2026-11-04-0221.html"
    );
}

#[test]
fn redactor_detects_usernames_from_any_path() {
    let f = fixture();
    let r = Redactor::from_bundle(&bundle(&f));
    assert_eq!(r.scrub("ciao mario.rossi!"), "ciao <utente>!");
    assert_eq!(r.scrub("desktop-secret1 online"), "<host> online");
    assert_eq!(
        r.scrub("Webcam USB\\VID_1234&PID_5678\\SERIALXYZ6"),
        "Webcam <id dispositivo>"
    );
}

/// A fixture whose problem device carries the user name, so it reaches the plain text.
fn fixture_with_named_device() -> Fixture {
    let mut f = fixture();
    f.drivers.devices.push(device(
        &format!("Cuffie di {USER}"),
        DeviceCategory::Audio,
        DeviceHealth::ProblemDetected,
    ));
    let mut input = input(&[]);
    input.system = Some(&f.system);
    input.drivers = Some(&f.drivers);
    input.processes = Some(&f.processes);
    f.report = evaluate(&input);
    f
}

#[test]
fn plain_text_is_redacted_like_the_rest() {
    let f = fixture_with_named_device();
    let unredacted = f
        .report
        .findings
        .iter()
        .find(|x| x.title.contains("Cuffie"))
        .expect("device finding");
    assert!(unredacted.simple.headline.contains(USER));

    let json = render_json(&bundle(&f), true).expect("json");
    assert_clean(&json);
    let doc: ExportDocument = serde_json::from_str(&json).expect("parse");
    let redacted = doc
        .report
        .findings
        .iter()
        .find(|x| x.title.contains("Cuffie"))
        .expect("device finding");
    assert_eq!(
        redacted.simple.headline,
        "Windows ha fermato un dispositivo perché ha dato errori: Cuffie di <utente>"
    );
    assert!(!redacted.simple.meaning.is_empty() && !redacted.simple.action.is_empty());

    let html = render_html(&bundle(&f), true).expect("html");
    assert_clean(&html);
    assert!(html.contains("Cuffie di &lt;utente&gt;"));
}

#[test]
fn html_shows_plain_block_before_technical_details() {
    let f = fixture();
    let html = render_html(&bundle(&f), false).expect("html");
    let finding = f
        .report
        .findings
        .iter()
        .find(|x| x.rule_id == "cpu.utilization.sustained")
        .expect("cpu finding");
    let block = html.find("In parole semplici").expect("plain block");
    let headline = html
        .find(&html::escape(&finding.simple.headline))
        .expect("headline");
    let action = html
        .find(&html::escape(&finding.simple.action))
        .expect("action");
    let technical = html
        .find(&html::escape(&finding.description))
        .expect("description");
    assert!(block < headline && headline < action && action < technical);
    assert_eq!(
        html.matches("In parole semplici").count(),
        f.report.findings.len()
    );
    // Injection through a process name stays escaped in the plain text too.
    assert!(html.contains("risorse del PC: &lt;script&gt;alert(1)&lt;/script&gt;.exe"));
}

#[test]
fn html_without_plain_text_falls_back_to_technical_only() {
    let mut f = fixture();
    for x in &mut f.report.findings {
        x.simple = Default::default();
    }
    let html = render_html(&bundle(&f), false).expect("html");
    assert!(!html.contains("In parole semplici"));
    assert!(html.contains(&html::escape(&f.report.findings[0].description)));
}

/// The fixture evaluated and exported in English.
fn english_fixture() -> Fixture {
    let mut f = fixture_with_named_device();
    let samples = live(60, |_, s| {
        s.cpu.usage_percent = avail(99.0, Unit::Percent, Source::Pdh);
    });
    let mut input = input(&samples);
    input.system = Some(&f.system);
    input.inventory = Some(&f.inventory);
    input.drivers = Some(&f.drivers);
    input.processes = Some(&f.processes);
    input.benchmarks = &f.benchmarks;
    input.language = Language::English;
    f.report = evaluate(&input);
    f
}

fn english_bundle(f: &Fixture) -> ReportBundle<'_> {
    ReportBundle {
        language: Language::English,
        ..bundle(f)
    }
}

#[test]
fn english_html_has_english_labels_dates_and_lang() {
    let f = english_fixture();
    let html = render_html(&english_bundle(&f), false).expect("html");
    assert!(html.starts_with("<!DOCTYPE html>\n<html lang=\"en\">"));
    for label in [
        "<title>PC Diagnostics report Nov 4, 2026, 1:31:00 AM UTC</title>",
        "<h1>Diagnostic report</h1>",
        "Coverage",
        "Findings",
        "Hardware configuration",
        "Tests run",
        "Drivers and devices",
        "Processes",
        "Limitations of the result",
        "In plain words",
        "What you can do:",
        "Technical details",
        "confidence high",
        "Generated by PC Diagnostics 0.1.0",
    ] {
        assert!(html.contains(label), "{label}");
    }
    for italian in [
        "Copertura",
        "Risultati",
        "In parole semplici",
        "Dettagli tecnici",
        "Limitazioni",
        "non disponibile",
        "Nessun",
        "lang=\"it\"",
    ] {
        assert!(!html.contains(italian), "Italian label {italian:?}");
    }
    // Benchmark labels and numbers use English formatting.
    assert!(html.contains("Sequential disk"));
    assert!(html.contains("Start</th><td class=\"wrap\">Nov 4, 2026, 1:21:00 AM UTC"));
}

#[test]
fn english_export_uses_english_placeholders() {
    let f = english_fixture();
    let json = render_json(&english_bundle(&f), true).expect("json");
    assert_clean(&json);
    let doc: ExportDocument = serde_json::from_str(&json).expect("parse");
    assert_eq!(doc.language, Language::English);
    let device = doc
        .report
        .findings
        .iter()
        .find(|x| x.title.contains("Cuffie"))
        .expect("device finding");
    assert_eq!(
        device.simple.headline,
        "Windows stopped a device because it reported errors: Cuffie di <user>"
    );
    assert!(json.contains("Users\\\\<user>"));
    let html = render_html(&english_bundle(&f), true).expect("html");
    assert_clean(&html);
    assert!(html.contains("Cuffie di &lt;user&gt;"));
    assert!(html.contains("removed (host name, paths, device identifiers"));
    let r = Redactor::from_bundle(&english_bundle(&f));
    assert_eq!(
        r.scrub("Webcam USB\\VID_1234&PID_5678\\SERIALXYZ6"),
        "Webcam <device id>"
    );
    // The Italian export of the same data keeps the Italian placeholders.
    let it = render_json(&bundle(&f), true).expect("json");
    assert!(it.contains("Cuffie di <utente>"));
    assert!(it.contains("\"language\": \"it\""));
}
