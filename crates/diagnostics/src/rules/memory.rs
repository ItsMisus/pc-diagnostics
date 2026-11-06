//! Memory (RAM) rules.

use models::Source;
use models::component::ComponentKind;
use models::diagnostics::{Confidence, Evidence, FindingKind, Severity};
use models::settings::Language;

use super::{
    Cov, Ctx, Outcome, count_of, ev, finding, no_samples, simple, strings, top_ram_processes,
    window_notes,
};
use crate::fmt;
use crate::window::{Sustained, src, val};

/// Available memory below this share of total is "low".
pub const MEM_AVAILABLE_LOW_PERCENT: f64 = 10.0;
/// Available memory below this share of total, sustained, makes the finding Critical.
pub const MEM_AVAILABLE_CRITICAL_PERCENT: f64 = 5.0;
/// Committed memory at or above this share of the commit limit is "near the limit".
pub const MEM_COMMIT_HIGH_PERCENT: f64 = 90.0;
/// `\Memory\Pages/sec` at or above this value is "high" paging (≈ 4 MiB/s with 4 KiB pages).
pub const MEM_PAGES_HIGH_PER_SEC: f64 = 1000.0;
/// Processes listed as evidence.
pub const MEM_TOP_PROCESSES: usize = 3;

fn recommendation(lang: Language) -> &'static str {
    lang.pick(
        "Verificare i processi con maggiore consumo e ripetere la misurazione durante il problema.",
        "Check the processes using the most memory and measure again while the problem occurs.",
    )
}

/// " I programmi che usavano più memoria erano: a e b." (singular for one name).
fn top_sentence(lang: Language, top_names: &[String]) -> String {
    match top_names {
        [] => String::new(),
        [one] => lang.pick(
            format!(" Il programma che usava più memoria era: {one}."),
            format!(" The program using the most memory was: {one}."),
        ),
        many => {
            let names = fmt::join_names(lang, many);
            lang.pick(
                format!(" I programmi che usavano più memoria erano: {names}."),
                format!(" The programs using the most memory were: {names}."),
            )
        }
    }
}

/// Plain-language counterpart of `recommendation`, naming the top processes when known.
fn simple_action(lang: Language, top_names: &[String]) -> String {
    let base = lang.pick(
        "Chiudi i programmi e le schede del browser che non ti servono, poi ripeti la misurazione mentre il problema si presenta.",
        "Close the programs and browser tabs you don't need, then measure again while the problem is happening.",
    );
    if top_names.is_empty() {
        lang.pick(
            format!("{base} Nella pagina Processi puoi vedere quali programmi usano più memoria."),
            format!("{base} The Processes page shows which programs use the most memory."),
        )
    } else {
        format!("{base}{}", top_sentence(lang, top_names))
    }
}

pub fn evaluate(ctx: &Ctx) -> Outcome {
    let mut out = Outcome::default();
    let mut cov = Cov::new(ComponentKind::Memory);
    let live = ctx.input.live;
    let lang = ctx.lang;

    let avail_pct = |s: &models::monitor::LiveSample| -> Option<f64> {
        let total = val(&s.memory.total_bytes)?;
        let a = val(&s.memory.available_bytes)?;
        (total > 0.0).then(|| a / total * 100.0)
    };
    let commit_pct = |s: &models::monitor::LiveSample| -> Option<f64> {
        let limit = val(&s.memory.commit_limit_bytes)?;
        let used = val(&s.memory.commit_used_bytes)?;
        (limit > 0.0).then(|| used / limit * 100.0)
    };
    let low = Sustained::compute(live, avail_pct, |v| v < MEM_AVAILABLE_LOW_PERCENT);
    let critical = Sustained::compute(live, avail_pct, |v| v < MEM_AVAILABLE_CRITICAL_PERCENT);
    let commit = Sustained::compute(live, commit_pct, |v| v >= MEM_COMMIT_HIGH_PERCENT);
    let pages = Sustained::compute(
        live,
        |s| val(&s.memory.pages_per_sec),
        |v| v >= MEM_PAGES_HIGH_PER_SEC,
    );

    if !low.usable() {
        cov.not_evaluated(if live.is_empty() {
            no_samples(lang)
        } else {
            lang.pick(
                "memoria disponibile non rilevata nella maggior parte dei campioni",
                "available memory not detected in most samples",
            )
        });
        out.coverage.push(cov.finish());
        return out;
    }
    let w = fmt::seconds(ctx.window_s);
    cov.note(lang.pick(
        format!("memoria disponibile, commit e paginazione valutati su {w}"),
        format!("available memory, commit and paging evaluated over {w}"),
    ));
    window_notes(&mut cov, ctx.window_s);
    if !commit.usable() {
        cov.partial(lang.pick(
            "memoria impegnata (commit) non disponibile",
            "committed memory (commit) not available",
        ));
    }
    if !pages.usable() {
        cov.partial(lang.pick(
            "paginazione (Pages/sec) non disponibile",
            "paging (Pages/sec) not available",
        ));
    }

    let first = live.first();
    let mem_src = first
        .and_then(|s| src(&s.memory.available_bytes))
        .unwrap_or(Source::Sysinfo);
    let commit_src = first
        .and_then(|s| src(&s.memory.commit_used_bytes))
        .unwrap_or(Source::Pdh);
    let pages_src = first
        .and_then(|s| src(&s.memory.pages_per_sec))
        .unwrap_or(Source::Pdh);
    let total = live
        .iter()
        .rev()
        .find_map(|s| val(&s.memory.total_bytes))
        .unwrap_or(0.0);

    let top = top_ram_processes(ctx.input.processes, MEM_TOP_PROCESSES);
    let top_names: Vec<String> = top.iter().map(|r| r.name.clone()).collect();
    let procs: Vec<Evidence> = top
        .into_iter()
        .filter_map(|r| {
            let ws = val(&r.working_set_bytes)?;
            let name = &r.name;
            Some(ev(
                lang.pick(
                    format!("Memoria usata da {name}"),
                    format!("Memory used by {name}"),
                ),
                fmt::bytes(lang, ws),
                src(&r.working_set_bytes).unwrap_or(Source::Pdh),
            ))
        })
        .collect();
    let action = simple_action(lang, &top_names);
    let no_procs_limitation = || {
        lang.pick(
            "Classifica dei processi non disponibile: non è possibile indicare quali programmi usano più memoria.",
            "Process ranking not available: the programs using the most memory cannot be named.",
        )
        .to_string()
    };

    let low_fired = low.is_sustained(ctx.window_s);
    let commit_fired = commit.is_sustained(ctx.window_s);

    if low_fired {
        let is_critical = critical.is_sustained(ctx.window_s);
        let (low_pct, share) = (
            fmt::pct(lang, MEM_AVAILABLE_LOW_PERCENT),
            fmt::pct(lang, low.hit_percent()),
        );
        let used_plain = fmt::il_pct(lang, 100.0 - low.mean);
        let mut f = finding(
            "memory.available.low",
            ComponentKind::Memory,
            if is_critical {
                Severity::Critical
            } else {
                Severity::Warning
            },
            FindingKind::Observation,
            Confidence::High,
            lang.pick("Memoria disponibile scarsa", "Low available memory"),
            lang.pick(
                format!("La memoria disponibile è rimasta sotto il {low_pct} del totale nel {share} dei campioni."),
                format!("Available memory stayed below {low_pct} of the total in {share} of the samples."),
            ),
            simple(
                if is_critical {
                    lang.pick(
                        "Il PC è rimasto quasi senza memoria libera",
                        "The PC almost ran out of free memory",
                    )
                } else {
                    lang.pick(
                        "Il PC sta usando quasi tutta la memoria",
                        "The PC is using almost all of its memory",
                    )
                },
                lang.pick(
                    format!("La memoria (RAM, dove il PC tiene i programmi aperti) è stata quasi piena per gran parte del tempo osservato: in media era occupato {used_plain} del totale. Quando la memoria è quasi piena il PC può diventare lento e i programmi possono bloccarsi per qualche istante."),
                    format!("The memory (RAM, where the PC keeps open programs) was almost full for most of the time observed: on average, {used_plain} of it was in use. When memory is almost full, the PC can become slow and programs can freeze for a moment."),
                ),
                action.clone(),
            ),
        );
        let (mean, min) = (fmt::pct(lang, low.mean), fmt::pct(lang, low.min));
        let mean_bytes = fmt::bytes(lang, total * low.mean / 100.0);
        f.evidence = vec![
            ev(
                lang.pick("Memoria disponibile media", "Average available memory"),
                lang.pick(
                    format!("{mean} del totale ({mean_bytes})"),
                    format!("{mean} of total ({mean_bytes})"),
                ),
                mem_src,
            ),
            ev(
                lang.pick("Memoria disponibile minima", "Minimum available memory"),
                lang.pick(format!("{min} del totale"), format!("{min} of total")),
                mem_src,
            ),
            ev(
                lang.pick("Memoria totale", "Total memory"),
                fmt::bytes(lang, total),
                mem_src,
            ),
        ];
        f.evidence.extend(procs.iter().cloned());
        f.possible_causes = strings(lang.pick(
            &[
                "Molti programmi o schede del browser aperti contemporaneamente",
                "Un programma che usa molta memoria",
            ],
            &[
                "Many programs or browser tabs open at the same time",
                "A program using a lot of memory",
            ],
        ));
        f.recommendation = recommendation(lang).into();
        f.limitations = strings(&[lang.pick(
            "Il dato descrive solo la finestra osservata.",
            "The data describes only the observed window.",
        )]);
        if procs.is_empty() {
            f.limitations.push(no_procs_limitation());
        }
        out.findings.push(f);
    }

    if commit_fired {
        let (high, share) = (
            fmt::pct(lang, MEM_COMMIT_HIGH_PERCENT),
            fmt::pct(lang, commit.hit_percent()),
        );
        let mean_plain = fmt::il_pct(lang, commit.mean);
        let mut f = finding(
            "memory.commit.high",
            ComponentKind::Memory,
            Severity::Warning,
            FindingKind::Observation,
            Confidence::High,
            lang.pick(
                "Memoria impegnata vicina al limite",
                "Committed memory close to the limit",
            ),
            lang.pick(
                format!("La memoria impegnata (commit) è stata pari o superiore al {high} del limite (RAM + file di paging) nel {share} dei campioni."),
                format!("Committed memory (commit) was at or above {high} of the limit (RAM + paging file) in {share} of the samples."),
            ),
            simple(
                lang.pick(
                    "I programmi hanno prenotato quasi tutta la memoria che Windows può offrire",
                    "Programs have reserved almost all the memory Windows can offer",
                ),
                lang.pick(
                    format!("I programmi aperti hanno riservato per sé quasi tutta la memoria disponibile, contando anche lo spazio di riserva che Windows usa sul disco (in media {mean_plain} del massimo). Se si arriva al limite Windows può avvisare che la memoria è insufficiente; da solo questo dato non significa che il PC sia lento."),
                    format!("The open programs set aside almost all the available memory for themselves, counting the backup space Windows uses on the disk ({mean_plain} of the maximum on average). If the limit is reached, Windows may warn that memory is running low. On its own, this doesn't mean the PC is slow."),
                ),
                action.clone(),
            ),
        );
        f.evidence = vec![
            ev(
                lang.pick(
                    "Commit medio rispetto al limite",
                    "Average commit relative to the limit",
                ),
                fmt::pct(lang, commit.mean),
                commit_src,
            ),
            ev(
                lang.pick(
                    "Commit massimo rispetto al limite",
                    "Peak commit relative to the limit",
                ),
                fmt::pct(lang, commit.max),
                commit_src,
            ),
        ];
        f.evidence.extend(procs.iter().cloned());
        f.possible_causes = strings(lang.pick(
            &[
                "Programmi che riservano molta memoria",
                "File di paging di dimensione ridotta rispetto all'uso",
            ],
            &[
                "Programs that reserve a lot of memory",
                "A paging file that is small for the current usage",
            ],
        ));
        f.recommendation = recommendation(lang).into();
        f.limitations = strings(&[lang.pick(
            "Quando il commit raggiunge il limite Windows può segnalare memoria insufficiente; il valore da solo non indica rallentamenti.",
            "When the commit reaches the limit, Windows may report low memory; the value alone does not indicate slowdowns.",
        )]);
        if procs.is_empty() {
            f.limitations.push(no_procs_limitation());
        }
        out.findings.push(f);
    }

    if pages.is_sustained(ctx.window_s) {
        let corroborated = low_fired || commit_fired;
        let mut f = finding(
            "memory.paging.high",
            ComponentKind::Memory,
            if corroborated {
                Severity::Warning
            } else {
                Severity::Info
            },
            FindingKind::Hypothesis,
            if corroborated {
                Confidence::Medium
            } else {
                Confidence::Low
            },
            lang.pick("Paginazione su disco elevata", "High disk paging"),
            if corroborated {
                lang.pick(
                    "La paginazione da disco è stata elevata insieme a una pressione sulla memoria: è probabile che il sistema stia compensando la RAM insufficiente con il file di paging.",
                    "Disk paging was high together with memory pressure: the system is probably making up for insufficient RAM with the paging file.",
                )
            } else {
                lang.pick(
                    "La paginazione da disco è stata elevata, ma la memoria disponibile e il commit erano nella norma: può dipendere dalla lettura di file mappati in memoria più che da una carenza di RAM.",
                    "Disk paging was high, but available memory and commit were normal: it may come from reading memory-mapped files rather than from a lack of RAM.",
                )
            },
            if corroborated {
                simple(
                    lang.pick(
                        "Il PC probabilmente sta usando il disco per compensare la memoria piena",
                        "The PC is probably using the disk to make up for full memory",
                    ),
                    lang.pick(
                        "Windows ha spostato spesso dati tra la memoria e il disco mentre la memoria era quasi piena. Il disco è molto più lento della memoria, quindi questo potrebbe rendere il PC lento e poco reattivo.",
                        "Windows often moved data between memory and the disk while memory was almost full. The disk is much slower than memory, so this might make the PC slow and less responsive.",
                    ),
                    action.clone(),
                )
            } else {
                simple(
                    lang.pick(
                        "Molti scambi di dati tra memoria e disco, forse normali",
                        "A lot of data moving between memory and the disk, possibly normal",
                    ),
                    lang.pick(
                        "Windows ha letto spesso dati dal disco verso la memoria, ma la memoria non era piena. Potrebbe essere normale, ad esempio quando un programma apre file molto grandi, e non è detto che indichi un problema.",
                        "Windows often read data from the disk into memory, but memory wasn't full. This might be normal, for example when a program opens very large files, and it doesn't necessarily mean there's a problem.",
                    ),
                    lang.pick(
                        format!(
                            "Nessuna azione necessaria se il PC funziona bene. Se noti rallentamenti, ripeti la misurazione mentre il problema si presenta e controlla nella pagina Processi quali programmi sono attivi.{}",
                            top_sentence(lang, &top_names)
                        ),
                        format!(
                            "No action needed if the PC works well. If you notice slowdowns, measure again while the problem is happening and check on the Processes page which programs are active.{}",
                            top_sentence(lang, &top_names)
                        ),
                    ),
                )
            },
        );
        let high = fmt::num(lang, MEM_PAGES_HIGH_PER_SEC, 0);
        f.evidence = vec![
            ev(
                lang.pick("Pagine/s medie", "Average pages/s"),
                fmt::num(lang, pages.mean, 0),
                pages_src,
            ),
            ev(
                lang.pick("Pagine/s massime", "Peak pages/s"),
                fmt::num(lang, pages.max, 0),
                pages_src,
            ),
            ev(
                lang.pick(
                    format!("Campioni ≥ {high} pagine/s"),
                    format!("Samples ≥ {high} pages/s"),
                ),
                count_of(lang, pages.hits, pages.available),
                pages_src,
            ),
        ];
        f.evidence.extend(procs.iter().cloned());
        f.possible_causes = if corroborated {
            strings(&[lang.pick(
                "Memoria RAM insufficiente per il carico attuale",
                "Not enough RAM for the current workload",
            )])
        } else {
            strings(lang.pick(
                &[
                    "Programmi che leggono file di grandi dimensioni mappati in memoria",
                    "Memoria RAM insufficiente per il carico attuale",
                ],
                &[
                    "Programs reading large memory-mapped files",
                    "Not enough RAM for the current workload",
                ],
            ))
        };
        f.recommendation = recommendation(lang).into();
        f.limitations = strings(&[lang.pick(
            "Pages/sec include anche letture di file mappati in memoria, non solo l'uso del file di paging.",
            "Pages/sec also includes reads of memory-mapped files, not only paging file use.",
        )]);
        out.findings.push(f);
    }

    out.coverage.push(cov.finish());
    out
}

#[cfg(test)]
mod tests {
    use crate::evaluate;
    use crate::testdata::*;
    use models::component::ComponentKind;
    use models::diagnostics::{Confidence, CoverageStatus, FindingKind, Severity};
    use models::monitor::LiveSample;
    use models::{Source, Unit};

    fn with_available(n: usize, pct_of_16: f64) -> Vec<LiveSample> {
        live(n, |_, s| {
            s.memory.available_bytes =
                avail(16.0 * GIB * pct_of_16 / 100.0, Unit::Bytes, Source::Sysinfo);
        })
    }

    fn find<'a>(
        r: &'a models::diagnostics::DiagnosticReport,
        id: &str,
    ) -> Option<&'a models::diagnostics::Finding> {
        r.findings.iter().find(|f| f.rule_id == id)
    }

    #[test]
    fn low_available_fires_below_10_percent() {
        let s = with_available(60, 9.99);
        let snap = snapshot(vec![
            process("small.exe", 1.0, 0.5, 5.0),
            process("big.exe", 1.0, 6.0, 30.0),
        ]);
        let mut inp = input(&s);
        inp.processes = Some(&snap);
        let r = evaluate(&inp);
        let f = find(&r, "memory.available.low").expect("low");
        assert_eq!(f.severity, Severity::Warning);
        assert_eq!(f.kind, FindingKind::Observation);
        assert!(f.evidence.iter().any(|e| e.label.contains("big.exe")));
        assert_eq!(
            f.recommendation,
            "Verificare i processi con maggiore consumo e ripetere la misurazione durante il problema."
        );
    }

    #[test]
    fn low_available_does_not_fire_at_exactly_10_percent() {
        let r = evaluate(&input(&with_available(60, 10.0)));
        assert!(find(&r, "memory.available.low").is_none());
    }

    #[test]
    fn critical_below_5_percent() {
        let r = evaluate(&input(&with_available(60, 4.0)));
        let f = find(&r, "memory.available.low").expect("low");
        assert_eq!(f.severity, Severity::Critical);
        assert!(
            f.limitations
                .iter()
                .any(|l| l.contains("Classifica dei processi"))
        );
    }

    #[test]
    fn commit_high_boundary() {
        let mk = |pct: f64| {
            live(60, move |_, s| {
                s.memory.commit_used_bytes =
                    avail(32.0 * GIB * pct / 100.0, Unit::Bytes, Source::Pdh);
            })
        };
        assert!(find(&evaluate(&input(&mk(90.0))), "memory.commit.high").is_some());
        assert!(find(&evaluate(&input(&mk(89.9))), "memory.commit.high").is_none());
    }

    #[test]
    fn paging_alone_is_low_confidence_hypothesis() {
        let s = live(60, |_, s| {
            s.memory.pages_per_sec = avail(1000.0, Unit::Count, Source::Pdh);
        });
        let r = evaluate(&input(&s));
        let f = find(&r, "memory.paging.high").expect("paging");
        assert_eq!(f.kind, FindingKind::Hypothesis);
        assert_eq!(f.confidence, Confidence::Low);
        assert_eq!(f.severity, Severity::Info);

        let s = live(60, |_, s| {
            s.memory.pages_per_sec = avail(999.0, Unit::Count, Source::Pdh);
        });
        assert!(find(&evaluate(&input(&s)), "memory.paging.high").is_none());
    }

    #[test]
    fn paging_with_low_memory_is_medium_confidence() {
        let s = live(60, |_, s| {
            s.memory.pages_per_sec = avail(5000.0, Unit::Count, Source::Pdh);
            s.memory.available_bytes = avail(GIB * 0.5, Unit::Bytes, Source::Sysinfo);
        });
        let r = evaluate(&input(&s));
        let f = find(&r, "memory.paging.high").expect("paging");
        assert_eq!(f.confidence, Confidence::Medium);
        assert_eq!(f.severity, Severity::Warning);
    }

    #[test]
    fn memory_coverage_states() {
        let cov = |r: &models::diagnostics::DiagnosticReport| {
            r.coverage
                .iter()
                .find(|c| c.component == ComponentKind::Memory)
                .map(|c| c.status)
                .expect("cov")
        };
        assert_eq!(cov(&evaluate(&input(&[]))), CoverageStatus::NotEvaluated);
        assert_eq!(
            cov(&evaluate(&input(&live(20, |_, _| {})))),
            CoverageStatus::Partial
        );
        assert_eq!(
            cov(&evaluate(&input(&live(60, |_, _| {})))),
            CoverageStatus::Evaluated
        );
        let s = live(60, |_, s| s.memory.pages_per_sec = missing());
        assert_eq!(cov(&evaluate(&input(&s))), CoverageStatus::Partial);
    }

    #[test]
    fn english_texts() {
        let s = with_available(60, 3.0);
        let mut inp = input(&s);
        inp.language = models::settings::Language::English;
        let r = evaluate(&inp);
        let f = r
            .findings
            .iter()
            .find(|f| f.rule_id == "memory.available.low")
            .expect("finding");
        assert_eq!(f.severity, Severity::Critical);
        assert_eq!(f.title, "Low available memory");
        assert_eq!(
            f.description,
            "Available memory stayed below 10.0% of the total in 100.0% of the samples."
        );
        assert_eq!(f.simple.headline, "The PC almost ran out of free memory");
        assert!(
            f.simple
                .meaning
                .contains("on average, 97% of it was in use"),
            "{}",
            f.simple.meaning
        );
        assert_eq!(f.evidence[2].label, "Total memory");
        assert_eq!(f.evidence[2].value, "16.0 GiB");
        assert!(
            f.limitations
                .iter()
                .any(|l| l.starts_with("Process ranking not available"))
        );
    }
}
