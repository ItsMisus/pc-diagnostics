//! GPU rules (live telemetry). GPU driver problems are evaluated in `drivers`.

use models::Source;
use models::component::ComponentKind;
use models::diagnostics::{Confidence, FindingKind, Severity};
use models::hardware::GpuInfo;
use models::monitor::GpuSample;

use super::{Cov, Ctx, Outcome, count_of, ev, finding, no_samples, simple, strings, window_notes};
use crate::fmt;
use crate::window::{Sustained, val};

/// Busiest-engine utilization considered "saturated".
pub const GPU_SATURATED_PERCENT: f64 = 95.0;
/// Dedicated VRAM use (share of the adapter's dedicated memory) considered "high".
pub const GPU_VRAM_HIGH_PERCENT: f64 = 90.0;

pub fn evaluate(ctx: &Ctx) -> Outcome {
    let mut out = Outcome::default();
    let mut cov = Cov::new(ComponentKind::Gpu);
    let live = ctx.input.live;
    let lang = ctx.lang;
    let inv_gpus: Vec<&GpuInfo> = ctx
        .input
        .inventory
        .map(|i| {
            i.gpus
                .iter()
                .filter(|g| !g.is_virtual && !g.is_software)
                .collect()
        })
        .unwrap_or_default();

    // GPU ids in first-seen order (deterministic), excluding virtual/software adapters.
    let mut ids: Vec<&str> = Vec::new();
    for s in live {
        for g in &s.gpus {
            let excluded = ctx.input.inventory.is_some_and(|i| {
                i.gpus
                    .iter()
                    .any(|x| x.id == g.id && (x.is_virtual || x.is_software))
            });
            if !excluded && !ids.contains(&g.id.as_str()) {
                ids.push(&g.id);
            }
        }
    }

    if ids.is_empty() {
        if inv_gpus.is_empty() && ctx.input.inventory.is_some() {
            cov.not_evaluated(lang.pick("nessuna GPU fisica rilevata", "no physical GPU detected"));
        } else if live.is_empty() {
            cov.not_evaluated(no_samples(lang));
        } else {
            cov.not_evaluated(lang.pick(
                "nessun campione GPU disponibile",
                "no GPU samples available",
            ));
        }
        out.coverage.push(cov.finish());
        return out;
    }
    let w = fmt::seconds(ctx.window_s);
    cov.note(lang.pick(
        format!("utilizzo e memoria video valutati su {w}"),
        format!("usage and video memory evaluated over {w}"),
    ));
    window_notes(&mut cov, lang, ctx.window_s);

    for id in ids {
        let info = inv_gpus.iter().find(|g| g.id == id).copied();
        let name = info.map_or_else(|| id.to_string(), |g| g.name.clone());
        let pick = |s: &models::monitor::LiveSample| -> Option<GpuSample> {
            s.gpus.iter().find(|g| g.id == id).cloned()
        };

        let usage = Sustained::compute(
            live,
            |s| pick(s).and_then(|g| val(&g.usage_percent)),
            |v| v >= GPU_SATURATED_PERCENT,
        );
        if !usage.usable() {
            cov.partial(lang.pick(
                format!("utilizzo non disponibile per {name}"),
                format!("usage not available for {name}"),
            ));
        } else if usage.is_sustained(ctx.window_s) {
            let sat = fmt::pct(lang, GPU_SATURATED_PERCENT);
            let share = fmt::pct(lang, usage.hit_percent());
            let mut f = finding(
                "gpu.utilization.sustained",
                ComponentKind::Gpu,
                Severity::Info,
                FindingKind::Observation,
                Confidence::High,
                lang.pick(
                    format!("GPU satura in modo prolungato: {name}"),
                    format!("Sustained GPU saturation: {name}"),
                ),
                lang.pick(
                    format!("Il motore più impegnato della GPU è stato pari o superiore a {sat} nel {share} dei campioni."),
                    format!("The GPU's busiest engine was at or above {sat} in {share} of the samples."),
                ),
                simple(
                    lang.pick(
                        format!("La scheda video ha lavorato al massimo per molto tempo: {name}"),
                        format!("The graphics card worked at full capacity for a long time: {name}"),
                    ),
                    lang.pick(
                        "La scheda video (la parte del PC che si occupa di grafica, giochi e video) è stata usata quasi al massimo per gran parte del tempo osservato. Durante un gioco o un programma di grafica è normale e non indica un problema.",
                        "The graphics card (the part of the PC that handles graphics, games, and video) was used at nearly full capacity for most of the time observed. During a game or a graphics program, this is normal and doesn't mean there's a problem.",
                    ),
                    lang.pick(
                        "Nessuna azione necessaria se stavi usando un gioco o un programma di grafica. Altrimenti controlla quali programmi sono aperti e chiudi quelli che non ti servono.",
                        "No action needed if you were playing a game or using a graphics program. Otherwise, check which programs are open and close the ones you don't need.",
                    ),
                ),
            );
            f.evidence = vec![
                ev(
                    lang.pick("Utilizzo medio GPU", "Average GPU usage"),
                    fmt::pct(lang, usage.mean),
                    Source::Pdh,
                ),
                ev(
                    lang.pick(format!("Campioni ≥ {sat}"), format!("Samples ≥ {sat}")),
                    count_of(lang, usage.hits, usage.available),
                    Source::Pdh,
                ),
            ];
            f.possible_causes = strings(lang.pick(
                &[
                    "Un gioco, un'applicazione 3D o un calcolo su GPU in esecuzione (comportamento normale)",
                    "Un'applicazione che usa la GPU in background",
                ],
                &[
                    "A game, 3D application or GPU computation running (normal behavior)",
                    "An application using the GPU in the background",
                ],
            ));
            f.recommendation = lang.pick(
                "Se il carico corrisponde a un'attività voluta non è necessaria alcuna azione; altrimenti verificare quali applicazioni usano la GPU.",
                "If the load matches an intended task, no action is needed; otherwise, check which applications use the GPU.",
            ).into();
            f.limitations = strings(lang.pick(
                &[
                    "Una GPU al 100 % durante un gioco è normale e non indica un problema.",
                    "L'utilizzo è quello del motore più impegnato (convenzione di Gestione attività).",
                ],
                &[
                    "A GPU at 100% during a game is normal and does not indicate a problem.",
                    "Usage is that of the busiest engine (the Task Manager convention).",
                ],
            ));
            out.findings.push(f);
        }

        match info.map(|g| g.dedicated_vram_bytes).filter(|v| *v > 0) {
            None => cov.partial(lang.pick(
                format!("memoria video dedicata totale non nota per {name}: uso VRAM non valutato"),
                format!(
                    "total dedicated video memory unknown for {name}: VRAM usage not evaluated"
                ),
            )),
            Some(total) => {
                let total = total as f64;
                let vram = Sustained::compute(
                    live,
                    |s| {
                        pick(s)
                            .and_then(|g| val(&g.dedicated_used_bytes))
                            .map(|u| u / total * 100.0)
                    },
                    |v| v >= GPU_VRAM_HIGH_PERCENT,
                );
                if vram.is_sustained(ctx.window_s) {
                    let high = fmt::pct(lang, GPU_VRAM_HIGH_PERCENT);
                    let share = fmt::pct(lang, vram.hit_percent());
                    let total_plain = fmt::bytes_plain(lang, total);
                    let mean_plain = fmt::il_pct(lang, vram.mean);
                    let used = fmt::bytes(lang, total * vram.mean / 100.0);
                    let (total_tech, mean_tech) =
                        (fmt::bytes(lang, total), fmt::pct(lang, vram.mean));
                    let mut f = finding(
                        "gpu.vram.high",
                        ComponentKind::Gpu,
                        Severity::Warning,
                        FindingKind::Observation,
                        Confidence::High,
                        lang.pick(
                            format!("Memoria video quasi esaurita: {name}"),
                            format!("Video memory almost full: {name}"),
                        ),
                        lang.pick(
                            format!("La memoria video dedicata usata è stata pari o superiore al {high} nel {share} dei campioni."),
                            format!("Dedicated video memory in use was at or above {high} in {share} of the samples."),
                        ),
                        simple(
                            lang.pick(
                                format!("La memoria della scheda video è quasi piena: {name}"),
                                format!("The graphics card's memory is almost full: {name}"),
                            ),
                            lang.pick(
                                format!("La scheda video ha una memoria tutta sua ({total_plain}), che per gran parte del tempo osservato è stata quasi piena (in media {mean_plain}). Quando è piena, giochi e programmi di grafica possono avere scatti o rallentamenti; alcuni programmi però la occupano anche se non la usano tutta."),
                                format!("The graphics card has its own memory ({total_plain}), which was almost full for most of the time observed ({mean_plain} on average). When it's full, games and graphics programs can stutter or slow down. However, some programs reserve it even if they don't use all of it."),
                            ),
                            lang.pick(
                                "Se noti scatti nei giochi o nei programmi di grafica, abbassa la qualità grafica o la risoluzione nelle impostazioni del programma e ripeti la misurazione. Chiudere gli altri programmi che usano la grafica può aiutare.",
                                "If you notice stuttering in games or graphics programs, lower the graphics quality or resolution in the program's settings and measure again. Closing other programs that use graphics can help.",
                            ),
                        ),
                    );
                    f.evidence = vec![
                        ev(
                            lang.pick("VRAM usata (media)", "VRAM used (average)"),
                            lang.pick(
                                format!("{used} di {total_tech} ({mean_tech})"),
                                format!("{used} of {total_tech} ({mean_tech})"),
                            ),
                            Source::Pdh,
                        ),
                        ev(
                            lang.pick("VRAM usata (massimo)", "VRAM used (peak)"),
                            fmt::pct(lang, vram.max),
                            Source::Pdh,
                        ),
                    ];
                    f.possible_causes = strings(lang.pick(
                        &[
                            "Impostazioni grafiche (risoluzione, qualità delle texture) elevate per la memoria disponibile",
                            "Più applicazioni che usano la GPU contemporaneamente",
                        ],
                        &[
                            "Graphics settings (resolution, texture quality) too high for the available memory",
                            "Several applications using the GPU at the same time",
                        ],
                    ));
                    f.recommendation = lang.pick(
                        "Se si notano scatti o rallentamenti nelle applicazioni grafiche, ridurre la qualità delle texture o la risoluzione e ripetere la misurazione.",
                        "If graphics applications stutter or slow down, lower the texture quality or resolution and measure again.",
                    ).into();
                    f.limitations = strings(&[lang.pick(
                        "Alcune applicazioni riservano la memoria video disponibile anche se non la usano tutta.",
                        "Some applications reserve the available video memory even if they don't use all of it.",
                    )]);
                    out.findings.push(f);
                }
            }
        }

        let temp = live
            .iter()
            .filter_map(&pick)
            .any(|g| val(&g.temperature_c).is_some());
        if !temp {
            cov.partial(lang.pick(
                format!("sensori GPU (temperatura) non disponibili per {name}"),
                format!("GPU sensors (temperature) not available for {name}"),
            ));
        }
    }

    out.coverage.push(cov.finish());
    out
}

#[cfg(test)]
mod tests {
    use crate::evaluate;
    use crate::testdata::*;
    use models::component::ComponentKind;
    use models::diagnostics::{CoverageStatus, DiagnosticReport};
    use models::{Source, Unit};

    fn has(r: &DiagnosticReport, id: &str) -> bool {
        r.findings.iter().any(|f| f.rule_id == id)
    }

    fn gpu_cov(r: &DiagnosticReport) -> (CoverageStatus, String) {
        r.coverage
            .iter()
            .find(|c| c.component == ComponentKind::Gpu)
            .map(|c| (c.status, c.note.clone()))
            .expect("cov")
    }

    #[test]
    fn saturation_boundaries() {
        let inv = inventory();
        let mk = |v: f64| {
            live(60, move |_, s| {
                s.gpus[0].usage_percent = avail(v, Unit::Percent, Source::Pdh);
            })
        };
        let s = mk(95.0);
        let mut inp = input(&s);
        inp.inventory = Some(&inv);
        let r = evaluate(&inp);
        assert!(has(&r, "gpu.utilization.sustained"));
        assert!(r.findings.iter().any(|f| f.title.contains("Test GPU 8GB")));
        let s = mk(94.9);
        let mut inp = input(&s);
        inp.inventory = Some(&inv);
        assert!(!has(&evaluate(&inp), "gpu.utilization.sustained"));
    }

    #[test]
    fn vram_boundaries_and_requires_inventory() {
        let inv = inventory();
        let mk = |gib: f64| {
            live(60, move |_, s| {
                s.gpus[0].dedicated_used_bytes = avail(gib * GIB, Unit::Bytes, Source::Pdh);
            })
        };
        let s = mk(7.2); // exactly 90 % of 8 GiB
        let mut inp = input(&s);
        inp.inventory = Some(&inv);
        assert!(has(&evaluate(&inp), "gpu.vram.high"));
        let s = mk(7.1);
        let mut inp = input(&s);
        inp.inventory = Some(&inv);
        assert!(!has(&evaluate(&inp), "gpu.vram.high"));
        // Without the inventory the total is unknown: not evaluated, noted.
        let s = mk(7.9);
        let r = evaluate(&input(&s));
        assert!(!has(&r, "gpu.vram.high"));
        let (status, note) = gpu_cov(&r);
        assert_eq!(status, CoverageStatus::Partial);
        assert!(note.contains("VRAM"));
    }

    #[test]
    fn missing_sensors_make_gpu_partial() {
        let inv = inventory();
        let s = live(60, |_, _| {});
        let mut inp = input(&s);
        inp.inventory = Some(&inv);
        let (status, note) = gpu_cov(&evaluate(&inp));
        assert_eq!(status, CoverageStatus::Partial);
        assert!(note.contains("sensori GPU"));

        let s = live(60, |_, s| {
            s.gpus[0].temperature_c = avail(55.0, Unit::Celsius, Source::Nvml);
        });
        let mut inp = input(&s);
        inp.inventory = Some(&inv);
        assert_eq!(gpu_cov(&evaluate(&inp)).0, CoverageStatus::Evaluated);
    }

    #[test]
    fn virtual_adapters_are_ignored_and_no_gpu_is_not_evaluated() {
        let mut inv = inventory();
        inv.gpus[0].is_virtual = true;
        let s = live(60, |_, s| {
            s.gpus[0].usage_percent = avail(100.0, Unit::Percent, Source::Pdh);
        });
        let mut inp = input(&s);
        inp.inventory = Some(&inv);
        let r = evaluate(&inp);
        assert!(!has(&r, "gpu.utilization.sustained"));
        assert_eq!(gpu_cov(&r).0, CoverageStatus::NotEvaluated);
    }

    #[test]
    fn english_texts() {
        let inv = inventory();
        let s = live(60, |_, s| {
            s.gpus[0].usage_percent = avail(99.0, Unit::Percent, Source::Pdh);
        });
        let mut inp = input(&s);
        inp.inventory = Some(&inv);
        inp.language = models::settings::Language::English;
        let r = evaluate(&inp);
        let f = r
            .findings
            .iter()
            .find(|f| f.rule_id == "gpu.utilization.sustained")
            .expect("finding");
        assert_eq!(f.title, "Sustained GPU saturation: Test GPU 8GB");
        assert_eq!(
            f.description,
            "The GPU's busiest engine was at or above 95.0% in 100.0% of the samples."
        );
        assert_eq!(
            f.simple.headline,
            "The graphics card worked at full capacity for a long time: Test GPU 8GB"
        );
        let (_, note) = gpu_cov(&r);
        assert!(
            note.ends_with("GPU sensors (temperature) not available for Test GPU 8GB"),
            "{note}"
        );
    }
}
