# Diagnostics rules, comparison and reports (phase 6)

Code: `crates/diagnostics` (pure: no I/O, no clock, no network, no randomness) and
`crates/persistence` (history storage). Rule set version: `RULES_VERSION = 1`. Bump it
whenever a rule, threshold or the meaning of a text changes.

## Principles

- Every finding has an epistemic status:
  - **Observation**: a measured fact, with no interpretation.
  - **Hypothesis**: an explanation that fits the evidence but is not proven. Its confidence
    is never `high`.
  - **Confirmed**: backed by an authoritative signal, such as a Windows device problem code,
    the Windows disk health status, or measured free space.
- There is no global health score. Each component gets its own findings plus one
  `CoverageItem` (`evaluated` / `partial` / `notEvaluated`) with a note. Missing data gives
  partial coverage. It never reads as "all good".
- No rule suggests buying or replacing hardware, disabling Windows components, or ending
  system processes. A test checks the report text for such wording.
- Every user-facing text exists in Italian and English (see "Languages" below). Process
  evidence names the process only, never its path.

## API

```rust
pub const RULES_VERSION: u32 = 1;
pub struct DiagnosticInput<'a> { app_version, now_ms, system, inventory, live, processes, drivers, benchmarks, language }
pub fn evaluate(input: &DiagnosticInput) -> DiagnosticReport;      // id = "diag-<now_ms>"
pub fn compare_in(lang: Language, current: &BenchmarkResult, previous: &BenchmarkResult) -> BenchmarkComparison;
pub fn compare(current: &BenchmarkResult, previous: &BenchmarkResult) -> BenchmarkComparison; // = compare_in(Italian, …)
pub fn find_previous_comparable<'a>(current: &BenchmarkResult, history: &'a [BenchmarkResult]) -> Option<&'a BenchmarkResult>;
pub struct ReportBundle<'a> { report, system, inventory, benchmarks, drivers, processes, utc_offset_minutes, language }
pub fn render_json(bundle: &ReportBundle, redact: bool) -> Result<String, AppError>;
pub fn render_html(bundle: &ReportBundle, redact: bool) -> Result<String, AppError>;
pub fn report_file_name(report: &DiagnosticReport, format: ReportFormat, utc_offset_minutes: Option<i32>) -> String;
```

## Languages

The command passes the effective UI language (`AppState::language()`, a
`models::settings::Language`: `Italian` or `English`) in `DiagnosticInput::language` and
`ReportBundle::language`. Every text the crate produces follows it: finding titles,
descriptions, evidence labels and values, possible causes, recommendations, limitations,
the plain-language text, coverage notes, report-level limitations, comparison caveats,
the HTML labels, `<html lang>`, dates and numbers, and the redaction placeholders.

- **Stored data keeps its language.** A `DiagnosticReport` is saved with the texts it was
  generated in and is never re-translated. Exporting it later in the other language
  translates only what the export adds (headings, labels, dates); run the diagnosis again
  to get the findings in the new language. Texts that come from other crates (benchmark
  metric labels, notes and warnings, device problem descriptions) are shown as stored.
- **Same facts in both languages.** The language changes wording and number formatting,
  never a rule, threshold, severity, confidence, evidence count or source. A test runs every
  scenario in both languages and checks exactly that.
- **Formatting** (`fmt`, every function takes the language): Italian `85,0 %`,
  `1,5 GiB`, `2026-11-04 12:21:05 (UTC+01:00)`, lists "a, b e c"; English `85.0%`,
  `1.5 GiB`, `Nov 4, 2026, 12:21:05 PM (UTC+01:00)`, lists "a, b, and c". No thousands
  separator in either.
- **Technical errors** (`AppError` message and detail) are English only; the UI translates
  the title per `ErrorCode`.
- **File names** contain no words (`pc-diagnostics-YYYY-MM-DD-HHMM.html`), so they are the
  same in both languages.

### Adding or changing a text

Write both languages next to each other with `lang.pick(italian, english)`; the language
is `ctx.lang` in a rule and a `lang: Language` argument in helpers:

```rust
f.recommendation = lang.pick(
    "Riavviare il PC e ripetere la misurazione.",
    "Restart the PC and measure again.",
).into();
cov.partial(lang.pick(
    format!("utilizzo non disponibile per {name}"),
    format!("usage not available for {name}"),
));
f.possible_causes = strings(lang.pick(&["Causa uno", "Causa due"], &["Cause one", "Cause two"]));
```

- Compute numbers once with the `fmt` helpers (`fmt::pct(lang, v)`, `fmt::bytes_plain(lang, b)`,
  `fmt::il_pct(lang, v)`, …) and interpolate the same variables into both texts.
- Shared phrases are small functions next to their use (`rules::no_action(lang)`,
  `rules::no_samples(lang)`, `rules::count_of(lang, hits, total)`, `processes::impact_disclaimer(lang)`),
  not a global table.
- English is plain US English: short sentences and everyday words in the plain-language
  text (same style rules as below), technical terms only in the technical fields. Handle
  singular and plural where a count can be 1.
- Do not put an ASCII double quote inside a text (use “…” or «…»): the rule-id scan of the
  plain-language test splits the sources on `"`.
- Run `cargo test -p diagnostics`: the English checks below fail on untranslated text,
  Italian words in an English report, jargon or unhedged hypotheses.

### Ordering

Findings are sorted by:

1. Severity, descending (critical, then warning, then info).
2. Kind: confirmed, then hypothesis, then observation.
3. `rule_id`, ascending.

Ties keep evaluation order (stable sort), which is deterministic. Coverage is listed in
this order: CPU, GPU, Memory, Storage, Drivers, Processes, System.

## Live window

- **Observed window**: the span from the first to the last sample, plus one median sample
  interval, rounded to whole seconds. Each sample measures the interval before it. With 0
  or 1 samples the window is 0.
- **Sustained**: the window is at least `SUSTAINED_MIN_WINDOW_S = 60` s, and the condition
  holds in at least `SUSTAINED_SHARE_PERCENT = 60` % of the samples where the metric is
  available. The check uses integer arithmetic, so exactly 60 % counts.
- **Usable metric**: available in at least 50 % of the samples. If it is not, the rule is
  skipped and coverage notes the gap.
- **Short window**: a window under `SUSTAINED_MIN_WINDOW_S` makes the CPU, GPU and memory
  coverage `partial`, without a per-component note: the window and the 60 s minimum are
  stated once (the app shows one warning; the HTML export adds it to the observed window).

Rationale: 60 s is long enough to rule out short spikes, such as an app launch, and the
monitor keeps 5 minutes of history. The 60 % share tolerates gaps and brief dips without
reacting to one burst.

## Rules

| rule_id | Condition | Kind | Severity | Confidence |
|---|---|---|---|---|
| `cpu.utilization.sustained` | CPU usage ≥ 85 % (sustained) | Observation | Warning | High |
| `cpu.process.dominant` | Top `ranking_window` process by CPU ≥ 25 % of the machine **and** ≥ 50 % of the mean total CPU (share check skipped and noted if live CPU is unavailable) | Observation | Info | Medium |
| `cpu.frequency.limit_reported` | ≥ 10 samples with limit and max available, and limit < 95 % of max in ≥ 50 % of them | Observation | Info | High |
| `cpu.frequency.limit_cause` | Same condition: possible causes are power plan, battery, thermal and firmware | Hypothesis | Info | Low |
| `cpu.benchmark.sustained_drop` | Latest completed `CpuSustained` run: drop ≥ 15 % (see below) | Hypothesis | Warning | Medium |
| `memory.available.low` | Available < 10 % of total (sustained); Critical if < 5 % is also sustained | Observation | Warning / Critical | High |
| `memory.commit.high` | Commit used ≥ 90 % of the commit limit (sustained) | Observation | Warning | High |
| `memory.pressure.high` | macOS kernel pressure level (`kern.memorystatus_vm_pressure_level`) Warning or Critical (sustained); Critical if the Critical level is also sustained. Swap in use is listed as evidence. Not reported on Windows | Observation | Warning / Critical | High |
| `memory.paging.high` | `Pages/sec` ≥ 1000 on Windows, pageins/s ≥ 250 on macOS (same ≈ 4 MiB/s; sustained) | Hypothesis | Warning + Medium if a memory rule above also fired; otherwise Info + Low | — |
| `gpu.utilization.sustained` | Per GPU: busiest engine ≥ 95 % (sustained) | Observation | Info | High |
| `gpu.vram.high` | Per GPU: dedicated used ≥ 90 % of inventory `dedicated_vram_bytes` (sustained). Not evaluated (coverage partial) when the inventory size is unavailable: "unified memory" for Apple GPUs that report it as not supported, "total unknown" otherwise | Observation | Warning | High |
| `gpu.driver.problem` | Driver inventory device, category `Gpu`, health `ProblemDetected` or `DriverMissing` | Confirmed | Critical | High (Medium if no problem code) |
| `storage.volume.low_space` | Fixed volume: free < 5 % gives Critical. Free < 10 %, or free < 15 GiB on a volume ≥ 30 GiB, gives Warning | Confirmed | Warning / Critical | High |
| `storage.disk.health` | `MSFT_PhysicalDisk.HealthStatus` Warning (gives Warning) or Unhealthy (gives Critical) | Confirmed | Warning / Critical | High |
| `storage.benchmark.result` | Latest completed run per storage kind: values only | Observation | Info | High |
| `storage.benchmark.drop` | That run versus `find_previous_comparable`: any metric worse by ≥ 25 % (lower for higher-is-better, higher otherwise) | Hypothesis | Warning | Medium (Low if the comparison has caveats, which are appended as limitations) |
| `drivers.device.problem` | Non-GPU device with `ProblemDetected` | Confirmed | Critical for StorageController, Disk, Ethernet, Wifi; Warning otherwise | High (Medium if no problem code) |
| `drivers.device.driver_missing` | Non-GPU device with `DriverMissing` | Confirmed | Same as above | Same as above |
| `drivers.updates.available` | ≥ 1 device with `UpdateAvailable`. Lists up to 10 devices, then "e altri N" | Observation | Info | High |
| `processes.impact.high` | `ranking_window` rows with mean `pressure.impact` ≥ 50. One finding lists up to 5 of them | Observation | Info | Medium |

Notes and rationale:

- **CPU 85 %**: high enough that the CPU is clearly the busy resource, low enough to catch
  a machine that stays near saturation. It is an observation, not a slowdown verdict, and
  the limitation says so.
- **Dominant process**: 25 % of the whole machine is a large share on a multi-core system.
  The requirement of half of the total keeps a process from being called dominant when
  the load is spread across many processes. The value comes from the process window
  ranking, not from the moment of the problem, and the limitation states this. Rows with
  `is_self` get an extra limitation.
- **Frequency limit**: Windows reports the limit (`CallNtPowerInformation`) but not its
  cause. The 95 % ratio absorbs rounding and small policy steps. The cause is only ever a
  low-confidence hypothesis. If the newest benchmark ran on battery, that fact is added
  as evidence.
- **CPU sustained drop**: computed from the metric `throughput_drop_percent` if the run
  publishes it, where a positive value means slower. Otherwise it is computed from the
  series `throughput` (at least 10 points): the mean of the first 20 % of points
  (minimum 2) is compared with the mean of the last 20 %. If the series `cpu_mhz` exists,
  its early → late values are added as evidence. Without CPU temperature the finding says
  that thermal and power limits cannot be told apart.
- **Memory**: below 10 % available, Windows is trimming working sets. At ≥ 90 % commit,
  allocations start to fail when the limit is reached. `Pages/sec` also counts
  memory-mapped file reads, so on its own it stays a low-confidence hypothesis. The
  recommendation for every memory rule is: "Verificare i processi con maggiore consumo e
  ripetere la misurazione durante il problema". The top 3 window-ranking processes by
  working set are added as evidence (name only).
- **GPU**: saturation is Info because a game at 100 % is normal. Virtual and software
  adapters (per the inventory) are ignored. VRAM needs the inventory total. Without it
  the rule is skipped and noted.
- **Storage space**: free space is a measured fact, so it is reported as Confirmed. The
  absolute 15 GiB threshold applies only to volumes of at least 30 GiB. Otherwise a
  small volume would always trigger it. Removable volumes are skipped.
- **Disk health**: the limitation is always "Stato riassuntivo di Windows, non un'analisi
  SMART completa". `Healthy` produces no finding and is never shown as "SMART OK".
  Coverage notes that the status is the Windows summary.
- **Storage benchmarks**: no "slow disk" verdict is given without a comparable reference.
  The 25 % threshold is wide because storage results vary between runs.
- **Drivers**: the Configuration Manager problem code is authoritative. `Disabled`,
  `NotVerifiable` and `NotChecked` are not problems. An update offer comes with the
  limitation that it does not mean the current driver is faulty.
- **Impact Score**: the description and the limitation both state that the score
  estimates pressure on resources and is not proof of a slowdown (see
  docs/impact-score.md).

## Plain-language text ("Semplice" view)

Every finding carries `simple: SimpleExplanation { headline, meaning, action }`, written for
someone who knows little about computers, in the report language. The UI shows it in the
"Semplice" / "Simple" view; the HTML report shows it as an "In parole semplici" ("In plain
words") block before the technical details. The rule
builder `rules::finding(…, simple)` takes it as a required argument, so a rule cannot be
written without it (a `debug_assert` also rejects empty parts).

Style guide:

- **headline**: one short sentence in everyday words, no final period, naming the device,
  disk, GPU or program when the technical title does ("Un dispositivo non funziona
  correttamente: Scheda audio", "Il PC sta usando quasi tutta la memoria").
- **meaning**: 1–2 sentences on why it matters in practice (slowdowns, freezes, a device
  or connection not working, data at risk). No acronyms or internal names (commit, PDH,
  CM_PROB, TDR, throttling, VRAM, SMART, paging, Impact Score, benchmark, MHz, GiB…). A
  technical word that cannot be avoided is explained in place: "il driver (il programma che
  fa funzionare il dispositivo)", "la memoria (RAM, dove il PC tiene i programmi aperti)".
- **action**: concrete, safe steps addressed to the user ("Riavvia il PC", "Chiudi i
  programmi che non ti servono"), consistent with the technical recommendation. Never:
  buying or replacing components, disabling Windows components, registry edits, BIOS/UEFI
  changes, overclocking/undervolting, ending system processes. The app never installs
  drivers; the text may tell the user to install or update one from the manufacturer's
  site or Windows Update where the technical recommendation does. Nothing to do: "Nessuna
  azione necessaria." / "No action needed." (`rules::no_action(lang)`), optionally with the
  condition under which to act.
- **Same facts, never stronger.** A hypothesis stays hedged ("potrebbe", "probabilmente",
  "forse"); low confidence sounds uncertain; an observation states what was measured and
  never claims a cause. Limitations that change the reading are carried over in plain
  words (a GPU at 100 % during a game is normal; an update offer does not mean the current
  driver is faulty; the Impact Score is not proof of a slowdown; comparisons with caveats
  are "poco affidabili").
- **Numbers only with context**: rounded percentages with the article ("in media era
  occupato il 92 % del totale", `fmt::il_pct`, which elides to "l'8 %"); sizes with the
  labels Esplora file uses ("1,0 GB su 100,0 GB", `fmt::bytes_plain`); drive letters as
  shown in Windows ("D:"). Lists use "a, b e c" (`fmt::join_names`). English: "92%" with
  no article, "1.0 GB of 100.0 GB", "a, b, and c".
- Same dynamic data as the technical text (device, disk and GPU names, process names,
  percentages). Plain texts go through the same redaction scrub as every other string.

Device findings (`drivers.device.*`, `gpu.driver.problem`) pick the text by health and
Windows problem code, then append what may stop working for the device category (Wi-Fi,
audio, screen and games, USB peripherals…):

Headlines below are the Italian ones; each has an English counterpart in `drivers.rs`
(e.g. 43: "Windows stopped a device because it reported errors").

| Case | Headline (prefix before ": <device>") | Action |
|---|---|---|
| `DriverMissing` (codes 1, 18, 28) | Manca il driver di un dispositivo | Install the driver from the manufacturer's site or Windows Update, restart |
| No problem code (status read failed) | Windows non è riuscito a controllare un dispositivo | Restart, then reinstall the driver from the manufacturer's site |
| 14 | Un dispositivo ha bisogno di un riavvio del PC | Restart |
| 43 | Windows ha fermato un dispositivo perché ha dato errori | Restart, then reinstall |
| 45 | Un dispositivo non risulta collegato | Check the connection, restart; ignore if no longer used |
| 47 | Un dispositivo è pronto per essere scollegato | Unplug and replug, or restart |
| 48 | Windows ha bloccato il driver di un dispositivo (known bad driver) | Look for an updated driver on the manufacturer's site |
| 52 | Windows ha bloccato il driver di un dispositivo (signature) | Reinstall the driver from the manufacturer's site |
| 3, 10, 19, 31, 32, 37, 38, 39, 40, 41 | Windows non riesce ad avviare un dispositivo | Restart, then reinstall |
| 44 | Un programma ha spento un dispositivo | Restart, then reinstall |
| any other code | Un dispositivo non funziona correttamente | Restart, then reinstall |

Test (`src/plain_tests.rs`): the rule ids are scanned from the rule sources, and a set of
scenarios must trigger every one of them (a new rule without a scenario fails the test).
For every produced finding it checks: headline, meaning and action are non-empty; the
headline is one sentence; no banned jargon token (`BANNED_JARGON`, case-insensitive);
no banned advice (`BANNED_ADVICE`); hypotheses and low-confidence findings contain a hedge
word (`HEDGES`); observations contain no causal wording (`CAUSAL_WORDS`). Focused tests pin
the exact text of several rules, singular/plural forms, the device texts by problem code,
and the report tests check that the HTML block is escaped, comes before the technical
details, and that a user name in a device name is redacted in the plain text too.
The English run of the same scenarios (`every_rule_has_valid_english_plain_text`) applies
the same checks with English word lists: `BANNED_JARGON` is shared, `BANNED_ADVICE_EN`
(buy, purchase, replace, disable, registry, BIOS…), `HEDGES_EN` (might, may, could,
probably, possibly…), `CAUSAL_WORDS_EN` (because, due to, caused by…).
`english_reports_mirror_italian_ones` checks that both languages produce the same findings
in the same order with the same severity, kind, confidence, component, evidence sources and
counts, that every text and coverage note differs, and that no Italian word appears in an
English report. Each rule module, `compare`, `lib` and the report tests also pin a few
English texts.
`cargo test -p diagnostics dump_plain_texts -- --ignored --nocapture` prints all texts in
both languages for proofreading.

## Coverage

| Component | NotEvaluated | Partial (any of) | Notes always added |
|---|---|---|---|
| Cpu | no samples, or usage unusable | window < 60 s; temperature unavailable in every sample; limit/max unusable | what was evaluated and over what window; no `CpuSustained` run / run without drop data |
| Gpu | no physical GPU, no samples, or no GPU samples | window < 60 s; usage unusable; VRAM total unknown; temperature unavailable (per GPU) | window |
| Memory | no samples, or available memory unusable | window < 60 s; commit unusable; pages/sec unusable | window |
| Storage | no inventory | no fixed volume; physical disks unavailable (with reason); health unknown/unavailable for a disk | the disk status is the Windows summary, not SMART; no storage benchmark |
| Drivers | no driver inventory | update check not run ("verifica aggiornamenti non eseguita" / "update check not run"); update check failed | device counts; disabled devices are not malfunctions |
| Processes | no snapshot | window < 30 s; unexpected Impact methodology version | processes analysed and how many have unreadable paths |
| System | no `SystemInfo` | always Partial when present: no OS-level rules in v1 (Windows Update status and system file integrity are not checked) | OS version |

Report-level limitations:

- The rule set version and the fact that only the available data was used.
- The observed window, or the absence of live history.
- CPU temperature unavailable.
- No benchmark available.
- "L'assenza di risultati … non significa che sia tutto in ordine" / "No findings for a
  component does not mean everything is fine".

## Benchmark comparison

- **Comparability**: decided only by `BenchmarkResult::comparable_with`. When runs are
  not comparable, `comparable = false`, `deltas` is empty, and the caveats name each
  differing field: kind, either run not completed, methodology version, build profile,
  request fields (duration, threads, volume, test file size, write consent), CPU model,
  logical processors, total memory, disk model, GPU name, GPU driver version. The GPU
  adapter id in the request is ignored: it is a LUID, which changes after a reboot.
- **Deltas**: one per metric id present in both runs, in the current run's order.
  `change_percent = (current − previous) / previous × 100`, or `None` if `previous == 0`
  or either value is not finite.
- **Context caveats** (the runs stay comparable):
  - Different power source.
  - Different power plan.
  - Background CPU > 10 % in either run.
  - Frequency limit < 99 % of max in either run.
  - Available memory differs by more than 25 % (measured against the larger value).
  - Warnings present in only one of the two runs (each listed). A warning both runs
    share (e.g. "Temperatura CPU non osservabile") describes the machine, not a
    difference between the runs, so it is not repeated.
- **Language**: `compare_in(lang, …)` writes the caveats in `lang`; `compare` is kept as an
  Italian wrapper for callers that do not pass a language yet.
- **`find_previous_comparable`**: picks the run with the greatest `started_at` that is
  strictly older than `current`, has a different `run_id` and is comparable. It does not
  depend on the history order.

## Reports

- **JSON layout** (`ExportDocument`):
  `{ format: "pc-diagnostics-report", formatVersion: 1, redacted, language: "it"|"en", generatedAtUtc, utcOffsetMinutes, report, system, hardware, benchmarks, drivers: { collectedAt, summary, updateCheck, problemDevices[] }, processes: { at, windowS, methodologyVersion, totalProcesses, top[10] } }`.
  `language` is the language of the labels the export added (older documents without it
  read as `en`).
  - Drivers: summary plus devices with `ProblemDetected` or `DriverMissing` only.
  - Processes: the top 10 of the window ranking, with name, category, impact, peak, CPU %
    and working set. Paths are never included, even without redaction.
- **HTML**:
  - One self-contained file with a CSP meta (`default-src 'none'; style-src 'unsafe-inline'`).
  - No scripts and no external resources.
  - Monochrome CSS with light and dark themes through `prefers-color-scheme`.
  - All text goes through `escape` (`& < > " '`).
  - Sections: header (date, software and rules version, window, redaction state),
    coverage, findings (with kind, severity, confidence, an "In parole semplici" block
    with headline, meaning and "Cosa puoi fare", then the technical details: description,
    evidence, causes, recommendation, limitations; the block is omitted for findings
    saved before the plain text existed), system, hardware, tests run (parameters, duration,
    result, metrics, notes, warnings, safety stop, error), drivers, processes, limitations.
  - `<html lang="it">` or `<html lang="en">`; every heading and label in that language.
  - Dates use local time with the offset printed, e.g. `2026-11-04 12:21:05 (UTC+01:00)`
    (English `Nov 4, 2026, 12:21:05 PM (UTC+01:00)`), so an instant stays unambiguous
    across DST changes; UTC when the offset is unknown.
- **File name**: `pc-diagnostics-YYYY-MM-DD-HHMM.{json|html}`, using the report's local
  time (Windows time zone, DST-aware, `SystemTimeToTzSpecificLocalTime`); UTC if the
  offset cannot be determined. The hostname is never part of it.

### Redaction (`redact = true`)

Structural:

- `system.hostname` is removed.
- Volume labels are removed.
- `instanceId` of problem devices is omitted.
- Process paths are never exported, with or without redaction.

String scrub, applied to every string of the document. Tokens are collected from the
unredacted bundle, including process `exe_path` and all device instance ids. The
placeholders follow the export language (English: `<device id>`, `<user>`, `<host>`):

1. Each device instance id, plus its last segment when that is ≥ 8 characters (often a
   serial number), becomes `<id dispositivo>`. This match is not whole-word.
2. Any `\Users\<name>` or `/Users/<name>` segment (ASCII case-insensitive) becomes
   `\Users\<utente>`.
3. Each user name detected in such paths becomes `<utente>`. Public, Default and All
   Users are excluded. This match is whole-word and ASCII case-insensitive.
4. The hostname becomes `<host>`. This match is whole-word and ASCII case-insensitive.

Model names (CPU, GPU, disk, devices) and mount points are kept.

## Persistence (migration v2)

```sql
benchmark_results(run_id TEXT PK, kind TEXT, state TEXT, methodology_version INTEGER, started_at INTEGER, json TEXT) STRICT
  + INDEX (kind, started_at DESC)
diagnostic_reports(id TEXT PK, generated_at INTEGER, json TEXT) STRICT
```

- `kind` and `state` hold the serde names (for example `cpuSustained`).
- `save_benchmark_result` upserts by `run_id`.
- `list_benchmark_results(kind, limit)` returns newest first (`started_at DESC, run_id DESC`)
  and clamps `limit` to 1..=500.
- `get_benchmark_result` returns one run by id.
- `save_diagnostic_report` upserts and then, in the same transaction, keeps only the
  newest 20 reports.
- `latest_diagnostic_report` returns the newest report that parses.
- Rows with corrupt JSON are skipped with a `tracing::warn`.

## Known limitations

- The bench crate is still empty, so the `CpuSustained` metric and series ids
  (`throughput_drop_percent`, `throughput`, `cpu_mhz`) are this crate's convention.
  The benchmark engine must publish them under these ids.
- Report dates are in UTC. The input has no time zone.
- If a benchmark metric value is NaN, the redacted export fails with `Internal`. JSON
  turns NaN into `null`, which does not deserialize back into `f64`.
- Volume labels are removed structurally only. They are not scrubbed from free text,
  because common labels such as "Windows" would damage the text.
- User names are detected only from `\Users\<name>\` paths that appear in the data.
