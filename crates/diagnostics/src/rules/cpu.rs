//! CPU rules.

use models::Source;
use models::benchmark::{BenchmarkKind, BenchmarkState};
use models::benchmark_run::{BenchmarkResult, PowerSource};
use models::component::ComponentKind;
use models::diagnostics::{Confidence, FindingKind, Severity};

use super::{Cov, Ctx, Outcome, count_of, ev, finding, no_samples, simple, strings, window_notes};
use crate::fmt;
use crate::window::{Sustained, src, val};

/// Whole-machine utilization considered "high".
pub const CPU_HIGH_PERCENT: f64 = 85.0;
/// A process is "dominant" if it uses at least this share of the whole machine…
pub const CPU_DOMINANT_MIN_PERCENT: f64 = 25.0;
/// …and at least this fraction of the observed mean total utilization (when known).
pub const CPU_DOMINANT_MIN_SHARE: f64 = 0.5;
/// The OS limit counts as "below max" when limit < max × this ratio.
pub const CPU_LIMIT_RATIO: f64 = 0.95;
/// Minimum samples with both limit and max available.
pub const CPU_LIMIT_MIN_SAMPLES: usize = 10;
/// Share of those samples (percent) that must show the limit.
pub const CPU_LIMIT_SHARE_PERCENT: usize = 50;
/// Throughput drop (early vs late part of a CpuSustained run) that triggers the hypothesis.
pub const CPU_SUSTAINED_DROP_PERCENT: f64 = 15.0;
/// Minimum series points to compute the drop from the `throughput` series.
pub const CPU_SUSTAINED_MIN_POINTS: usize = 10;
/// Preferred metric id for the drop, if the benchmark publishes it.
pub const SUSTAINED_DROP_METRIC_ID: &str = "throughput_drop_percent";
/// Series id used as a fallback.
pub const THROUGHPUT_SERIES_ID: &str = "throughput";
pub const CPU_MHZ_SERIES_ID: &str = "cpu_mhz";

pub fn evaluate(ctx: &Ctx) -> Outcome {
    let mut out = Outcome::default();
    let mut cov = Cov::new(ComponentKind::Cpu);
    let live = ctx.input.live;
    let lang = ctx.lang;

    let usage = Sustained::compute(
        live,
        |s| val(&s.cpu.usage_percent),
        |v| v >= CPU_HIGH_PERCENT,
    );
    let usage_src = live
        .iter()
        .find_map(|s| src(&s.cpu.usage_percent))
        .unwrap_or(Source::Pdh);

    if !usage.usable() {
        cov.not_evaluated(if live.is_empty() {
            no_samples(lang)
        } else {
            lang.pick(
                "utilizzo CPU non disponibile nella maggior parte dei campioni",
                "CPU usage unavailable in most samples",
            )
        });
    } else {
        let w = fmt::seconds(ctx.window_s);
        cov.note(lang.pick(
            format!("utilizzo e limite di frequenza valutati su {w}"),
            format!("usage and frequency limit evaluated over {w}"),
        ));
        window_notes(&mut cov, lang, ctx.window_s);
        if usage.is_sustained(ctx.window_s) {
            let high = fmt::pct(lang, CPU_HIGH_PERCENT);
            let share = fmt::pct(lang, usage.hit_percent());
            let share_plain = fmt::pct_plain(lang, usage.hit_percent());
            let mut f = finding(
                "cpu.utilization.sustained",
                ComponentKind::Cpu,
                Severity::Warning,
                FindingKind::Observation,
                Confidence::High,
                lang.pick("Utilizzo CPU elevato e prolungato", "Sustained high CPU usage"),
                lang.pick(
                    format!("L'utilizzo complessivo della CPU è stato pari o superiore a {high} nel {share} dei campioni della finestra osservata."),
                    format!("Overall CPU usage was at or above {high} in {share} of the samples in the observed window."),
                ),
                simple(
                    lang.pick(
                        "Il processore è stato molto impegnato per gran parte del tempo",
                        "The processor was very busy most of the time",
                    ),
                    lang.pick(
                        format!("Per gran parte del tempo osservato ({share_plain} delle misurazioni) il processore (la parte del PC che esegue i programmi) ha lavorato quasi al massimo. In questi momenti i programmi possono rispondere più lentamente; durante un lavoro pesante, come un gioco o l'esportazione di un video, è normale."),
                        format!("For most of the time observed ({share_plain} of the readings), the processor (the part of the PC that runs programs) worked at nearly full capacity. At times like these, programs can respond more slowly. During heavy work, such as a game or a video export, this is normal."),
                    ),
                    lang.pick(
                        "Apri la pagina Processi per vedere quali programmi lavorano di più. Se è un'attività che hai avviato tu, non serve fare nulla; altrimenti chiudi i programmi che non stai usando.",
                        "Open the Processes page to see which programs are working the hardest. If it's something you started yourself, there's nothing to do; otherwise, close the programs you aren't using.",
                    ),
                ),
            );
            f.evidence = vec![
                ev(
                    lang.pick("Utilizzo medio CPU", "Average CPU usage"),
                    fmt::pct(lang, usage.mean),
                    usage_src,
                ),
                ev(
                    lang.pick("Utilizzo massimo CPU", "Peak CPU usage"),
                    fmt::pct(lang, usage.max),
                    usage_src,
                ),
                ev(
                    lang.pick(format!("Campioni ≥ {high}"), format!("Samples ≥ {high}")),
                    count_of(lang, usage.hits, usage.available),
                    usage_src,
                ),
                ev(
                    lang.pick("Finestra osservata", "Observed window"),
                    fmt::seconds(ctx.window_s),
                    usage_src,
                ),
            ];
            f.possible_causes = strings(lang.pick(
                &[
                    "Un'attività impegnativa in corso (compilazione, rendering, giochi, compressione)",
                    "Processi in background (aggiornamenti, indicizzazione, scansioni antivirus)",
                ],
                &[
                    "A demanding task in progress (compiling, rendering, gaming, compression)",
                    "Background processes (updates, indexing, antivirus scans)",
                ],
            ));
            f.recommendation = lang.pick(
                "Verificare nella pagina Processi quali programmi usano la CPU durante il problema. Se il carico corrisponde a un'attività voluta non è necessaria alcuna azione.",
                "Check on the Processes page which programs use the CPU while the problem occurs. If the load matches an intended task, no action is needed.",
            ).into();
            f.limitations = strings(&[lang.pick(
                "L'utilizzo indica quanto la CPU è occupata, non che il sistema sia lento.",
                "Usage shows how busy the CPU is, not that the system is slow.",
            )]);
            out.findings.push(f);
        }
    }

    dominant_process(ctx, &usage, &mut out);
    frequency_limit(ctx, &mut out, &mut cov);

    let temp_available = live.iter().any(|s| val(&s.cpu.temperature_c).is_some());
    if !live.is_empty() && !temp_available {
        cov.partial(lang.pick(
            "temperatura CPU non disponibile",
            "CPU temperature not available",
        ));
        out.limitations.push(
            lang.pick(
                "Temperatura CPU non disponibile: non è possibile confermare o escludere una limitazione termica.",
                "CPU temperature not available: thermal throttling can be neither confirmed nor ruled out.",
            )
            .into(),
        );
    }

    sustained_benchmark(ctx, temp_available, &mut out, &mut cov);

    out.coverage.push(cov.finish());
    out
}

fn dominant_process(ctx: &Ctx, usage: &Sustained, out: &mut Outcome) {
    let Some(snap) = ctx.input.processes else {
        return;
    };
    let lang = ctx.lang;
    let top = snap
        .ranking_window
        .iter()
        .filter_map(|r| val(&r.cpu_percent).map(|c| (r, c)))
        .max_by(|a, b| a.1.total_cmp(&b.1).then_with(|| b.0.name.cmp(&a.0.name)));
    let Some((row, cpu)) = top else {
        return;
    };
    if cpu < CPU_DOMINANT_MIN_PERCENT {
        return;
    }
    let share_known = usage.usable() && usage.mean > 0.0;
    if share_known && cpu < usage.mean * CPU_DOMINANT_MIN_SHARE {
        return;
    }
    let source = src(&row.cpu_percent).unwrap_or(Source::Pdh);
    let name = &row.name;
    let cpu_pct = fmt::pct(lang, cpu);
    let cpu_plain = fmt::il_pct(lang, cpu);
    let self_note = if row.is_self {
        lang.pick(
            " È questa stessa applicazione (PC Diagnostics).",
            " It's this app itself (PC Diagnostics).",
        )
    } else {
        ""
    };
    let mut f = finding(
        "cpu.process.dominant",
        ComponentKind::Cpu,
        Severity::Info,
        FindingKind::Observation,
        Confidence::Medium,
        lang.pick(
            format!("Processo con uso CPU predominante: {name}"),
            format!("Process with dominant CPU usage: {name}"),
        ),
        lang.pick(
            format!("Il processo «{name}» usa {cpu_pct} della capacità totale della CPU nella classifica della finestra di analisi dei processi."),
            format!("The process “{name}” uses {cpu_pct} of the total CPU capacity in the ranking for the process analysis window."),
        ),
        simple(
            lang.pick(
                format!("Un programma sta usando molto il processore: {name}"),
                format!("A program is using a lot of the processor: {name}"),
            ),
            lang.pick(
                format!("Il programma «{name}» da solo ha usato circa {cpu_plain} della capacità del processore nell'ultimo periodo analizzato. Può essere normale se lo stai usando per un lavoro pesante; se non lo stai usando, conviene controllare.{self_note}"),
                format!("On its own, the program “{name}” used about {cpu_plain} of the processor's capacity in the last period analyzed. That can be normal if you're using it for heavy work; if you aren't using it, it's worth checking.{self_note}"),
            ),
            lang.pick(
                "Controlla se è un programma che stai usando. Se non lo è, guarda le sue impostazioni oppure chiudilo dal programma stesso. Non chiudere i programmi di sistema di Windows.",
                "Check whether it's a program you're using. If it isn't, look at its settings or close it from within the program. Don't close Windows system programs.",
            ),
        ),
    );
    f.evidence.push(ev(
        lang.pick(
            format!("CPU usata da {name}"),
            format!("CPU used by {name}"),
        ),
        cpu_pct,
        source,
    ));
    if share_known {
        f.evidence.push(ev(
            lang.pick(
                "Utilizzo medio CPU complessivo",
                "Average overall CPU usage",
            ),
            fmt::pct(lang, usage.mean),
            Source::Pdh,
        ));
    }
    f.evidence.push(ev(
        lang.pick("Finestra di analisi processi", "Process analysis window"),
        fmt::seconds(snap.window_s),
        source,
    ));
    f.possible_causes = strings(lang.pick(
        &[
            "Il programma sta svolgendo un lavoro intenso previsto",
            "Il programma è bloccato in un ciclo o sta eseguendo attività in background",
        ],
        &[
            "The program is doing expected heavy work",
            "The program is stuck in a loop or running background tasks",
        ],
    ));
    f.recommendation = lang.pick(
        "Verificare se l'attività di questo programma è attesa. Se non lo è, controllarne lo stato o le impostazioni nel programma stesso.",
        "Check whether this program's activity is expected. If it isn't, check its status or settings in the program itself.",
    ).into();
    f.limitations = strings(&[lang.pick(
        "Il valore descrive la finestra di analisi dei processi, non necessariamente il momento del problema.",
        "The value describes the process analysis window, not necessarily the moment of the problem.",
    )]);
    if row.is_self {
        f.limitations.push(
            lang.pick(
                "Il processo appartiene a questa applicazione (PC Diagnostics).",
                "The process belongs to this app (PC Diagnostics).",
            )
            .into(),
        );
    }
    if !share_known {
        f.limitations.push(
            lang.pick(
                "Utilizzo CPU complessivo non disponibile: la quota relativa non è stata verificata.",
                "Overall CPU usage not available: the relative share was not checked.",
            )
            .into(),
        );
    }
    out.findings.push(f);
}

fn frequency_limit(ctx: &Ctx, out: &mut Outcome, cov: &mut Cov) {
    let lang = ctx.lang;
    let pairs: Vec<(f64, f64)> = ctx
        .input
        .live
        .iter()
        .filter_map(|s| Some((val(&s.cpu.limit_mhz)?, val(&s.cpu.max_mhz)?)))
        .filter(|(_, max)| *max > 0.0)
        .collect();
    if pairs.len() < CPU_LIMIT_MIN_SAMPLES {
        if !ctx.input.live.is_empty() {
            cov.partial(lang.pick(
                "limite di frequenza della CPU non disponibile o con troppi pochi campioni",
                "CPU frequency limit unavailable or with too few samples",
            ));
        }
        return;
    }
    let limited: Vec<&(f64, f64)> = pairs
        .iter()
        .filter(|(lim, max)| *lim < max * CPU_LIMIT_RATIO)
        .collect();
    if limited.len() * 100 < pairs.len() * CPU_LIMIT_SHARE_PERCENT {
        return;
    }
    let min_limit = limited.iter().map(|p| p.0).fold(f64::INFINITY, f64::min);
    let max_mhz = pairs.iter().map(|p| p.1).fold(0.0, f64::max);
    let share = limited.len() as f64 * 100.0 / pairs.len() as f64;
    let evidence = vec![
        ev(
            lang.pick("Limite minimo segnalato", "Lowest reported limit"),
            fmt::mhz(lang, min_limit),
            Source::PowerApi,
        ),
        ev(
            lang.pick("Frequenza massima", "Maximum frequency"),
            fmt::mhz(lang, max_mhz),
            Source::PowerApi,
        ),
        ev(
            lang.pick("Campioni con limite attivo", "Samples with an active limit"),
            format!(
                "{} ({})",
                count_of(lang, limited.len(), pairs.len()),
                fmt::pct(lang, share)
            ),
            Source::PowerApi,
        ),
    ];
    let lowest = fmt::il_pct(lang, min_limit / max_mhz * 100.0);
    let mut f = finding(
        "cpu.frequency.limit_reported",
        ComponentKind::Cpu,
        Severity::Info,
        FindingKind::Observation,
        Confidence::High,
        lang.pick("Limite di frequenza segnalato", "Frequency limit reported"),
        lang.pick(
            "Windows ha segnalato un limite di frequenza della CPU inferiore al massimo durante la finestra osservata. Il dato indica la presenza di un limite, non la sua causa.",
            "Windows reported a CPU frequency limit below the maximum during the observed window. This shows that a limit was present, not what caused it.",
        ),
        simple(
            lang.pick(
                "Windows ha tenuto il processore sotto la sua velocità massima",
                "Windows kept the processor below its top speed",
            ),
            lang.pick(
                format!("Per almeno metà del tempo osservato Windows non ha lasciato arrivare il processore alla sua velocità massima (nel momento più basso poteva arrivare solo a circa {lowest} del massimo). Nei lavori pesanti il PC può quindi essere un po' più lento; Windows non indica il motivo."),
                format!("For at least half of the time observed, Windows didn't let the processor reach its top speed (at the lowest point, it could only reach about {lowest} of the maximum). So the PC may be a little slower during heavy work. Windows doesn't say why."),
            ),
            lang.pick(
                "Nessuna azione necessaria se il PC ti sembra abbastanza veloce. Altrimenti leggi il risultato sulle possibili cause di questo limite.",
                "No action needed if the PC seems fast enough to you. Otherwise, read the result about the possible causes of this limit.",
            ),
        ),
    );
    f.evidence = evidence.clone();
    f.recommendation = lang.pick(
        "Nessuna azione necessaria se le prestazioni sono adeguate. Vedere l'ipotesi associata per le possibili cause.",
        "No action needed if performance is adequate. See the related hypothesis for possible causes.",
    ).into();
    f.limitations = strings(&[lang.pick(
        "Windows riporta il limite ma non il motivo.",
        "Windows reports the limit but not the reason.",
    )]);
    out.findings.push(f);

    let on_battery = latest(ctx.input.benchmarks, None)
        .is_some_and(|b| b.context.power_source == PowerSource::Battery);
    let battery_note = if on_battery {
        lang.pick(
            " Durante l'ultimo test il PC era alimentato a batteria.",
            " During the last test, the PC was running on battery.",
        )
    } else {
        ""
    };
    let mut h = finding(
        "cpu.frequency.limit_cause",
        ComponentKind::Cpu,
        Severity::Info,
        FindingKind::Hypothesis,
        Confidence::Low,
        lang.pick(
            "Possibile causa del limite di frequenza",
            "Possible cause of the frequency limit",
        ),
        lang.pick(
            "Il limite di frequenza può dipendere dalla modalità di alimentazione, dall'alimentazione a batteria o da una protezione termica. I dati disponibili non permettono di stabilire quale.",
            "The frequency limit may come from the power mode, running on battery, or thermal protection. The available data cannot tell which.",
        ),
        simple(
            lang.pick(
                "Il processore potrebbe essere rallentato per risparmiare energia o per il calore",
                "The processor may have been slowed down to save power or because of heat",
            ),
            lang.pick(
                format!("Non è possibile sapere con certezza perché il processore è stato limitato: potrebbe dipendere dalla modalità di risparmio energetico, dal funzionamento a batteria, dal PC che si scalda troppo o da impostazioni del produttore.{battery_note}"),
                format!("There's no way to know for sure why the processor was limited. It could be the power-saving mode, running on battery, the PC getting too hot, or the manufacturer's settings.{battery_note}"),
            ),
            lang.pick(
                "Apri Impostazioni > Sistema > Alimentazione e controlla la modalità di alimentazione. Se è un portatile, collegalo alla corrente e ripeti la misurazione.",
                "Open Settings > System > Power & battery and check the power mode. If it's a laptop, plug it in and measure again.",
            ),
        ),
    );
    h.evidence = evidence;
    if on_battery {
        h.evidence.push(ev(
            lang.pick(
                "Alimentazione all'ultimo benchmark",
                "Power source at the last benchmark",
            ),
            lang.pick("batteria", "battery"),
            Source::PowerApi,
        ));
    }
    h.possible_causes = strings(lang.pick(
        &[
            "Modalità di alimentazione orientata al risparmio energetico",
            "Funzionamento a batteria",
            "Limitazione termica (temperatura elevata)",
            "Impostazioni del firmware o del produttore",
        ],
        &[
            "Power mode set to save energy",
            "Running on battery",
            "Thermal throttling (high temperature)",
            "Firmware or manufacturer settings",
        ],
    ));
    h.recommendation = lang.pick(
        "Verificare la modalità di alimentazione in Impostazioni > Sistema > Alimentazione e, sui portatili, ripetere la misurazione collegati alla rete elettrica.",
        "Check the power mode in Settings > System > Power & battery and, on laptops, repeat the measurement while plugged in.",
    ).into();
    h.limitations = strings(&[lang.pick(
        "Ipotesi a bassa confidenza: la causa del limite non è misurabile con i dati disponibili.",
        "Low-confidence hypothesis: the cause of the limit cannot be measured with the available data.",
    )]);
    out.findings.push(h);
}

/// Newest result (by `started_at`) of `kind` (any kind if `None`) that completed.
pub fn latest(
    benchmarks: &[BenchmarkResult],
    kind: Option<BenchmarkKind>,
) -> Option<&BenchmarkResult> {
    benchmarks
        .iter()
        .filter(|b| b.state == BenchmarkState::Completed && kind.is_none_or(|k| b.kind == k))
        .max_by(|a, b| {
            a.started_at
                .cmp(&b.started_at)
                .then_with(|| b.run_id.cmp(&a.run_id))
        })
}

/// Drop in percent between the early and late part of a series (positive = slower).
pub fn series_drop(points: &[f64]) -> Option<(f64, f64, f64)> {
    if points.len() < CPU_SUSTAINED_MIN_POINTS {
        return None;
    }
    let k = (points.len() / 5).max(2);
    let early = points[..k].iter().sum::<f64>() / k as f64;
    let late = points[points.len() - k..].iter().sum::<f64>() / k as f64;
    (early > 0.0).then(|| (early, late, (early - late) / early * 100.0))
}

fn sustained_benchmark(ctx: &Ctx, temp_available: bool, out: &mut Outcome, cov: &mut Cov) {
    let lang = ctx.lang;
    let Some(b) = latest(ctx.input.benchmarks, Some(BenchmarkKind::CpuSustained)) else {
        cov.note(lang.pick(
            "nessun benchmark CPU prolungato disponibile",
            "no sustained CPU benchmark available",
        ));
        return;
    };
    let series_values = |id: &str| -> Option<Vec<f64>> {
        b.series
            .iter()
            .find(|s| s.id == id)
            .map(|s| s.points.iter().map(|p| p.value).collect())
    };
    let mut evidence = Vec::new();
    let drop = if let Some(m) = b.metrics.iter().find(|m| m.id == SUSTAINED_DROP_METRIC_ID) {
        Some(m.value)
    } else if let Some((early, late, d)) =
        series_values(THROUGHPUT_SERIES_ID).and_then(|p| series_drop(&p))
    {
        evidence.push(ev(
            lang.pick(
                "Throughput iniziale (media)",
                "Initial throughput (average)",
            ),
            format!("{} op/s", fmt::num(lang, early, 0)),
            Source::Benchmark,
        ));
        evidence.push(ev(
            lang.pick("Throughput finale (media)", "Final throughput (average)"),
            format!("{} op/s", fmt::num(lang, late, 0)),
            Source::Benchmark,
        ));
        Some(d)
    } else {
        None
    };
    let Some(drop) = drop else {
        cov.note(lang.pick(
            "l'ultimo benchmark CPU prolungato non contiene dati sul calo nel tempo",
            "the latest sustained CPU benchmark has no data on the drop over time",
        ));
        return;
    };
    if drop < CPU_SUSTAINED_DROP_PERCENT {
        return;
    }
    evidence.insert(
        0,
        ev(
            lang.pick(
                "Calo delle prestazioni durante il test",
                "Performance drop during the test",
            ),
            fmt::pct(lang, drop),
            Source::Benchmark,
        ),
    );
    if let Some((early, late, _)) = series_values(CPU_MHZ_SERIES_ID).and_then(|p| series_drop(&p)) {
        evidence.push(ev(
            lang.pick(
                "Frequenza CPU iniziale → finale",
                "CPU frequency, start → end",
            ),
            format!("{} → {}", fmt::mhz(lang, early), fmt::mhz(lang, late)),
            Source::Benchmark,
        ));
    }
    evidence.push(ev(
        lang.pick("Durata del test", "Test duration"),
        fmt::seconds(u32::try_from(b.duration_ms / 1000).unwrap_or(u32::MAX)),
        Source::Benchmark,
    ));
    if b.context.power_source == PowerSource::Battery {
        evidence.push(ev(
            lang.pick("Alimentazione", "Power source"),
            lang.pick("batteria", "battery"),
            Source::PowerApi,
        ));
    }
    let drop_pct = fmt::pct(lang, drop);
    let drop_plain = fmt::il_pct(lang, drop);
    let no_temp_note = if temp_available {
        ""
    } else {
        lang.pick(
            " Senza la temperatura del processore non si può capire quale sia il motivo.",
            " Without the processor's temperature, there's no way to tell which it is.",
        )
    };
    let mut f = finding(
        "cpu.benchmark.sustained_drop",
        ComponentKind::Cpu,
        Severity::Warning,
        FindingKind::Hypothesis,
        Confidence::Medium,
        lang.pick(
            "Calo delle prestazioni CPU sotto carico prolungato",
            "CPU performance drop under sustained load",
        ),
        lang.pick(
            format!("Nell'ultimo benchmark CPU prolungato le prestazioni sono calate del {drop_pct} tra la parte iniziale e quella finale del test. Un calo di questo tipo è compatibile con una limitazione termica o di potenza."),
            format!("In the latest sustained CPU benchmark, performance dropped by {drop_pct} between the beginning and the end of the test. A drop like this is consistent with thermal or power throttling."),
        ),
        simple(
            lang.pick(
                "Il processore potrebbe rallentare quando lavora a lungo",
                "The processor might slow down when it works for a long time",
            ),
            lang.pick(
                format!("Nell'ultimo test lungo il processore è andato più piano verso la fine (circa {drop_plain} in meno rispetto all'inizio). Potrebbe dipendere dal calore (ad esempio ventole o prese d'aria piene di polvere), da limiti di consumo per risparmiare energia o da altri programmi partiti durante il test.{no_temp_note}"),
                format!("In the last long test, the processor ran slower toward the end (about {drop_plain} less than at the start). This could be due to heat (for example, fans or air vents clogged with dust), power limits to save energy, or other programs that started during the test.{no_temp_note}"),
            ),
            lang.pick(
                "Ripeti il test con il PC collegato alla corrente e con pochi programmi aperti. Se il calo si ripete, controlla temperature e ventole con il programma del produttore del PC.",
                "Run the test again with the PC plugged in and only a few programs open. If the drop happens again, check temperatures and fans with the PC manufacturer's app.",
            ),
        ),
    );
    f.evidence = evidence;
    f.possible_causes = strings(lang.pick(
        &[
            "Limitazione termica (dissipazione insufficiente, polvere, ventole)",
            "Limiti di potenza della modalità di alimentazione o del produttore",
            "Attività in background comparsa durante il test",
        ],
        &[
            "Thermal throttling (insufficient cooling, dust, fans)",
            "Power limits from the power mode or the manufacturer",
            "Background activity that started during the test",
        ],
    ));
    f.recommendation = lang.pick(
        "Ripetere il benchmark collegati alla rete elettrica e con poche applicazioni aperte; se il calo si ripete, controllare temperature e ventole con lo strumento del produttore.",
        "Repeat the benchmark while plugged in and with few apps open; if the drop happens again, check temperatures and fans with the manufacturer's tool.",
    ).into();
    f.limitations = strings(&[lang.pick(
        "Il calo è misurato su una sola esecuzione del test.",
        "The drop is measured on a single test run.",
    )]);
    if !temp_available {
        f.limitations.push(
            lang.pick(
                "Senza la temperatura CPU non è possibile distinguere una limitazione termica da un limite di potenza.",
                "Without the CPU temperature, thermal throttling cannot be told apart from a power limit.",
            )
            .into(),
        );
    }
    out.findings.push(f);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evaluate;
    use crate::testdata::*;
    use models::Unit;
    use models::benchmark_run::{BenchmarkSeries, SeriesPoint};
    use models::diagnostics::{CoverageStatus, DiagnosticReport};

    fn ids(r: &DiagnosticReport) -> Vec<&str> {
        r.findings.iter().map(|f| f.rule_id.as_str()).collect()
    }

    fn cpu_cov(r: &DiagnosticReport) -> CoverageStatus {
        r.coverage
            .iter()
            .find(|c| c.component == ComponentKind::Cpu)
            .map(|c| c.status)
            .expect("cpu coverage")
    }

    fn with_usage(n: usize, high: usize, value: f64) -> Vec<models::monitor::LiveSample> {
        live(n, |i, s| {
            if i < high {
                s.cpu.usage_percent = avail(value, Unit::Percent, Source::Pdh);
            }
        })
    }

    #[test]
    fn sustained_utilization_fires_at_boundary() {
        let samples = with_usage(60, 36, 85.0);
        let r = evaluate(&input(&samples));
        assert!(ids(&r).contains(&"cpu.utilization.sustained"));
        let f = r
            .findings
            .iter()
            .find(|f| f.rule_id == "cpu.utilization.sustained")
            .expect("finding");
        assert_eq!(f.kind, FindingKind::Observation);
        assert!(!f.evidence.is_empty());
    }

    #[test]
    fn sustained_utilization_does_not_fire_below_boundaries() {
        // 35/60 samples high.
        let r = evaluate(&input(&with_usage(60, 35, 95.0)));
        assert!(!ids(&r).contains(&"cpu.utilization.sustained"));
        // Just under the utilization threshold.
        let r = evaluate(&input(&with_usage(60, 60, 84.9)));
        assert!(!ids(&r).contains(&"cpu.utilization.sustained"));
        // Window too short (59 s).
        let r = evaluate(&input(&with_usage(59, 59, 99.0)));
        assert!(!ids(&r).contains(&"cpu.utilization.sustained"));
        assert_eq!(cpu_cov(&r), CoverageStatus::Partial);
    }

    #[test]
    fn no_live_samples_means_cpu_not_evaluated() {
        let r = evaluate(&input(&[]));
        assert_eq!(cpu_cov(&r), CoverageStatus::NotEvaluated);
        assert!(!ids(&r).iter().any(|i| i.starts_with("cpu.")));
    }

    #[test]
    fn missing_temperature_makes_cpu_partial_with_note() {
        let samples = live(120, |_, _| {});
        let r = evaluate(&input(&samples));
        let c = r
            .coverage
            .iter()
            .find(|c| c.component == ComponentKind::Cpu)
            .expect("cov");
        assert_eq!(c.status, CoverageStatus::Partial);
        assert!(c.note.contains("temperatura CPU non disponibile"));
        assert!(r.limitations.iter().any(|l| l.contains("Temperatura CPU")));
    }

    #[test]
    fn with_temperature_and_long_window_cpu_is_evaluated() {
        let samples = live(120, |_, s| {
            s.cpu.temperature_c = avail(60.0, Unit::Celsius, Source::Wmi);
        });
        let b = [bench("s", BenchmarkKind::CpuSustained, T0)];
        let mut inp = input(&samples);
        inp.benchmarks = &b;
        let r = evaluate(&inp);
        assert_eq!(cpu_cov(&r), CoverageStatus::Evaluated);
    }

    #[test]
    fn dominant_process_fires_and_names_process_without_path() {
        let samples = with_usage(60, 60, 50.0);
        let snap = snapshot(vec![
            process("render.exe", 30.0, 1.0, 40.0),
            process("other.exe", 5.0, 1.0, 10.0),
        ]);
        let mut inp = input(&samples);
        inp.processes = Some(&snap);
        let r = evaluate(&inp);
        let f = r
            .findings
            .iter()
            .find(|f| f.rule_id == "cpu.process.dominant")
            .expect("dominant");
        assert!(f.title.contains("render.exe"));
        let text = serde_json::to_string(f).expect("json");
        assert!(!text.contains("AppData"));
    }

    #[test]
    fn dominant_process_boundaries() {
        let samples = with_usage(60, 60, 50.0);
        // Exactly 25 % of the machine and exactly half of the mean → fires.
        let snap = snapshot(vec![process("a.exe", 25.0, 1.0, 10.0)]);
        let mut inp = input(&samples);
        inp.processes = Some(&snap);
        assert!(ids(&evaluate(&inp)).contains(&"cpu.process.dominant"));
        // Below 25 %.
        let snap = snapshot(vec![process("a.exe", 24.9, 1.0, 10.0)]);
        inp.processes = Some(&snap);
        assert!(!ids(&evaluate(&inp)).contains(&"cpu.process.dominant"));
        // 30 % but total mean 70 % → share 0.43 < 0.5.
        let samples = with_usage(60, 60, 70.0);
        let snap = snapshot(vec![process("a.exe", 30.0, 1.0, 10.0)]);
        let mut inp = input(&samples);
        inp.processes = Some(&snap);
        assert!(!ids(&evaluate(&inp)).contains(&"cpu.process.dominant"));
    }

    #[test]
    fn frequency_limit_reports_observation_and_low_confidence_hypothesis() {
        let samples = live(20, |i, s| {
            if i < 10 {
                s.cpu.limit_mhz = avail(2000.0, Unit::Megahertz, Source::PowerApi);
            }
        });
        let r = evaluate(&input(&samples));
        let obs = r
            .findings
            .iter()
            .find(|f| f.rule_id == "cpu.frequency.limit_reported")
            .expect("obs");
        assert_eq!(obs.kind, FindingKind::Observation);
        let hyp = r
            .findings
            .iter()
            .find(|f| f.rule_id == "cpu.frequency.limit_cause")
            .expect("hyp");
        assert_eq!(hyp.kind, FindingKind::Hypothesis);
        assert_eq!(hyp.confidence, Confidence::Low);
    }

    #[test]
    fn frequency_limit_does_not_fire_below_boundaries() {
        // 9 of 20 samples limited (< 50 %).
        let samples = live(20, |i, s| {
            if i < 9 {
                s.cpu.limit_mhz = avail(2000.0, Unit::Megahertz, Source::PowerApi);
            }
        });
        assert!(!ids(&evaluate(&input(&samples))).contains(&"cpu.frequency.limit_reported"));
        // Limit at exactly 95 % of max is not "below".
        let samples = live(20, |_, s| {
            s.cpu.limit_mhz = avail(4700.0 * 0.95, Unit::Megahertz, Source::PowerApi);
        });
        assert!(!ids(&evaluate(&input(&samples))).contains(&"cpu.frequency.limit_reported"));
        // Too few samples.
        let samples = live(9, |_, s| {
            s.cpu.limit_mhz = avail(2000.0, Unit::Megahertz, Source::PowerApi);
        });
        assert!(!ids(&evaluate(&input(&samples))).contains(&"cpu.frequency.limit_reported"));
    }

    fn sustained_run(early: f64, late: f64) -> BenchmarkResult {
        let mut b = bench("sus", BenchmarkKind::CpuSustained, T0);
        let points = (0..20)
            .map(|i| SeriesPoint {
                elapsed_ms: i * 1000,
                value: if i < 10 { early } else { late },
            })
            .collect();
        b.series.push(BenchmarkSeries {
            id: THROUGHPUT_SERIES_ID.into(),
            label: "Throughput".into(),
            unit: Unit::OperationsPerSecond,
            points,
        });
        b
    }

    #[test]
    fn sustained_benchmark_drop_from_series() {
        let b = [sustained_run(1000.0, 850.0)]; // exactly 15 %
        let mut inp = input(&[]);
        inp.benchmarks = &b;
        let r = evaluate(&inp);
        let f = r
            .findings
            .iter()
            .find(|f| f.rule_id == "cpu.benchmark.sustained_drop")
            .expect("drop");
        assert_eq!(f.kind, FindingKind::Hypothesis);

        let b = [sustained_run(1000.0, 851.0)];
        inp.benchmarks = &b;
        assert!(!ids(&evaluate(&inp)).contains(&"cpu.benchmark.sustained_drop"));
    }

    #[test]
    fn sustained_benchmark_drop_from_metric_and_ignores_failed_runs() {
        let mut ok = bench("m", BenchmarkKind::CpuSustained, T0);
        ok.metrics
            .push(metric(SUSTAINED_DROP_METRIC_ID, 20.0, Unit::Percent, false));
        let mut failed = ok.clone();
        failed.run_id = "f".into();
        failed.state = BenchmarkState::Failed;
        failed.started_at = T0 + 10;
        failed.metrics[1].value = 0.0;
        let b = [failed, ok];
        let mut inp = input(&[]);
        inp.benchmarks = &b;
        assert!(ids(&evaluate(&inp)).contains(&"cpu.benchmark.sustained_drop"));
    }

    #[test]
    fn series_drop_requires_points() {
        assert!(series_drop(&[1.0; 9]).is_none());
        let (_, _, d) = series_drop(&[100.0; 10]).expect("drop");
        assert_eq!(d, 0.0);
    }

    #[test]
    fn english_texts_and_number_formatting() {
        let s = with_usage(60, 60, 99.0);
        let mut inp = input(&s);
        inp.language = models::settings::Language::English;
        let r = evaluate(&inp);
        let f = r
            .findings
            .iter()
            .find(|f| f.rule_id == "cpu.utilization.sustained")
            .expect("finding");
        assert_eq!(f.title, "Sustained high CPU usage");
        assert_eq!(
            f.description,
            "Overall CPU usage was at or above 85.0% in 100.0% of the samples in the observed window."
        );
        assert_eq!(
            f.simple.headline,
            "The processor was very busy most of the time"
        );
        assert!(
            f.evidence
                .iter()
                .any(|e| e.label == "Samples ≥ 85.0%" && e.value == "60 of 60")
        );
        let cov = r
            .coverage
            .iter()
            .find(|c| c.component == ComponentKind::Cpu)
            .expect("cov");
        assert!(
            cov.note
                .starts_with("usage and frequency limit evaluated over 60 s"),
            "{}",
            cov.note
        );
    }
}
