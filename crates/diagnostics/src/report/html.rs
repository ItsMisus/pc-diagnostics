//! Self-contained HTML report: no scripts, no external resources, escaped text only.
//!
//! Every label exists in Italian and English (`lang.pick`), chosen by
//! `ExportDocument::language`. The findings keep the language they were generated in.

use models::Metric;
use models::benchmark_run::{BenchmarkResult, PowerSource};
use models::diagnostics::Finding;
use models::drivers::DeviceHealth;
use models::hardware::{DiskHealthStatus, MediaKind};
use models::settings::Language;

use super::{
    ExportDocument, component_label, confidence_label, coverage_label, finding_kind_label,
    kind_label, severity_label, state_label,
};
use crate::fmt;
use crate::rules::drivers::category_label;
use crate::rules::processes::impact_disclaimer;
use crate::rules::storage::unavailability_label;

/// Escapes text for HTML element content and quoted attribute values.
pub fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
    out
}

const CSS: &str = "\
:root{color-scheme:light dark;--bg:#fff;--fg:#111;--muted:#555;--line:#ccc;--soft:#f3f3f3}\
@media (prefers-color-scheme:dark){:root{--bg:#121212;--fg:#eee;--muted:#aaa;--line:#444;--soft:#1e1e1e}}\
*{box-sizing:border-box}\
body{margin:0 auto;max-width:960px;padding:24px 16px;background:var(--bg);color:var(--fg);font:15px/1.5 system-ui,-apple-system,'Segoe UI',sans-serif}\
h1{font-size:1.6em;margin:0 0 4px}h2{font-size:1.2em;margin:32px 0 8px;border-bottom:1px solid var(--line);padding-bottom:4px}\
h3{font-size:1em;margin:0 0 4px}\
.muted{color:var(--muted)}\
table{border-collapse:collapse;width:100%;margin:8px 0}\
th,td{text-align:left;vertical-align:top;padding:4px 8px;border-bottom:1px solid var(--line)}\
th{font-weight:600}\
.finding{border:1px solid var(--line);border-radius:6px;padding:12px;margin:12px 0}\
.simple{background:var(--soft);border-radius:4px;padding:4px 10px;margin:8px 0}\
.simple p{margin:4px 0}\
.tag{display:inline-block;border:1px solid var(--line);border-radius:4px;padding:0 6px;margin-right:6px;font-size:.85em;background:var(--soft)}\
.critical{border-width:2px;border-color:var(--fg)}\
ul{margin:4px 0;padding-left:20px}\
.wrap{overflow-wrap:anywhere}";

fn tr(cells: &[String]) -> String {
    let mut s = String::from("<tr>");
    for c in cells {
        s.push_str("<td>");
        s.push_str(c);
        s.push_str("</td>");
    }
    s.push_str("</tr>");
    s
}

fn th(cells: &[&str]) -> String {
    let mut s = String::from("<tr>");
    for c in cells {
        s.push_str("<th>");
        s.push_str(&escape(c));
        s.push_str("</th>");
    }
    s.push_str("</tr>");
    s
}

fn kv_table(rows: &[(&str, String)]) -> String {
    let mut s = String::from("<table>");
    for (k, v) in rows {
        s.push_str(&format!(
            "<tr><th>{}</th><td class=\"wrap\">{}</td></tr>",
            escape(k),
            escape(v)
        ));
    }
    s.push_str("</table>");
    s
}

fn list(items: &[String]) -> String {
    if items.is_empty() {
        return String::new();
    }
    let mut s = String::from("<ul>");
    for i in items {
        s.push_str(&format!("<li>{}</li>", escape(i)));
    }
    s.push_str("</ul>");
    s
}

fn opt(lang: Language, v: &Option<String>) -> String {
    v.clone()
        .unwrap_or_else(|| lang.pick("non rilevato", "not detected").into())
}

fn metric_text<T>(lang: Language, m: &Metric<T>, f: impl Fn(&T) -> String) -> String {
    match m {
        Metric::Available { value, .. } => f(value),
        Metric::Unavailable { reason } => {
            let why = unavailability_label(lang, reason);
            lang.pick(
                format!("non disponibile ({why})"),
                format!("not available ({why})"),
            )
        }
    }
}

pub fn render(doc: &ExportDocument) -> String {
    let r = &doc.report;
    let lang = doc.language;
    let na = fmt::not_available(lang);
    let when = |ms: u64| fmt::datetime_local(lang, ms, doc.utc_offset_minutes);
    let mut h = String::with_capacity(32 * 1024);
    h.push_str(&format!(
        "<!DOCTYPE html>\n<html lang=\"{}\"><head><meta charset=\"utf-8\">",
        lang.pick("it", "en")
    ));
    h.push_str("<meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; style-src 'unsafe-inline'\">");
    h.push_str("<meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">");
    h.push_str("<meta name=\"color-scheme\" content=\"light dark\">");
    let generated = when(r.generated_at);
    h.push_str(&format!(
        "<title>{}</title><style>{CSS}</style></head><body>",
        escape(&lang.pick(
            format!("Report PC Diagnostics {generated}"),
            format!("PC Diagnostics report {generated}"),
        ))
    ));

    // Header.
    h.push_str(lang.pick("<h1>Report diagnostico</h1>", "<h1>Diagnostic report</h1>"));
    h.push_str(&kv_table(&[
        (lang.pick("Data e ora", "Date and time"), generated.clone()),
        (
            lang.pick("Versione software", "Software version"),
            r.app_version.clone(),
        ),
        (
            lang.pick("Versione regole", "Rules version"),
            r.rules_version.to_string(),
        ),
        (
            lang.pick(
                "Finestra di monitoraggio osservata",
                "Observed monitoring window",
            ),
            fmt::seconds(r.observed_window_s),
        ),
        (lang.pick("Identificativo", "ID"), r.id.clone()),
        (
            lang.pick("Dati personali", "Personal data"),
            if doc.redacted {
                lang.pick(
                    "rimossi (nome host, percorsi, identificativi dei dispositivi, etichette dei volumi, nome utente)",
                    "removed (host name, paths, device identifiers, volume labels, user name)",
                )
            } else {
                lang.pick("non rimossi", "not removed")
            }
            .into(),
        ),
    ]));
    h.push_str(lang.pick(
        "<p class=\"muted\">Nessun punteggio globale: ogni componente ha i propri risultati e una copertura che indica cosa è stato valutato.</p>",
        "<p class=\"muted\">No overall score: each component has its own findings and a coverage status that shows what was evaluated.</p>",
    ));

    // Coverage.
    h.push_str(lang.pick("<h2>Copertura</h2><table>", "<h2>Coverage</h2><table>"));
    h.push_str(&th(&lang.pick(
        ["Componente", "Stato", "Note"],
        ["Component", "Status", "Notes"],
    )));
    for c in &r.coverage {
        h.push_str(&tr(&[
            escape(component_label(lang, c.component)),
            escape(coverage_label(lang, c.status)),
            escape(&c.note),
        ]));
    }
    h.push_str("</table>");

    // Findings.
    h.push_str(lang.pick("<h2>Risultati</h2>", "<h2>Findings</h2>"));
    if r.findings.is_empty() {
        h.push_str(lang.pick(
            "<p>Nessun risultato dalle regole sui dati disponibili. Questo non significa che sia tutto in ordine: vedere la copertura e le limitazioni.</p>",
            "<p>The rules produced no findings on the available data. This does not mean everything is fine: see the coverage and the limitations.</p>",
        ));
    }
    for f in &r.findings {
        h.push_str(&finding_html(lang, f));
    }

    // System.
    h.push_str(lang.pick("<h2>Sistema</h2>", "<h2>System</h2>"));
    match &doc.system {
        None => h.push_str(lang.pick(
            "<p class=\"muted\">Informazioni sul sistema non disponibili.</p>",
            "<p class=\"muted\">System information not available.</p>",
        )),
        Some(s) => {
            let mut rows = vec![
                (
                    lang.pick("Sistema operativo", "Operating system"),
                    opt(
                        lang,
                        &s.os_long_version.clone().or_else(|| s.os_name.clone()),
                    ),
                ),
                (lang.pick("Versione", "Version"), opt(lang, &s.os_version)),
                ("Kernel", opt(lang, &s.kernel_version)),
                (
                    lang.pick("Architettura CPU", "CPU architecture"),
                    s.cpu_arch.clone(),
                ),
                (
                    lang.pick("Architettura dell'applicazione", "Application architecture"),
                    s.build_arch.clone(),
                ),
            ];
            if let Some(u) = s.uptime_s {
                rows.push((
                    lang.pick("Tempo di attività", "Uptime"),
                    fmt::seconds(u32::try_from(u).unwrap_or(u32::MAX)),
                ));
            }
            if let Some(host) = &s.hostname {
                rows.push((lang.pick("Nome host", "Host name"), host.clone()));
            }
            h.push_str(&kv_table(&rows));
        }
    }

    // Hardware.
    h.push_str(lang.pick(
        "<h2>Configurazione hardware</h2>",
        "<h2>Hardware configuration</h2>",
    ));
    match &doc.hardware {
        None => h.push_str(lang.pick(
            "<p class=\"muted\">Inventario hardware non disponibile.</p>",
            "<p class=\"muted\">Hardware inventory not available.</p>",
        )),
        Some(hw) => {
            h.push_str(&kv_table(&[
                ("CPU", opt(lang, &hw.cpu.brand)),
                (
                    lang.pick("Core / thread", "Cores / threads"),
                    format!(
                        "{} / {}",
                        hw.cpu
                            .physical_cores
                            .map_or_else(|| na.into(), |c| c.to_string()),
                        hw.cpu.logical_cores
                    ),
                ),
                (
                    lang.pick("Frequenza massima", "Maximum frequency"),
                    metric_text(lang, &hw.cpu.max_mhz, |v| fmt::mhz(lang, *v)),
                ),
                (
                    lang.pick("Memoria totale", "Total memory"),
                    fmt::bytes_u64(lang, hw.memory.total_bytes),
                ),
            ]));
            if let Metric::Available { value: mods, .. } = &hw.memory.modules
                && !mods.is_empty()
            {
                h.push_str(lang.pick(
                    "<h3>Moduli di memoria</h3><table>",
                    "<h3>Memory modules</h3><table>",
                ));
                h.push_str(&th(&lang.pick(
                    ["Capacità", "Velocità", "Produttore", "Modello"],
                    ["Capacity", "Speed", "Manufacturer", "Model"],
                )));
                for m in mods {
                    h.push_str(&tr(&[
                        escape(&fmt::bytes_u64(lang, m.capacity_bytes)),
                        escape(
                            &m.configured_speed_mts
                                .or(m.speed_mts)
                                .map_or_else(|| na.into(), |s| format!("{s} MT/s")),
                        ),
                        escape(&opt(lang, &m.manufacturer)),
                        escape(&opt(lang, &m.part_number)),
                    ]));
                }
                h.push_str("</table>");
            }
            if !hw.gpus.is_empty() {
                h.push_str("<h3>GPU</h3><table>");
                h.push_str(&th(&lang.pick(
                    ["Nome", "Memoria dedicata", "Driver", "Note"],
                    ["Name", "Dedicated memory", "Driver", "Notes"],
                )));
                for g in &hw.gpus {
                    let note = if g.is_virtual {
                        lang.pick("adattatore virtuale", "virtual adapter")
                    } else if g.is_software {
                        lang.pick("adattatore software", "software adapter")
                    } else {
                        ""
                    };
                    h.push_str(&tr(&[
                        escape(&g.name),
                        escape(&fmt::bytes_u64(lang, g.dedicated_vram_bytes)),
                        escape(&metric_text(lang, &g.driver_version, |v| v.clone())),
                        escape(note),
                    ]));
                }
                h.push_str("</table>");
            }
            h.push_str(lang.pick("<h3>Dischi</h3>", "<h3>Disks</h3>"));
            match &hw.physical_disks {
                Metric::Unavailable { reason } => {
                    let why = escape(&unavailability_label(lang, reason));
                    h.push_str(&lang.pick(
                        format!(
                            "<p class=\"muted\">Elenco dei dischi non disponibile ({why}).</p>"
                        ),
                        format!("<p class=\"muted\">Disk list not available ({why}).</p>"),
                    ))
                }
                Metric::Available { value: disks, .. } => {
                    h.push_str("<table>");
                    h.push_str(&th(&lang.pick(
                        ["Modello", "Tipo", "Capacità", "Stato riportato da Windows"],
                        ["Model", "Type", "Capacity", "Status reported by Windows"],
                    )));
                    for d in disks {
                        h.push_str(&tr(&[
                            escape(&opt(lang, &d.model)),
                            escape(&format!("{} {:?}", media_label(d.media), d.bus)),
                            escape(&fmt::bytes_u64(lang, d.size_bytes)),
                            escape(&metric_text(lang, &d.health, |v| {
                                health_label(lang, *v).into()
                            })),
                        ]));
                    }
                    h.push_str(lang.pick(
                        "</table><p class=\"muted\">Lo stato è il riepilogo di Windows, non un'analisi SMART completa.</p>",
                        "</table><p class=\"muted\">The status is the Windows summary, not a full SMART analysis.</p>",
                    ));
                }
            }
            if !hw.volumes.is_empty() {
                h.push_str(lang.pick("<h3>Volumi</h3><table>", "<h3>Volumes</h3><table>"));
                h.push_str(&th(&lang.pick(
                    ["Volume", "Etichetta", "File system", "Libero", "Totale"],
                    ["Volume", "Label", "File system", "Free", "Total"],
                )));
                for v in &hw.volumes {
                    let free = if v.total_bytes > 0 {
                        format!(
                            "{} ({})",
                            fmt::bytes_u64(lang, v.available_bytes),
                            fmt::pct(
                                lang,
                                v.available_bytes as f64 / v.total_bytes as f64 * 100.0
                            )
                        )
                    } else {
                        fmt::bytes_u64(lang, v.available_bytes)
                    };
                    h.push_str(&tr(&[
                        escape(&v.mount_point),
                        escape(v.label.as_deref().unwrap_or("—")),
                        escape(v.file_system.as_deref().unwrap_or("—")),
                        escape(&free),
                        escape(&fmt::bytes_u64(lang, v.total_bytes)),
                    ]));
                }
                h.push_str("</table>");
            }
        }
    }

    // Benchmarks.
    h.push_str(lang.pick("<h2>Test eseguiti</h2>", "<h2>Tests run</h2>"));
    if doc.benchmarks.is_empty() {
        h.push_str(lang.pick(
            "<p class=\"muted\">Nessun benchmark incluso nel report.</p>",
            "<p class=\"muted\">No benchmarks included in the report.</p>",
        ));
    }
    for b in &doc.benchmarks {
        h.push_str(&benchmark_html(lang, b, doc.utc_offset_minutes));
    }

    // Drivers.
    h.push_str(lang.pick(
        "<h2>Driver e dispositivi</h2>",
        "<h2>Drivers and devices</h2>",
    ));
    match &doc.drivers {
        None => h.push_str(lang.pick(
            "<p class=\"muted\">Inventario dei dispositivi non disponibile.</p>",
            "<p class=\"muted\">Device inventory not available.</p>",
        )),
        Some(d) => {
            let s = &d.summary;
            h.push_str(&kv_table(&[
                (
                    lang.pick("Dispositivi presenti", "Devices present"),
                    s.total.to_string(),
                ),
                (lang.pick("Funzionanti", "Working"), s.working.to_string()),
                (
                    lang.pick("Con problemi", "With problems"),
                    s.problems.to_string(),
                ),
                (
                    lang.pick("Senza driver", "Without a driver"),
                    s.driver_missing.to_string(),
                ),
                (lang.pick("Disattivati", "Disabled"), s.disabled.to_string()),
                (
                    lang.pick("Aggiornamenti disponibili", "Updates available"),
                    s.updates_available.to_string(),
                ),
                (
                    lang.pick("Verifica aggiornamenti", "Update check"),
                    match &d.update_check {
                        None => lang.pick("non eseguita", "not run").into(),
                        Some(c) if c.error.is_some() => {
                            let at = when(c.checked_at);
                            lang.pick(format!("non riuscita ({at})"), format!("failed ({at})"))
                        }
                        Some(c) => {
                            let at = when(c.checked_at);
                            lang.pick(
                                format!("eseguita il {at} tramite Windows Update"),
                                format!("run on {at} via Windows Update"),
                            )
                        }
                    },
                ),
            ]));
            if d.problem_devices.is_empty() {
                h.push_str(lang.pick(
                    "<p>Nessun dispositivo con problemi segnalati da Windows.</p>",
                    "<p>No devices with problems reported by Windows.</p>",
                ));
            } else {
                h.push_str(lang.pick(
                    "<h3>Dispositivi con problemi</h3><table>",
                    "<h3>Devices with problems</h3><table>",
                ));
                let mut head = lang
                    .pick(
                        ["Dispositivo", "Categoria", "Stato", "Dettaglio", "Driver"],
                        ["Device", "Category", "Status", "Detail", "Driver"],
                    )
                    .to_vec();
                let with_id = d.problem_devices.iter().any(|p| p.instance_id.is_some());
                if with_id {
                    head.push("Instance id");
                }
                h.push_str(&th(&head));
                for p in &d.problem_devices {
                    let mut cells = vec![
                        escape(&p.name),
                        escape(category_label(lang, p.category)),
                        escape(match p.health {
                            DeviceHealth::DriverMissing => {
                                lang.pick("driver mancante", "driver missing")
                            }
                            DeviceHealth::ProblemDetected => {
                                lang.pick("problema rilevato", "problem detected")
                            }
                            DeviceHealth::Disabled => lang.pick("disattivato", "disabled"),
                            DeviceHealth::Working => lang.pick("funzionante", "working"),
                        }),
                        escape(p.problem_description.as_deref().unwrap_or("—")),
                        escape(
                            format!(
                                "{} {}",
                                p.driver_provider.as_deref().unwrap_or(""),
                                p.driver_version.as_deref().unwrap_or("")
                            )
                            .trim(),
                        ),
                    ];
                    if with_id {
                        cells.push(format!(
                            "<span class=\"wrap\">{}</span>",
                            escape(p.instance_id.as_deref().unwrap_or("—"))
                        ));
                    }
                    h.push_str(&tr(&cells));
                }
                h.push_str("</table>");
            }
        }
    }

    // Processes.
    h.push_str(lang.pick("<h2>Processi</h2>", "<h2>Processes</h2>"));
    match &doc.processes {
        None => h.push_str(lang.pick(
            "<p class=\"muted\">Analisi dei processi non disponibile.</p>",
            "<p class=\"muted\">Process analysis not available.</p>",
        )),
        Some(p) => {
            let (n, w, total, v, disclaimer) = (
                p.top.len(),
                escape(&fmt::seconds(p.window_s)),
                p.total_processes,
                p.methodology_version,
                escape(impact_disclaimer(lang)),
            );
            h.push_str(&lang.pick(
                format!("<p>Primi {n} processi per Impact Score medio nella finestra di {w} ({total} processi analizzati, metodologia v{v}). {disclaimer}</p>"),
                format!("<p>Top {n} processes by average Impact Score in the {w} window ({total} processes analyzed, methodology v{v}). {disclaimer}</p>"),
            ));
            h.push_str("<table>");
            h.push_str(&th(&lang.pick(
                ["Processo", "Impact medio", "Picco", "CPU", "Memoria"],
                ["Process", "Average impact", "Peak", "CPU", "Memory"],
            )));
            for e in &p.top {
                h.push_str(&tr(&[
                    escape(&e.name),
                    escape(&fmt::num(lang, e.impact, 1)),
                    escape(&fmt::num(lang, e.impact_peak, 1)),
                    escape(
                        &e.cpu_percent
                            .map_or_else(|| na.into(), |v| fmt::pct(lang, v)),
                    ),
                    escape(
                        &e.working_set_bytes
                            .map_or_else(|| na.into(), |v| fmt::bytes(lang, v)),
                    ),
                ]));
            }
            h.push_str("</table>");
        }
    }

    // Limitations.
    h.push_str(lang.pick(
        "<h2>Limitazioni del risultato</h2>",
        "<h2>Limitations of the result</h2>",
    ));
    h.push_str(&list(&r.limitations));
    let (app, format, version) = (
        escape(&r.app_version),
        escape(&doc.format),
        doc.format_version,
    );
    h.push_str(&lang.pick(
        format!("<p class=\"muted\">Generato da PC Diagnostics {app} — {format} {version}.</p>"),
        format!("<p class=\"muted\">Generated by PC Diagnostics {app} — {format} {version}.</p>"),
    ));
    h.push_str("</body></html>\n");
    h
}

fn finding_html(lang: Language, f: &Finding) -> String {
    let critical = f.severity == models::diagnostics::Severity::Critical;
    let mut s = format!(
        "<div class=\"finding{}\">",
        if critical { " critical" } else { "" }
    );
    s.push_str(&format!(
        "<p><span class=\"tag\">{}</span><span class=\"tag\">{}</span><span class=\"tag\">{}</span><span class=\"tag\">{} {}</span></p>",
        escape(severity_label(lang, f.severity)),
        escape(finding_kind_label(lang, f.kind)),
        escape(component_label(lang, f.component)),
        lang.pick("confidenza", "confidence"),
        escape(confidence_label(lang, f.confidence))
    ));
    s.push_str(&format!("<h3>{}</h3>", escape(&f.title)));
    // Reports saved before the plain-language text existed have it empty.
    let p = &f.simple;
    if !(p.headline.is_empty() && p.meaning.is_empty() && p.action.is_empty()) {
        s.push_str(&format!(
            "<div class=\"simple\"><p class=\"muted\"><strong>{}</strong></p>",
            lang.pick("In parole semplici", "In plain words")
        ));
        s.push_str(&format!("<p><strong>{}</strong></p>", escape(&p.headline)));
        s.push_str(&format!("<p>{}</p>", escape(&p.meaning)));
        s.push_str(&format!(
            "<p><strong>{}</strong> {}</p></div>",
            lang.pick("Cosa puoi fare:", "What you can do:"),
            escape(&p.action)
        ));
        s.push_str(&format!(
            "<p class=\"muted\"><strong>{}</strong></p>",
            lang.pick("Dettagli tecnici", "Technical details")
        ));
    }
    s.push_str(&format!("<p>{}</p>", escape(&f.description)));
    if !f.evidence.is_empty() {
        s.push_str("<table>");
        s.push_str(&th(&lang.pick(
            ["Evidenza", "Valore", "Fonte"],
            ["Evidence", "Value", "Source"],
        )));
        for e in &f.evidence {
            s.push_str(&tr(&[
                escape(&e.label),
                escape(&e.value),
                escape(&format!("{:?}", e.source)),
            ]));
        }
        s.push_str("</table>");
    }
    if !f.possible_causes.is_empty() {
        s.push_str(&format!(
            "<p><strong>{}</strong></p>",
            lang.pick("Possibili cause", "Possible causes")
        ));
        s.push_str(&list(&f.possible_causes));
    }
    if !f.recommendation.is_empty() {
        s.push_str(&format!(
            "<p><strong>{}</strong> {}</p>",
            lang.pick("Raccomandazione:", "Recommendation:"),
            escape(&f.recommendation)
        ));
    }
    if !f.limitations.is_empty() {
        s.push_str(&format!(
            "<p><strong>{}</strong></p>",
            lang.pick("Limitazioni", "Limitations")
        ));
        s.push_str(&list(&f.limitations));
    }
    s.push_str(&format!(
        "<p class=\"muted\">{} {}</p></div>",
        lang.pick("Regola:", "Rule:"),
        escape(&f.rule_id)
    ));
    s
}

fn benchmark_html(lang: Language, b: &BenchmarkResult, utc_offset_minutes: Option<i32>) -> String {
    let mut s = format!(
        "<div class=\"finding\"><h3>{}</h3>",
        escape(kind_label(lang, b.kind))
    );
    let q = &b.request;
    let mut params = Vec::new();
    if let Some(d) = q.duration_s {
        params.push(lang.pick(format!("durata {d} s"), format!("duration {d} s")));
    }
    params.push(match q.threads {
        Some(t) => lang.pick(
            format!("{t} thread"),
            format!("{t} {}", if t == 1 { "thread" } else { "threads" }),
        ),
        None => lang
            .pick("tutti i processori logici", "all logical processors")
            .into(),
    });
    if let Some(v) = &q.storage_volume {
        params.push(format!("volume {v}"));
    }
    if let Some(m) = q.storage_file_mib {
        params.push(lang.pick(
            format!("file di test {m} MiB"),
            format!("test file {m} MiB"),
        ));
    }
    let c = &b.context;
    let power = match c.power_source {
        PowerSource::Ac => lang.pick("rete elettrica", "AC power"),
        PowerSource::Battery => lang.pick("batteria", "battery"),
        PowerSource::Unknown => lang.pick("non rilevata", "not detected"),
    };
    let plan = c
        .power_plan
        .as_deref()
        .map(|p| lang.pick(format!(", modalità {p}"), format!(", {p} power mode")))
        .unwrap_or_default();
    s.push_str(&kv_table(&[
        (
            lang.pick("Avvio", "Start"),
            fmt::datetime_local(lang, b.started_at, utc_offset_minutes),
        ),
        (
            lang.pick("Durata", "Duration"),
            format!("{} s", fmt::num(lang, b.duration_ms as f64 / 1000.0, 1)),
        ),
        (
            lang.pick("Esito", "Result"),
            state_label(lang, b.state).into(),
        ),
        (lang.pick("Parametri", "Parameters"), params.join(", ")),
        (
            lang.pick("Metodologia", "Methodology"),
            format!("v{} ({})", b.methodology_version, b.build_profile),
        ),
        (
            lang.pick("Alimentazione", "Power"),
            format!("{power}{plan}"),
        ),
        (
            lang.pick("Carico CPU all'avvio", "CPU load at start"),
            metric_text(lang, &c.background_cpu_percent, |v| fmt::pct(lang, *v)),
        ),
    ]));
    if !b.metrics.is_empty() {
        s.push_str("<table>");
        s.push_str(&th(&lang.pick(
            ["Metrica", "Valore", "Migliore se"],
            ["Metric", "Value", "Better if"],
        )));
        for m in &b.metrics {
            s.push_str(&tr(&[
                escape(&m.label),
                escape(&fmt::value_with_unit(lang, m.value, m.unit)),
                escape(if m.higher_is_better {
                    lang.pick("più alto", "higher")
                } else {
                    lang.pick("più basso", "lower")
                }),
            ]));
        }
        s.push_str("</table>");
    }
    if !b.notes.is_empty() {
        s.push_str(&format!(
            "<p><strong>{}</strong></p>",
            lang.pick("Note", "Notes")
        ));
        s.push_str(&list(&b.notes));
    }
    if !b.warnings.is_empty() {
        s.push_str(&format!(
            "<p><strong>{}</strong></p>",
            lang.pick("Avvisi", "Warnings")
        ));
        s.push_str(&list(&b.warnings));
    }
    if let Some(stop) = &b.safety_stop {
        s.push_str(&format!(
            "<p><strong>{}</strong> {}</p>",
            lang.pick("Interrotto per sicurezza:", "Stopped for safety:"),
            escape(stop)
        ));
    }
    if let Some(e) = &b.error {
        s.push_str(&format!(
            "<p><strong>{}</strong> {}{}</p>",
            lang.pick("Errore:", "Error:"),
            escape(&e.message),
            e.detail
                .as_deref()
                .map(|d| format!(" — {}", escape(d)))
                .unwrap_or_default()
        ));
    }
    s.push_str("</div>");
    s
}

fn media_label(m: MediaKind) -> &'static str {
    match m {
        MediaKind::Ssd => "SSD",
        MediaKind::Hdd => "HDD",
        MediaKind::Unknown => "",
    }
}

fn health_label(lang: Language, h: DiskHealthStatus) -> &'static str {
    match h {
        DiskHealthStatus::Healthy => {
            lang.pick("Integro (riepilogo Windows)", "Healthy (Windows summary)")
        }
        DiskHealthStatus::Warning => lang.pick("Avviso", "Warning"),
        DiskHealthStatus::Unhealthy => lang.pick("Non integro", "Unhealthy"),
        DiskHealthStatus::Unknown => lang.pick("Sconosciuto", "Unknown"),
    }
}

#[cfg(test)]
mod tests {
    use super::escape;

    #[test]
    fn escapes_markup_and_quotes() {
        assert_eq!(
            escape(r#"<script>alert("x")</script> & 'y'"#),
            "&lt;script&gt;alert(&quot;x&quot;)&lt;/script&gt; &amp; &#39;y&#39;"
        );
        assert_eq!(escape("città"), "città");
    }
}
