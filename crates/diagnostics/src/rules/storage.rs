//! Storage rules: free space, OS disk health summary (Windows; macOS when the inventory
//! provides one), storage benchmarks.

use models::Source;
use models::app::Platform;
use models::benchmark::BenchmarkKind;
use models::component::ComponentKind;
use models::diagnostics::{Confidence, FindingKind, Severity};
use models::hardware::DiskHealthStatus;
use models::settings::Language;
use models::{Metric, Unavailability};

use super::cpu::latest;
use super::{Cov, Ctx, Outcome, by_os, ev, finding, no_action, simple, strings};
use crate::compare::{compare_in, find_previous_comparable};
use crate::fmt;
use crate::report::kind_label;
use crate::window::src;

const GIB: u64 = 1024 * 1024 * 1024;
/// Free space below this share of the volume → Warning.
pub const FREE_WARNING_PERCENT: f64 = 10.0;
/// Free space below this share of the volume → Critical.
pub const FREE_CRITICAL_PERCENT: f64 = 5.0;
/// Free space below this absolute amount → Warning…
pub const FREE_WARNING_BYTES: u64 = 15 * GIB;
/// …only on volumes at least this large (smaller volumes would always trigger it).
pub const FREE_ABSOLUTE_MIN_VOLUME_BYTES: u64 = 2 * FREE_WARNING_BYTES;
/// Worsening of a storage benchmark metric vs the previous comparable run.
pub const STORAGE_BENCH_DROP_PERCENT: f64 = 25.0;

fn health_limitation(lang: Language, platform: Platform) -> &'static str {
    by_os(
        platform,
        lang.pick(
            "Stato riassuntivo di Windows, non un'analisi SMART completa.",
            "Windows summary status, not a full SMART analysis.",
        ),
        lang.pick(
            "Stato riassuntivo di macOS, non un'analisi SMART completa.",
            "macOS summary status, not a full SMART analysis.",
        ),
    )
}

/// Plain-language headline for a volume low on space. Windows names the drive letter
/// (`C:`); macOS volumes are mount points, so `/` is called the startup disk and other
/// volumes keep their mount point (`/Volumes/Dati`). Volume labels are never used: they
/// may be personal and the report redaction removes them.
fn low_space_headline(lang: Language, platform: Platform, mount: &str, critical: bool) -> String {
    if platform == Platform::Windows {
        let drive = drive_name(mount);
        return if critical {
            lang.pick(
                format!("Il disco {drive} è quasi pieno"),
                format!("Drive {drive} is almost full"),
            )
        } else {
            lang.pick(
                format!("Sul disco {drive} resta poco spazio libero"),
                format!("Drive {drive} is running low on free space"),
            )
        };
    }
    let (it, en) = if mount == "/" {
        ("disco di avvio".to_owned(), "The startup disk".to_owned())
    } else {
        (format!("disco {mount}"), format!("Drive {mount}"))
    };
    if critical {
        lang.pick(
            format!("Il {it} è quasi pieno"),
            format!("{en} is almost full"),
        )
    } else {
        lang.pick(
            format!("Sul {it} resta poco spazio libero"),
            format!("{en} is running low on free space"),
        )
    }
}

/// Drive name as users see it in Esplora file: `C:\` → `C:`.
fn drive_name(mount_point: &str) -> &str {
    let trimmed = mount_point.trim_end_matches(['\\', '/']);
    if trimmed.is_empty() {
        mount_point
    } else {
        trimmed
    }
}

/// What a storage benchmark measures, in everyday words.
fn plain_kind(lang: Language, kind: BenchmarkKind) -> &'static str {
    match kind {
        BenchmarkKind::StorageSequential => {
            lang.pick("test con file grandi", "test with large files")
        }
        BenchmarkKind::StorageRandom => {
            lang.pick("test con tanti file piccoli", "test with many small files")
        }
        _ => lang.pick("test del disco", "disk test"),
    }
}

pub fn evaluate(ctx: &Ctx) -> Outcome {
    let mut out = Outcome::default();
    let mut cov = Cov::new(ComponentKind::Storage);
    let lang = ctx.lang;
    let p = ctx.platform;

    match ctx.input.inventory {
        None => cov.not_evaluated(lang.pick(
            "inventario hardware non disponibile: spazio libero e stato dei dischi non valutati",
            "hardware inventory not available: free space and disk status not evaluated",
        )),
        Some(inv) => {
            let mut checked = 0;
            for v in inv
                .volumes
                .iter()
                .filter(|v| !v.removable && v.total_bytes > 0)
            {
                checked += 1;
                let free_pct = v.available_bytes as f64 / v.total_bytes as f64 * 100.0;
                let severity = if free_pct < FREE_CRITICAL_PERCENT {
                    Some(Severity::Critical)
                } else if free_pct < FREE_WARNING_PERCENT
                    || (v.total_bytes >= FREE_ABSOLUTE_MIN_VOLUME_BYTES
                        && v.available_bytes < FREE_WARNING_BYTES)
                {
                    Some(Severity::Warning)
                } else {
                    None
                };
                let Some(severity) = severity else { continue };
                let mount = &v.mount_point;
                let (free, size, pct) = (
                    fmt::bytes_u64(lang, v.available_bytes),
                    fmt::bytes_u64(lang, v.total_bytes),
                    fmt::pct(lang, free_pct),
                );
                let (free_plain, size_plain, pct_plain) = (
                    fmt::storage_plain(lang, p, v.available_bytes as f64),
                    fmt::storage_plain(lang, p, v.total_bytes as f64),
                    fmt::il_pct(lang, free_pct),
                );
                let mut f = finding(
                    "storage.volume.low_space",
                    ComponentKind::Storage,
                    severity,
                    FindingKind::Confirmed,
                    Confidence::High,
                    lang.pick(
                        format!("Spazio libero scarso su {mount}"),
                        format!("Low free space on {mount}"),
                    ),
                    lang.pick(
                        format!("Il volume {mount} ha {free} liberi su {size} ({pct})."),
                        format!("Volume {mount} has {free} free of {size} ({pct})."),
                    ),
                    simple(
                        low_space_headline(lang, p, mount, severity == Severity::Critical),
                        by_os(
                            p,
                            lang.pick(
                                format!("Restano liberi solo {free_plain} su {size_plain}, cioè {pct_plain} dello spazio. Con poco spazio Windows può non riuscire a installare gli aggiornamenti, i programmi possono non riuscire a salvare i file e il PC può rallentare."),
                                format!("Only {free_plain} of {size_plain} is free, which is {pct_plain} of the space. With little space, Windows may fail to install updates, programs may fail to save files, and the PC can slow down."),
                            ),
                            lang.pick(
                                format!("Restano liberi solo {free_plain} su {size_plain}, cioè {pct_plain} dello spazio. Con poco spazio macOS può non riuscire a installare gli aggiornamenti, i programmi possono non riuscire a salvare i file e il PC può rallentare."),
                                format!("Only {free_plain} of {size_plain} is free, which is {pct_plain} of the space. With little space, macOS may fail to install updates, programs may fail to save files, and the PC can slow down."),
                            ),
                        ),
                        by_os(
                            p,
                            lang.pick(
                                "Libera spazio: svuota il Cestino, elimina i file che non ti servono (ad esempio nella cartella Download) e usa «Pulizia disco» o «Sensore memoria» di Windows. Se hai un altro disco con spazio libero, puoi spostarci foto e video.",
                                "Free up space: empty the Recycle Bin, delete files you don't need (for example, in the Downloads folder), and use the Windows “Disk Cleanup” or “Storage Sense” tools. If you have another drive with free space, you can move photos and videos there.",
                            ),
                            lang.pick(
                                "Libera spazio: svuota il Cestino, elimina i file che non ti servono (ad esempio nella cartella Download) e guarda i suggerimenti in Impostazioni di Sistema > Generali > Spazio. Se hai un altro disco con spazio libero, puoi spostarci foto e video.",
                                "Free up space: empty the Trash, delete files you don't need (for example, in the Downloads folder), and check the recommendations in System Settings > General > Storage. If you have another drive with free space, you can move photos and videos there.",
                            ),
                        ),
                    ),
                );
                f.evidence = vec![
                    ev(
                        lang.pick(
                            format!("Spazio libero su {mount}"),
                            format!("Free space on {mount}"),
                        ),
                        format!("{free} ({pct})"),
                        Source::Sysinfo,
                    ),
                    ev(
                        lang.pick("Capacità del volume", "Volume capacity"),
                        size,
                        Source::Sysinfo,
                    ),
                ];
                f.possible_causes = strings(lang.pick(
                    &[
                        "File temporanei, download o file personali accumulati",
                        by_os(
                            p,
                            "Aggiornamenti di Windows o punti di ripristino",
                            "Aggiornamenti di macOS o istantanee locali di Time Machine",
                        ),
                    ],
                    &[
                        "Accumulated temporary files, downloads or personal files",
                        by_os(
                            p,
                            "Windows updates or restore points",
                            "macOS updates or local Time Machine snapshots",
                        ),
                    ],
                ));
                f.recommendation = by_os(
                    p,
                    lang.pick(
                        "Liberare spazio: rimuovere file non necessari, svuotare il Cestino, usare Pulizia disco o Sensore memoria di Windows, oppure spostare file personali su un altro volume.",
                        "Free up space: remove unneeded files, empty the Recycle Bin, use Windows Disk Cleanup or Storage Sense, or move personal files to another volume.",
                    ),
                    lang.pick(
                        "Liberare spazio: rimuovere file non necessari, svuotare il Cestino, usare Impostazioni di Sistema > Generali > Spazio, oppure spostare file personali su un altro volume.",
                        "Free up space: remove unneeded files, empty the Trash, use System Settings > General > Storage, or move personal files to another volume.",
                    ),
                ).into();
                f.limitations = strings(&[lang.pick(
                    "Lo spazio libero è misurato al momento della raccolta dell'inventario.",
                    "Free space is measured when the inventory is collected.",
                )]);
                out.findings.push(f);
            }
            if checked == 0 {
                cov.partial(lang.pick("nessun volume fisso rilevato", "no fixed volume detected"));
            }

            match &inv.physical_disks {
                Metric::Unavailable { reason } => {
                    let why = unavailability_label(lang, reason);
                    cov.partial(lang.pick(
                        format!("stato dei dischi non disponibile ({why})"),
                        format!("disk status not available ({why})"),
                    ))
                }
                Metric::Available { value: disks, .. } => {
                    for d in disks {
                        let name = d.model.clone().unwrap_or_else(|| {
                            lang.pick(format!("Disco {}", d.id), format!("Disk {}", d.id))
                        });
                        match d.health.value().copied() {
                            Some(DiskHealthStatus::Warning) | Some(DiskHealthStatus::Unhealthy) => {
                                let unhealthy =
                                    d.health.value() == Some(&DiskHealthStatus::Unhealthy);
                                let status = if unhealthy {
                                    lang.pick("non integro (Unhealthy)", "Unhealthy")
                                } else {
                                    lang.pick("avviso (Warning)", "Warning")
                                };
                                let os = by_os(p, "Windows", "macOS");
                                let mut f = finding(
                                    "storage.disk.health",
                                    ComponentKind::Storage,
                                    if unhealthy {
                                        Severity::Critical
                                    } else {
                                        Severity::Warning
                                    },
                                    FindingKind::Confirmed,
                                    Confidence::High,
                                    lang.pick(
                                        format!("{os} segnala un problema di salute del disco: {name}"),
                                        format!("{os} reports a disk health problem: {name}"),
                                    ),
                                    lang.pick(
                                        format!("Lo stato di salute riportato da {os} per il disco «{name}» è «{status}»."),
                                        format!("The health status {os} reports for disk “{name}” is “{status}”."),
                                    ),
                                    simple(
                                        if unhealthy {
                                            lang.pick(
                                                format!("{os} segnala che un disco ha un problema serio: {name}"),
                                                format!("{os} reports that a disk has a serious problem: {name}"),
                                            )
                                        } else {
                                            lang.pick(
                                                format!("{os} segnala un avviso sullo stato di un disco: {name}"),
                                                format!("{os} reports a warning about a disk's condition: {name}"),
                                            )
                                        },
                                        lang.pick(
                                            format!("Di solito significa che il disco ha registrato errori o segni di usura. Un disco in queste condizioni può perdere i dati che contiene, quindi è importante metterli al sicuro. È un riepilogo fornito da {os}, non un controllo completo del disco."),
                                            format!("This usually means the disk has recorded errors or signs of wear. A disk in this condition can lose the data on it, so it's important to keep that data safe. This is a summary provided by {os}, not a full check of the disk."),
                                        ),
                                        lang.pick(
                                            "Fai subito una copia dei tuoi file importanti su un'altra unità o su un servizio di archiviazione online, poi controlla lo stato del disco con il programma del produttore.",
                                            "Right away, copy your important files to another drive or to an online storage service, then check the disk's condition with the manufacturer's program.",
                                        ),
                                    ),
                                );
                                f.evidence = vec![ev(
                                    by_os(
                                        p,
                                        lang.pick(
                                            "Stato di salute (MSFT_PhysicalDisk.HealthStatus)",
                                            "Health status (MSFT_PhysicalDisk.HealthStatus)",
                                        ),
                                        lang.pick("Stato di salute", "Health status"),
                                    ),
                                    if unhealthy { "Unhealthy" } else { "Warning" },
                                    by_os(p, Source::Wmi, src(&d.health).unwrap_or(Source::IoKit)),
                                )];
                                f.possible_causes = strings(&[lang.pick(
                                    "Il disco ha segnalato errori o usura tramite i propri contatori interni",
                                    "The disk reported errors or wear through its internal counters",
                                )]);
                                f.recommendation = lang.pick(
                                    "Eseguire subito un backup dei dati importanti e verificare i dettagli con lo strumento del produttore del disco.",
                                    "Back up important data right away and check the details with the disk manufacturer's tool.",
                                ).into();
                                f.limitations = strings(&[health_limitation(lang, p)]);
                                out.findings.push(f);
                            }
                            Some(DiskHealthStatus::Healthy) => {}
                            Some(DiskHealthStatus::Unknown) | None => {
                                cov.partial(lang.pick(
                                    format!("stato di salute non disponibile per {name}"),
                                    format!("health status not available for {name}"),
                                ));
                            }
                        }
                    }
                    cov.note(by_os(
                        p,
                        lang.pick(
                            "stato dei dischi: riepilogo di Windows, non un'analisi SMART completa",
                            "disk status: Windows summary, not a full SMART analysis",
                        ),
                        lang.pick(
                            "stato dei dischi: riepilogo di macOS, non un'analisi SMART completa",
                            "disk status: macOS summary, not a full SMART analysis",
                        ),
                    ));
                }
            }
        }
    }

    storage_benchmarks(ctx, &mut out, &mut cov);
    out.coverage.push(cov.finish());
    out
}

fn storage_benchmarks(ctx: &Ctx, out: &mut Outcome, cov: &mut Cov) {
    let lang = ctx.lang;
    let mut any = false;
    for kind in [
        BenchmarkKind::StorageSequential,
        BenchmarkKind::StorageRandom,
    ] {
        let Some(b) = latest(ctx.input.benchmarks, Some(kind)) else {
            continue;
        };
        any = true;
        let label = kind_label(lang, kind);
        let plain = plain_kind(lang, kind);
        let when = fmt::datetime_utc(lang, b.started_at);
        let model = b.context.storage_model.as_deref();
        let mut f = finding(
            "storage.benchmark.result",
            ComponentKind::Storage,
            Severity::Info,
            FindingKind::Observation,
            Confidence::High,
            lang.pick(
                format!("Risultati dell'ultimo benchmark: {label}"),
                format!("Results of the latest benchmark: {label}"),
            ),
            lang.pick(
                format!(
                    "Valori misurati il {when}{}.",
                    model.map(|m| format!(" sul disco «{m}»")).unwrap_or_default()
                ),
                format!(
                    "Values measured on {when}{}.",
                    model.map(|m| format!(" on disk “{m}”")).unwrap_or_default()
                ),
            ),
            simple(
                lang.pick(
                    format!("È stata misurata la velocità del disco ({plain})"),
                    format!("The disk's speed was measured ({plain})"),
                ),
                lang.pick(
                    "Questi valori servono come riferimento: da soli non dicono se il disco è lento o veloce, ma permettono di confrontare i risultati dei test futuri.",
                    "These values serve as a reference: on their own, they don't tell whether the disk is slow or fast, but they let you compare the results of future tests.",
                ),
                no_action(lang),
            ),
        );
        f.evidence = b
            .metrics
            .iter()
            .map(|m| {
                ev(
                    m.label.clone(),
                    fmt::value_with_unit(lang, m.value, m.unit),
                    Source::Benchmark,
                )
            })
            .collect();
        f.recommendation = lang
            .pick(
                "Nessuna azione: i valori servono come riferimento per confronti futuri.",
                "No action: the values serve as a reference for future comparisons.",
            )
            .into();
        f.limitations = strings(&[lang.pick(
            "Senza un riferimento comparabile questi valori non indicano se il disco è lento o veloce.",
            "Without a comparable reference, these values do not show whether the disk is slow or fast.",
        )]);
        out.findings.push(f);

        let Some(prev) = find_previous_comparable(b, ctx.input.benchmarks) else {
            continue;
        };
        let cmp = compare_in(lang, b, prev);
        let worse: Vec<_> = cmp
            .deltas
            .iter()
            .filter_map(|d| {
                let pct = d.change_percent?;
                let worsening = if d.higher_is_better { -pct } else { pct };
                (worsening >= STORAGE_BENCH_DROP_PERCENT).then_some((d, worsening))
            })
            .collect();
        if worse.is_empty() {
            continue;
        }
        let worst = worse.iter().map(|(_, w)| *w).fold(0.0, f64::max);
        let prev_when = fmt::datetime_utc(lang, prev.started_at);
        let drop = fmt::pct(lang, STORAGE_BENCH_DROP_PERCENT);
        let worst_plain = fmt::il_pct(lang, worst);
        let caveat_note = if cmp.caveats.is_empty() {
            ""
        } else {
            lang.pick(
                " I due test sono stati fatti in condizioni diverse, quindi il confronto è poco affidabile.",
                " The two tests were run under different conditions, so the comparison is not very reliable.",
            )
        };
        let mut h = finding(
            "storage.benchmark.drop",
            ComponentKind::Storage,
            Severity::Warning,
            FindingKind::Hypothesis,
            if cmp.caveats.is_empty() {
                Confidence::Medium
            } else {
                Confidence::Low
            },
            lang.pick(
                format!("Prestazioni del disco peggiorate rispetto all'esecuzione precedente: {label}"),
                format!("Disk performance worse than in the previous run: {label}"),
            ),
            lang.pick(
                format!("Rispetto all'esecuzione comparabile del {prev_when}, almeno una metrica è peggiorata del {drop} o più."),
                format!("Compared with the comparable run of {prev_when}, at least one metric got worse by {drop} or more."),
            ),
            simple(
                lang.pick(
                    format!("Il disco potrebbe essere più lento di prima ({plain})"),
                    format!("The disk might be slower than before ({plain})"),
                ),
                lang.pick(
                    format!("Nell'ultimo test il disco è andato più piano rispetto al test precedente (una delle misure è peggiorata di circa {worst_plain}). Potrebbe dipendere da un disco quasi pieno, da altri programmi che usavano il disco durante il test o dal calore; i risultati dei dischi cambiano anche da un test all'altro.{caveat_note}"),
                    format!("In the latest test, the disk was slower than in the previous one (one of the measurements got worse by about {worst_plain}). This might come from a nearly full disk, other programs using the disk during the test, or heat. Disk results also vary from one test to the next.{caveat_note}"),
                ),
                lang.pick(
                    "Ripeti il test con pochi programmi aperti. Se il risultato resta peggiore, controlla lo spazio libero e lo stato del disco.",
                    "Run the test again with few programs open. If the result is still worse, check the free space and the condition of the disk.",
                ),
            ),
        );
        h.evidence = worse
            .iter()
            .map(|(d, w)| {
                let now = fmt::value_with_unit(lang, d.current, d.unit);
                let before = fmt::value_with_unit(lang, d.previous, d.unit);
                let w = fmt::pct(lang, *w);
                ev(
                    d.label.clone(),
                    lang.pick(
                        format!("{now} ora, {before} prima (peggioramento {w})"),
                        format!("{now} now, {before} before ({w} worse)"),
                    ),
                    Source::Benchmark,
                )
            })
            .collect();
        h.possible_causes = strings(lang.pick(
            &[
                "Disco quasi pieno",
                "Attività in background sul disco durante il test",
                "Limitazione termica del disco",
            ],
            &[
                "Disk almost full",
                "Background disk activity during the test",
                "Thermal throttling of the disk",
            ],
        ));
        h.recommendation = lang.pick(
            "Ripetere il test con poche applicazioni aperte; se il peggioramento si ripete, verificare lo spazio libero e lo stato del disco.",
            "Run the test again with few applications open; if the drop repeats, check the free space and the condition of the disk.",
        ).into();
        h.limitations = strings(&[lang.pick(
            "Confronto tra due sole esecuzioni: le prestazioni dei dischi variano tra esecuzioni.",
            "Comparison of only two runs: disk performance varies between runs.",
        )]);
        h.limitations.extend(cmp.caveats);
        out.findings.push(h);
    }
    if !any {
        cov.note(lang.pick(
            "nessun benchmark del disco disponibile",
            "no disk benchmark available",
        ));
    }
}

pub fn unavailability_label(lang: Language, r: &Unavailability) -> String {
    match r {
        Unavailability::NotSupported => lang.pick("non supportato", "not supported").into(),
        Unavailability::ProviderMissing(p) => lang.pick(
            format!("componente mancante: {p}"),
            format!("missing component: {p}"),
        ),
        Unavailability::PermissionDenied => lang
            .pick("permessi insufficienti", "insufficient permissions")
            .into(),
        Unavailability::NotImplemented => lang
            .pick("non ancora implementato", "not implemented yet")
            .into(),
        Unavailability::Failed(e) => lang.pick(
            format!("lettura non riuscita: {}", e.message),
            format!("read failed: {}", e.message),
        ),
    }
}

#[cfg(test)]
mod tests {
    use crate::evaluate;
    use crate::testdata::*;
    use models::benchmark::BenchmarkKind;
    use models::component::ComponentKind;
    use models::diagnostics::{CoverageStatus, DiagnosticReport, Finding, FindingKind, Severity};
    use models::hardware::DiskHealthStatus;
    use models::{Metric, Unavailability, Unit};

    fn run(inv: &models::hardware::HardwareInventory) -> DiagnosticReport {
        let mut inp = input(&[]);
        inp.inventory = Some(inv);
        evaluate(&inp)
    }

    fn by<'a>(r: &'a DiagnosticReport, id: &str) -> Vec<&'a Finding> {
        r.findings.iter().filter(|f| f.rule_id == id).collect()
    }

    fn with_volume(total: f64, free: f64) -> models::hardware::HardwareInventory {
        let mut inv = inventory();
        inv.volumes = vec![volume("D:\\", total, free)];
        inv
    }

    #[test]
    fn low_space_thresholds() {
        // 100 GiB volume.
        let f = run(&with_volume(100.0, 4.9));
        assert_eq!(
            by(&f, "storage.volume.low_space")[0].severity,
            Severity::Critical
        );
        assert_eq!(
            by(&f, "storage.volume.low_space")[0].kind,
            FindingKind::Confirmed
        );
        let f = run(&with_volume(100.0, 5.0)); // 5 % but < 15 GiB → Warning
        assert_eq!(
            by(&f, "storage.volume.low_space")[0].severity,
            Severity::Warning
        );
        let f = run(&with_volume(1000.0, 99.0)); // 9.9 %
        assert_eq!(
            by(&f, "storage.volume.low_space")[0].severity,
            Severity::Warning
        );
        let f = run(&with_volume(1000.0, 100.0)); // exactly 10 %, 100 GiB free
        assert!(by(&f, "storage.volume.low_space").is_empty());
        let f = run(&with_volume(100.0, 14.9)); // 14.9 % but < 15 GiB
        assert_eq!(
            by(&f, "storage.volume.low_space")[0].severity,
            Severity::Warning
        );
        let f = run(&with_volume(100.0, 15.0));
        assert!(by(&f, "storage.volume.low_space").is_empty());
        // Small volume: absolute threshold not applied.
        let f = run(&with_volume(20.0, 10.0));
        assert!(by(&f, "storage.volume.low_space").is_empty());
    }

    #[test]
    fn removable_volumes_are_ignored() {
        let mut inv = with_volume(100.0, 1.0);
        inv.volumes[0].removable = true;
        assert!(by(&run(&inv), "storage.volume.low_space").is_empty());
    }

    #[test]
    fn disk_health_warning_and_unhealthy_are_confirmed_with_limitation() {
        for (h, sev) in [
            (DiskHealthStatus::Warning, Severity::Warning),
            (DiskHealthStatus::Unhealthy, Severity::Critical),
        ] {
            let mut inv = inventory();
            inv.physical_disks = Metric::Available {
                value: vec![disk(h)],
                unit: Unit::None,
                source: models::Source::Wmi,
                at: T0,
            };
            let r = run(&inv);
            let f = by(&r, "storage.disk.health");
            assert_eq!(f.len(), 1);
            assert_eq!(f[0].severity, sev);
            assert_eq!(f[0].kind, FindingKind::Confirmed);
            assert!(
                f[0].limitations
                    .iter()
                    .any(|l| l.contains("non un'analisi SMART completa"))
            );
        }
    }

    #[test]
    fn healthy_disk_is_not_reported_as_smart_ok() {
        let r = run(&inventory());
        assert!(by(&r, "storage.disk.health").is_empty());
        let json = serde_json::to_string(&r).expect("json");
        assert!(!json.contains("SMART OK"));
        let cov = r
            .coverage
            .iter()
            .find(|c| c.component == ComponentKind::Storage)
            .expect("cov");
        assert_eq!(cov.status, CoverageStatus::Evaluated);
        assert!(cov.note.contains("non un'analisi SMART completa"));
    }

    #[test]
    fn unavailable_disks_and_missing_inventory_coverage() {
        let mut inv = inventory();
        inv.physical_disks = Metric::unavailable(Unavailability::PermissionDenied);
        let r = run(&inv);
        let cov = r
            .coverage
            .iter()
            .find(|c| c.component == ComponentKind::Storage)
            .expect("cov");
        assert_eq!(cov.status, CoverageStatus::Partial);
        let r = evaluate(&input(&[]));
        let cov = r
            .coverage
            .iter()
            .find(|c| c.component == ComponentKind::Storage)
            .expect("cov");
        assert_eq!(cov.status, CoverageStatus::NotEvaluated);
    }

    #[test]
    fn storage_benchmark_is_observation_and_drop_is_hypothesis() {
        let mut prev = bench("p", BenchmarkKind::StorageSequential, T0);
        prev.metrics = vec![
            metric("read", 3000.0, Unit::BytesPerSecond, true),
            metric("latency", 100.0, Unit::Microseconds, false),
        ];
        let mut cur = prev.clone();
        cur.run_id = "c".into();
        cur.started_at = T0 + 1000;
        cur.metrics[0].value = 2250.0; // -25 %
        let history = [cur.clone(), prev.clone()];
        let mut inp = input(&[]);
        inp.benchmarks = &history;
        let r = evaluate(&inp);
        assert_eq!(by(&r, "storage.benchmark.result").len(), 1);
        assert_eq!(
            by(&r, "storage.benchmark.result")[0].kind,
            FindingKind::Observation
        );
        let drop = by(&r, "storage.benchmark.drop");
        assert_eq!(drop.len(), 1);
        assert_eq!(drop[0].kind, FindingKind::Hypothesis);

        // Latency up 24.9 % and read unchanged: no drop.
        cur.metrics[0].value = 3000.0;
        cur.metrics[1].value = 124.9;
        let history = [cur.clone(), prev.clone()];
        inp.benchmarks = &history;
        assert!(by(&evaluate(&inp), "storage.benchmark.drop").is_empty());
        // Latency up 25 % (lower is better) → drop.
        cur.metrics[1].value = 125.0;
        let history = [cur, prev];
        inp.benchmarks = &history;
        assert_eq!(by(&evaluate(&inp), "storage.benchmark.drop").len(), 1);
    }

    #[test]
    fn single_storage_benchmark_has_no_verdict() {
        let b = [bench("s", BenchmarkKind::StorageRandom, T0)];
        let mut inp = input(&[]);
        inp.benchmarks = &b;
        let r = evaluate(&inp);
        assert_eq!(by(&r, "storage.benchmark.result").len(), 1);
        assert!(by(&r, "storage.benchmark.drop").is_empty());
    }

    #[test]
    fn english_texts() {
        let mut inv = inventory();
        inv.volumes = vec![volume("C:\\", 100.0, 1.0)];
        inv.physical_disks = Metric::Unavailable {
            reason: Unavailability::PermissionDenied,
        };
        let mut inp = input(&[]);
        inp.inventory = Some(&inv);
        inp.language = models::settings::Language::English;
        let r = evaluate(&inp);
        let f = r
            .findings
            .iter()
            .find(|f| f.rule_id == "storage.volume.low_space")
            .expect("finding");
        assert_eq!(f.title, "Low free space on C:\\");
        assert_eq!(
            f.description,
            "Volume C:\\ has 1.0 GiB free of 100.0 GiB (1.0%)."
        );
        assert_eq!(f.simple.headline, "Drive C: is almost full");
        let cov = r
            .coverage
            .iter()
            .find(|c| c.component == ComponentKind::Storage)
            .expect("cov");
        assert!(
            cov.note
                .contains("disk status not available (insufficient permissions)"),
            "{}",
            cov.note
        );
    }
}
