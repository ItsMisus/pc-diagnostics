# Process analysis and the Impact Score

Phase 3 backend: `crates/processes` (sampler, scoring, rankings) and
`crates/platform/src/process.rs` + `windows/process.rs` (OS access). Every value is
measured. No admin rights are needed. The analyser is **read-only**: it never terminates,
suspends, re-prioritizes, injects into or otherwise modifies a process.

The Impact Score estimates **pressure on resources**. It is not evidence that a process
is slowing the computer down, and a high score is never a recommendation to stop a
process or disable a Windows component.

Methodology version: `IMPACT_METHODOLOGY_VERSION = 1`. Scores are comparable only within
the same version; any change to a formula or constant below bumps it.

## API

```rust
pub struct ProcessMonitorConfig { pub interval_ms: u32, pub top_n: u32, pub window_s: u32, pub history_points: usize }
impl Default for ProcessMonitorConfig { /* 2000 ms, 10, 60 s, 150 */ }
impl ProcessMonitor {
    pub fn start<F>(config: ProcessMonitorConfig, on_snapshot: F) -> Result<ProcessMonitor, AppError>
        where F: Fn(&ProcessSnapshot) + Send + 'static;
    pub fn snapshot(&self) -> Result<ProcessSnapshot, AppError>;
    pub fn history(&self, key: &str) -> Result<ProcessHistory, AppError>;
    pub fn set_top_n(&self, top_n: u32) -> Result<(), AppError>;
    pub fn shutdown(self);   // Drop also stops and joins
}
```

- Validation (`InvalidInput`): `interval_ms` 500–10 000 (settings bounds), `top_n`
  `PROCESS_TOP_N_MIN..=PROCESS_TOP_N_MAX` (1–50), `window_s` 1–300,
  `history_points` 1–600.
- `snapshot()` returns `NotFound` (retriable) until two scans have completed. The
  baseline scan runs at start; the first snapshot follows `min(interval, 1 s)` later.
- `history(key)`: a malformed key is `InvalidInput`; an unknown key (never seen, or
  exited more than one window ago) is `NotFound`. Points are oldest first, at most
  `history_points`; `running` is false once the process has exited.
- `set_top_n` takes effect immediately: the stored snapshot is rebuilt from the current
  state (no new scan, no event), and every later event uses the new length.
- `ProcessMonitor` is `Send + Sync`. The sampler thread blocks in `recv_timeout` on a
  control channel (no busy-wait), uses absolute deadlines (a missed tick is skipped, not
  bursted), catches panics in a tick and in `on_snapshot`, and stops promptly.

### Process key

`key = "<pid>:<creation time as Unix ms>"`, e.g. `"1234:1762000000123"`. Windows reuses
PIDs; the creation time makes the key unique per process instance. Parsing is strict
(two runs of ASCII digits separated by one `:`). Boot-time pseudo-processes with no
creation time get `…:0`.

## Data collection

| Value | Source |
|---|---|
| CPU time (kernel + user), working set, private bytes, I/O transfer counts, creation time, parent pid, session id, image name | `NtQuerySystemInformation(SystemProcessInformation)`, one call per tick |
| Executable path | `OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION)` + `QueryFullProcessImageNameW`, once per process instance (cached by key) |
| Window ownership | `EnumWindows` + `IsWindowVisible` + `DwmGetWindowAttribute(DWMWA_CLOAKED)` + `GetWindowThreadProcessId`, once per tick |
| Total / available physical memory | `GlobalMemoryStatusEx`, once per tick |
| Logical processors | `GetActiveProcessorCount(ALL_PROCESSOR_GROUPS)` |

**Why `NtQuerySystemInformation`.** One call returns the counters of *every* process
without opening a handle to any of them, so coverage does not depend on privileges, and
it costs a few milliseconds for ~400 processes. It is what Task Manager and the PDH
`Process` object (perfproc) use. The alternative (Toolhelp + `OpenProcess` +
`GetProcessTimes` / `GetProcessIoCounters` / `GetProcessMemoryInfo` per process) needs
three or four calls and a handle per process per tick, and loses all counters for
processes the user cannot open (about half of them on the development machine).
Several fields used here are `Reserved*` in the SDK's struct; their layout is the
long-standing one documented by phnt and used by every process tool since Vista, and
the struct size and offsets are checked at compile time. The query buffer is reused
across ticks and grows on `STATUS_INFO_LENGTH_MISMATCH` (capped at 64 MiB).

The executable path is opened with `PROCESS_QUERY_LIMITED_INFORMATION` only (a
read-only right). The opened process's creation time is compared with the scan's, so a
recycled PID never gets another process's path.

**Accessibility.** `accessible = false` means Windows denied
`PROCESS_QUERY_LIMITED_INFORMATION` (typically services running as `SYSTEM` or another
account, protected processes, `Memory Compression`); `exe_path` is then `None`. Their
CPU, memory and I/O counters come from the kernel snapshot, which is not
access-checked, so they are still real measurements, not estimates. Any metric that
cannot be computed is `Metric::Unavailable` with a reason, never zero-filled.

**Metric `source`.** The `Source` enum has no variant for `NtQuerySystemInformation`;
process metrics report `Source::Pdh`, because the PDH `Process` object is served from
the same kernel snapshot and reports the same values (see "Contract gaps").

### Rates

- `cpu_percent = Δ(kernel + user time) / Δwall / logical processors × 100`, a share of
  the whole machine, clamped to 0–100. Δwall comes from the monotonic clock between
  scans.
- `io_read_bytes_per_sec`, `io_write_bytes_per_sec` = Δ`ReadTransferCount` /
  Δ`WriteTransferCount` per second. This is **logical I/O**: everything the process asks
  the I/O manager to transfer, including cached file reads, network sockets, pipes and
  devices. It is **not** physical disk bytes; a process reading a file that is already
  in the cache shows I/O while the disk is idle.
- A process first seen after the start of analysis is given a zero baseline (its
  counters started inside the interval) if it was created at most 1 s before the
  previous scan; otherwise its first rates are `Unavailable(Failed(NotFound,
  retriable))` ("no earlier reading").
- A cumulative counter that goes backwards yields `Unavailable(Failed(Platform))` for
  that tick, and the process is re-baselined.
- The **System Idle Process** (pid 0) is idle time, not load: it is never tracked,
  ranked or counted in `total_processes`.

### Categories

Categories describe what kind of process it is. They never imply that it can or should
be stopped.

1. **System**: pid 4 (`System`); the kernel pseudo-processes `Registry`,
   `Memory Compression`, `Secure System` (only when their parent is pid 4 and they have
   no Win32 image path); and the core OS images `smss.exe`, `csrss.exe`, `wininit.exe`,
   `winlogon.exe`, `services.exe`, `lsass.exe`, `LsaIso.exe`, `fontdrvhost.exe`,
   `dwm.exe`, only when their path is directly in `%SystemRoot%\System32` (or the path is
   unreadable, as for protected processes). A look-alike name elsewhere is not System.
2. **Service**: session 0 and not System (services and their hosts).
3. **Application**: interactive session (≠ 0) and owns a visible, non-cloaked top-level
   window.
4. **Background**: interactive session, no such window.
5. **Unknown**: no image name, or window ownership could not be determined this tick.

Limitations: multi-process applications (browsers, Electron apps) have one window-owning
process; their helpers are Background. UWP apps' frames belong to
`ApplicationFrameHost.exe`. An app minimized to the tray has no visible window.

`is_self` marks this process and all its descendants (WebView2), following parent links
from the same scan; a link is accepted only if the child is not older than the parent
(rejects stale links to a recycled parent PID).

## Impact Score

```
impact = 0.45 · CPU + 0.25 · RAM + 0.30 · IO          (ImpactWeights::DEFAULT)
```

Each term is 0–100, so the score is 0–100. Weights sum to 1.

### CPU pressure

`CPU = cpu_percent` (machine share, unscaled).

It is already normalized by the logical processor count, linear in the work the process
does, monotone and bounded. 100 means the process alone keeps every logical processor
busy. No extra scaling is applied: a curve would make scores harder to explain and would
not be comparable across machines with different core counts. Consequence: a process
saturating one thread on a 24-thread CPU scores CPU ≈ 4.2. That is its real share of
the machine's capacity; single-thread saturation is a different question (future work:
a per-thread term).

### RAM pressure

```
RAM = 100 · min(1, working_set / max(available, 2 % · total))
```

The working set as a percentage of the memory that is currently available, capped at
100. Equivalently, `share_of_RAM × 1 / available_fraction`: the share of physical memory
the process holds, amplified as the system runs out of memory. So a process is not
flagged just for being large on a machine with plenty of free memory:

| Case | RAM |
|---|---|
| 4 GiB process, 64 GiB machine, 58 GiB available | ≈ 6.9 (modest) |
| same process, 5 % (3.2 GiB) available | 100 (high) |
| 100 MiB process, 5 % available | ≈ 3 |
| 1 GiB process, 16 GiB machine, 8 GiB available | 12.5 |

Non-decreasing in the working set, non-increasing in available memory, always 0–100. The
2 % floor keeps it finite when almost nothing is available. Working set (resident
physical memory) is used because it is what occupies RAM; shared pages are counted in
every process that maps them, so DLL-heavy processes are slightly overstated.

### I/O pressure

```
IO = 100 · min(1, (read_bytes_per_sec + write_bytes_per_sec) / io_reference)
io_reference = 100 MiB/s (fixed)
```

Linear up to the reference, then saturated (the contract defines the reference as "the
rate that maps to 100"). 100 MiB/s of sustained logical I/O by one process is heavy on
any machine: it is most of a hard disk's sequential throughput, and well above what an
interactive workload issues. A fixed reference was chosen over one derived from
observed disk throughput because logical I/O includes network and cached I/O, which a
disk measurement does not bound. A fixed reference also keeps scores comparable across
machines and over time. On a fast NVMe machine I/O pressure is therefore conservative
(it measures load, not saturation of that particular drive). The reference is reported
in `ProcessContext.io_reference_bytes_per_sec`.

### Unavailable terms

An unavailable rate (no baseline, counter reset) counts as **0** in the score, and the
row still shows the metric as `Unavailable` with its reason, so the score of such a row
is a lower bound. With the kernel snapshot this happens only on a process's first tick
in rare races. Inaccessible processes are listed with `accessible = false`, counted in
`inaccessible_processes`, and scored from their kernel counters like every other
process.

## Rankings

- `ranking_instant`: top `top_n` by the latest sample's impact.
- `ranking_window`: top `top_n` by the **mean** impact over the last `window_s` seconds,
  using only the samples where the process existed. The row's `pressure` holds the mean
  of each term (the impact mean equals the weighted sum of the term means), and
  `impact_peak` is the highest impact in the window. Metric fields (`cpu_percent`, …)
  are the latest values. `window_s` in the snapshot is the span actually covered, shorter
  right after start.
- Both are sorted by impact descending, with ties broken by key (pid, then creation
  time, numerically), so the order is deterministic.
- Only running processes are ranked. A process that exited inside the window is not
  ranked (`ProcessRow` has no "running" flag), but its history stays available.

## History and memory bounds

- Each process instance has a ring of `max(history_points, window samples)` points
  (`window samples = ⌈window_s / interval⌉ + 1`), stored as `f32` (56 bytes per point).
- A process that exited is kept for one `window_s` (so `history()` reports
  `running = false`), then pruned.
- At most `MAX_TRACKED_PROCESSES = 2048` instances are tracked. When full, the
  longest-exited instance is evicted; if every tracked instance is running, new ones are
  not analysed (still counted in `total_processes`). Worst case is 2048 × 600 × 56 B ≈
  69 MiB; typical (≈400 processes × 150 points) is ≈ 3.4 MiB.

## Measured cost

Development machine (24 logical processors, 31 GiB RAM, ~385 processes, of which 182 are
inaccessible without admin rights), `cargo run -p processes --example process_probe`, 2 s
interval:

| Part | Debug median | Release median |
|---|---|---|
| `NtQuerySystemInformation` + parse | 5.9 ms | 6.0 ms |
| `EnumWindows` (+ cloak check) | 0.34 ms | 0.37 ms |
| `GlobalMemoryStatusEx` | 6 µs | 6 µs |
| Ingest (rates, scores, categories) | 0.75 ms | 0.13 ms |
| Snapshot (rankings, top 10) | 0.48 ms | 0.12 ms |
| **Tick total** | **7.4 ms** | **6.7 ms** (max 8.8 ms) |

The first (baseline) tick also resolves every path once: about 19 ms in total. The
kernel call dominates and is the same in debug and release.

## Contract gaps (models)

- `Source` has no variant for `NtQuerySystemInformation` / Win32; `Source::Pdh` is used
  (`processes::PROCESS_METRIC_SOURCE`, a one-line change).
- `ProcessRow.accessible` is documented as "OS denied access to this process's
  counters". With the kernel snapshot the counters are never denied; `accessible` means
  the process could not be opened, so its path is missing.
- `ProcessRow` has no `running` flag, so exited processes are not ranked in the window
  ranking even if they were heavy inside the window.
