# Benchmark methodology (phase 5)

Engine: `crates/bench`. OS facts: `crates/platform/src/bench_env.rs` and
`crates/platform/src/gpu_bench.rs`. All methodology versions below are **1** (GpuCompute
went from 0, not implemented, to 1). A version is bumped whenever a workload, its parameters or a
metric definition changes; results with different versions are never compared
(`BenchmarkResult::comparable_with`).

Priorities: correctness, then system safety, then reliability, then performance. Nothing
starts automatically. One heavy benchmark runs at a time. There are no overclock,
undervolt, BIOS, registry or power-plan changes, and no admin rights are needed.

## API (`bench`)

```rust
pub type LatestSample = Arc<dyn Fn() -> Option<LiveSample> + Send + Sync>; // closure over Monitor::latest
impl BenchController {
    pub fn new(app_version: String, build_profile: String, latest: LatestSample) -> Self;
    pub fn capabilities(&self) -> Vec<BenchmarkCapability>;   // one per BenchmarkKind
    pub fn start<P, F>(&self, request, on_progress: P, on_finished: F) -> Result<String, AppError>;
    pub fn cancel(&self, run_id: &str) -> Result<(), AppError>; // NotFound if not active
    pub fn status(&self) -> Option<BenchmarkProgress>;
    pub fn shutdown(&self);                                     // also run by Drop
}
```

- `start` validates synchronously and returns errors directly: `InvalidInput` for bounds,
  missing consent, an unknown volume, too little free space, a `gpuId` on a non-GPU kind,
  or a `gpuId` that is unknown or names an unavailable GPU; `NotSupported` for
  `GpuCompute` when no suitable GPU was found or the default GPU is unusable (detail = the
  reason); `Busy` while a run is active.
- `capabilities()` probes the GPUs on its first call (it starts the GPU worker once in
  probe mode for all adapters and reads each physical adapter's thermal thresholds,
  ~0.5–1.1 s on the test machine) and caches the result for the controller's lifetime.
- GPU worker entry point (called first thing in `main`):
  `bench::gpu_worker::run_if_requested() -> Option<i32>`; `Some(code)` means the process
  was started as the worker and must exit with `code`. It then returns the run id, which is a
  UUID-shaped string.
- `on_progress` is called about every 250 ms, on the supervisor thread.
- `on_finished` is called **exactly once** per started run, with `Completed`, `Cancelled` or
  `Failed`. `metrics` is empty unless the run completed. `error` is set when the run
  failed. `safety_stop` is set when the controller stopped the run. The slot is released
  *before* `on_finished` runs, so the callback may start the next run.
- `shutdown` cancels the active run and joins every thread. The cancelled run's
  `on_finished` still fires before `shutdown` returns.
- `Monitor::latest()` was added to `crates/monitor`.
- `build_profile == "debug"` adds the warning "Build di debug: risultati non confrontabili
  con build di release".

## Controller

**States.** `Idle → Preparing → Running → Completed`. A cancel goes
`Preparing|Running → Cancelling → Cancelled`. Errors and panics go to `Failed`. Every
transition passes through `BenchmarkState::can_transition_to`. A test records every
applied transition and checks that each one is legal.

**Threads per run:**

- **Supervisor.** Owns the callbacks and collects context. It then polls every 25 ms,
  emits progress every 250 ms and runs the safety checks.
- **Driver** (scoped). Runs the workload inside `catch_unwind`.
- **Workers** (scoped, inside the driver). Because they are scoped, they are always joined,
  including during unwinding. A worker panic ends the run as `Failed`.
- **Context thread** (detached). Only read-only queries run here (WMI disk model), so a
  stalled WMI call can never delay a cancel.

**Cancellation** is a shared flag that workers check at fine granularity:

| Workload | How often the flag is checked |
|---|---|
| CPU | Every work unit (~50 µs) |
| Memory | Every 1 MiB sub-chunk; the pointer chase every 65 536 loads |
| Storage | Every I/O |

Measured on the test machine (release build, multi-core run on all 24 threads), the time
from `cancel()` to `on_finished(Cancelled)` was **2.0 / 2.5 / 6.5 ms** across three
attempts. In the debug test with 2 threads it was 26 ms. The test asserts < 500 ms, and
also asserts that 0 benchmark threads are alive when `on_finished` runs.

**Cleanup:**

- Buffers are `Vec`s, so ordinary drop and unwinding free them.
- The storage file is opened with `FILE_FLAG_DELETE_ON_CLOSE`, and `BenchFile::drop` also
  deletes it explicitly.
- `BenchController::new` sweeps stale files (see Storage).

### Safety thresholds and rationale

| Check | Threshold | Rationale |
|---|---|---|
| Watchdog (all kinds) | CPU: duration + 20 s (covers the 1 s warm-up). Memory: 120 s. Storage: 1200 s | Catches a stuck workload; the bounds are generous for slow HDDs (4 GiB written and read at ~10 MB/s). Effect: `safety_stop`, then `Cancelled` |
| Available memory during memory tests | Stop if available < `max(1 GiB, 5 % of RAM)`. Read directly every 250 ms with `GlobalMemoryStatusEx`, so it works without the live monitor | Leaves the OS and the apps headroom and avoids paging. 1.55 GiB on the 31 GiB machine |
| Memory buffer sizing (before allocating) | Bandwidth: `min(1 GiB, 25 % of available)`, rounded down to 64 MiB, at least 256 MiB. Latency: a fixed 256 MiB that must also fit in 25 % of available. In both cases available − buffer must stay ≥ the reserve, otherwise `Failed(InvalidInput)` with a clear message | 256 MiB is far above any L3 (64 MiB on the 7900X); 25 % keeps the footprint modest |
| Sustained CPU frequency limit | Stop if the OS limit (`MhzLimit`, read directly every 1 s) is < 60 % of `MaxMhz` on 2 consecutive checks. Only applies if the limit was ≥ 60 % at start | A large drop during the run means policy or thermal limiting. The cause is reported as unknown. Two checks avoid single-sample glitches. A limit that was already low at start is a start condition (warning), not a drop |
| CPU temperature | **None.** It is not observable on this machine (no kernel driver), so no threshold is invented. Instead: sustained default 60 s, max 300 s; single/multi default 15 s, max 60 s. Warning: "Temperatura CPU non osservabile: applicati limiti di durata conservativi" | No universal temperature limit exists. Duration caps are the conservative substitute |
| GPU temperature (GpuCompute) | Per selected GPU, from the driver's own thresholds, checked every 1 s (live monitor sample for that GPU id, else a direct sensor read: NVML on NVIDIA, WDDM/D3DKMT otherwise). (1) NVIDIA with NVML thresholds: stop at `min(slowdown − 5 °C, GPU max operating temperature)`; on the test machine (RTX 4070 SUPER: slowdown 96 °C, max operating 90 °C, shutdown 101 °C) the stop point is **90 °C** (its WDDM warning of 84 °C is not used: the NVML rule takes precedence). (2) Otherwise, when the driver reports a WDDM `TemperatureWarning`, stop at that value as is. (3) Otherwise no temperature stop, max duration 30 s instead of 60 s, and the warning "Soglia termica non fornita dal driver: applicati limiti di durata conservativi" (temperature readable, e.g. the AMD iGPU) or "Temperatura GPU non osservabile: applicati limiti di durata conservativi" (not readable). A run with a threshold in use whose temperature becomes unreadable for 3 consecutive checks is stopped | The thresholds are the driver's own values for this exact GPU, so nothing universal is invented. Max operating temperature is where clocks may drop below base; slowdown is where hardware throttling starts; the WDDM warning is the level the driver itself declares as a warning. The shutdown and WDDM `TemperatureMax` values alone are never used as stop points (too close to a hardware trip) |
| GPU watchdog | Hard limit: duration + warm-up (2 s) + 40 s. Stall: no message from the worker for 20 s → kill → `Failed`. Cancel: `cancel` on stdin + stdin closed, 1 s grace, then kill | The worker reports every 250 ms; 40 s covers process start, adapter selection and shader compilation with a wide margin |
| Storage free space | `free ≥ file + clamp(10 % of volume, 2 GiB, 20 GiB)`. Checked at `start` and again before the file is created | Never fill a volume |
| Storage write budget | Sequential: file size. Random: file size + 256 MiB. Every write is charged; going over fails the run | Bounded wear. The bound is published as `max_bytes_written` |

## Context and warnings

`BenchmarkContext` is collected while the run is `Preparing`:

- OS version, CPU brand and logical processors.
- Total RAM (sysinfo).
- Power source (`GetSystemPowerStatus`).
- Active plan name (`PowerGetActiveScheme` + `PowerReadFriendlyName`).
- Background CPU: the mean `usage_percent` of the distinct live samples (by `seq`, at most
  3.5 s old) observed during a 3 s pre-start window. The window is applied only when the
  monitor is running; otherwise the field is `ProviderMissing("live sampler not running")`.
- Available memory: from the latest sample, or else read directly (`Source::Benchmark`).
- CPU limit and max MHz (`CallNtPowerInformation`).
- Storage model: the volume is resolved to a disk with `IOCTL_VOLUME_GET_VOLUME_DISK_EXTENTS`
  (opened with zero access rights). If every extent is on one disk, the model is
  `MSFT_PhysicalDisk.FriendlyName` for that disk. A volume that spans several disks, or a
  failed lookup, gives `None`.

Warnings (in the language effective when the run starts, Italian or English; stored
with the result in that language, like the metric and series labels, phases, notes and
the safety stop):

- Battery power.
- Background CPU > **10 %**.
- Limit < max − 1 MHz at start.
- Debug build.
- CPU temperature not observable (CPU kinds).
- Live monitor not running.
- Kind-specific warnings (below).

## Workloads

### CPU (`CpuSingleCore`, `CpuMultiCore`, `CpuSustained`)

One **work unit** is a fixed, deterministic mix of three kernels:

1. **Integer:** xorshift64*-driven read-modify-write on a 256-entry `u64` table, with
   data-dependent index, rotate, multiply and accumulate, 8192 steps.
2. **Hashing:** FNV-1a 64 over the 2 KiB table, 4 rounds.
3. **Floating point:** Mandelbrot escape iterations on a fixed 32×32 grid, at most 64
   iterations, with a seed-dependent sub-pixel offset.

About 50 µs per unit on a 7900X core.

**Anti-elision and verification.** Seeds and results pass through `black_box`, and results
fold into a per-worker checksum. Each worker first recomputes the reference unit
(`seed 0x5EED000000000001`) and compares it with the value computed at prepare time. It
repeats the check every 1024 units, and a mismatch fails the run. The v1 golden value
`0xFCC69579238CA50E` is pinned by a unit test that passes in both debug and release, so
any change to the workload forces a version bump. A second test asserts that a unit takes
measurable time.

**Measurement.** A 1 s warm-up is excluded, then the timed window runs for `duration_s`.
Throughput = units completed in the window / window length (`OperationsPerSecond`).

| Kind | Threads | Duration (default / min / max) |
|---|---|---|
| Single | 1 | 15 / 5 / 60 s |
| Multi | `threads` (default = logical processors, 1..=logical) | 15 / 5 / 60 s |
| Sustained | Same as Multi | 60 / 30 / 300 s |

**Metrics.** The headline is always first and always has id `throughput`:

| Kind | Metrics |
|---|---|
| Single | `throughput` |
| Multi | `throughput`, `throughput_per_thread` |
| Sustained | `throughput` (mean over the window); `throughput_drop_percent` (`Percent`, **positive = slower at the end**: `(1 − mean(last 10 % of per-second points) / mean(first 10 %)) × 100`, at least one point each, `higher_is_better = false`); `throughput_cv` (coefficient of variation of the per-second points, %, lower is better) |

Scaling versus single-thread is **not** measured inside a multi-core run, so it is omitted
rather than faked. Compare it with a single-core run of the same methodology version.

If `throughput_drop_percent ≥ 10 %`, the sustained run adds the warning "Possibile
riduzione sostenuta delle prestazioni …, causa non determinabile". This is a warning,
never a throttling diagnosis.

**Series:**

- `throughput`: ops/s per second.
- `cpu_mhz`: average current MHz from `latest()`, when the monitor runs.
- `limit_mhz`: OS limit, read directly every 1 s; sustained runs only.

### Memory bandwidth (`MemoryBandwidth`)

- **Buffer.** Sized by the safety rule above (1 GiB on the test machine). It is allocated
  with `try_reserve_exact`, so a failed allocation is an error rather than an abort. It is
  zero-filled in 64 MiB steps, which touches every page before timing.
- **Threads.** All logical processors. Each worker owns one chunk of each half of the
  buffer.
- **Passes.** 10 timed passes each of read (wrapping `u64` sum), write (`fill`, standard
  stores) and copy (`copy_from_slice` from half A to half B, standard `memcpy`).
  Synchronization uses a breakable barrier, so a failed spawn cannot deadlock.
- **Timing.** Each worker timestamps its own chunk. Pass time = max(end) − min(start).
  (Timing from the coordinating thread was wrong: with 24 busy workers on 24 CPUs it woke
  late and inflated the rates to an impossible 309 GB/s.)
- **Bytes per pass.** Read: whole buffer. Write: whole buffer. Copy: half read + half
  written, which is also the whole buffer (STREAM-style accounting).
- **Metrics.** `read_bandwidth`, `write_bandwidth` and `copy_bandwidth` are the **median**
  (`BytesPerSecond`). `*_best` is the maximum. `buffer_size` is also reported.
  The headline is `read_bandwidth`.
- **Checks.** The copy is verified by sampling destination == source, and read sums go
  into a checksum.
- **Store type.** No non-temporal stores, so write results are lower than in tools that use
  streaming stores.
- **Warning.** It does not verify that the RAM is error-free.

### Memory latency (`MemoryLatency`)

- **Chain.** A single random cycle over the 64-byte cache lines of a 256 MiB buffer
  (Sattolo's algorithm with a fixed seed; the next index is stored in each line).
- **Measurement.** One untimed warm-up batch, then batches of 2^20 dependent loads for up
  to 2 s (at least 3 batches, at most 40). The result is the median ns per access.
- **Reference.** Also a 32 KiB in-cache chain.
- **Metrics.** `latency` (headline) and `latency_cache_32k`, both `Nanoseconds`, lower is
  better.
- **Warning.** It is labelled *indicativa*: it includes TLB effects with 4 KiB pages and
  does not verify that the RAM is error-free.

### Storage (`StorageSequential`, `StorageRandom`)

**Requirements.** `storage_write_consent == true`, and `storage_volume` must be a drive
root (`C`, `C:`, `C:\`) of a present fixed volume (`GetDriveTypeW == DRIVE_FIXED`).
Otherwise the request is `InvalidInput`.

**File size.** 256..=4096 MiB, default 1024 MiB. Tests can lower the minimum to 16 MiB
through a test-only hook.

**File location:**

- System volume: `%TEMP%`, provided it is on that volume.
- Other volumes: `<X:>\PCDiagnosticsBench\`. This directory is created when needed and
  removed when empty. If it is not writable the run fails with `PermissionDenied`.

**File handling:**

- Name: `PCDiagnosticsBench-<run id>.tmp`. Only a new file is ever created
  (`create_new`): no user data, no raw device access.
- Flags: `FILE_FLAG_NO_BUFFERING | FILE_FLAG_WRITE_THROUGH | FILE_FLAG_DELETE_ON_CLOSE`.
- Buffers are aligned to `max(4096, logical sector size)`.
- Data is incompressible xorshift output, and each block carries a unique (nonce, block)
  stamp so the drive cannot deduplicate it.

**Sequential test.** Write the whole file in 1 MiB blocks at QD1, then read it back. Each
block's stamp is verified on read; a mismatch fails the run.

- Metrics: `seq_write` (headline), `seq_read` and `bytes_written`.
- Series: `seq_write_rate` and `seq_read_rate`, one point per second.
- Bytes written = file size.

**Random test.**

1. Fill the file sequentially (untimed).
2. **4 KiB random reads, QD1**, for 5 s.
3. **4 KiB random reads with 4 threads**, each on its own unbuffered handle (synchronous
   handles serialize I/O), for 5 s.
4. **4 KiB random writes, QD1**, for 5 s or 256 MiB, whichever comes first.

- Metrics: `rand_read_qd1_iops` (headline), `rand_read_qd1` (B/s), `rand_read_qd1_latency`
  (µs), `rand_read_t4_iops`, `rand_read_t4`, `rand_write_qd1_iops`, `rand_write_qd1`,
  `rand_write_qd1_latency` and `bytes_written`.
- Bytes written ≤ file size + 256 MiB.

**Stale-file sweep** (at `BenchController::new`):

- Looks only in `%TEMP%` and in `<X:>\PCDiagnosticsBench` of each fixed volume.
- Deletes only regular files whose name exactly matches the prefix, a UUID-shaped run id
  and the `.tmp` suffix.
- A file that is still open (no `FILE_SHARE_DELETE`) cannot be deleted and is skipped.

**Caching.** Results reflect the device: the OS cache is bypassed (`NO_BUFFERING`). The
drive's own SLC/DRAM cache is not bypassed. A 1 GiB file usually fits in the SLC cache of
consumer SSDs, so sustained-write behaviour beyond the cache is not measured.

### GPU (`GpuCompute`)

Engine: `crates/bench/src/gpu.rs` (parent) and `crates/bench/src/gpu_worker/` (child).
Sensors and thresholds (NVML, WDDM/D3DKMT) and the job object:
`crates/platform/src/gpu_bench.rs`.

**Process isolation.** The workload runs in a child process: the app executable started
with `--pcdiag-gpu-worker` (`main` calls `bench::gpu_worker::run_if_requested()` first).
A driver fault (TDR, device removal, driver crash) can take that process down without
touching the app.

- Spawned with `CREATE_NO_WINDOW`, stdin/stdout/stderr piped, and placed in a job object
  with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`, so the worker cannot outlive the app even if
  the app crashes. The worker also stops on stdin EOF and when a stdout write fails.
- **Protocol** (private, versioned, `gpu_worker/protocol.rs`): newline-delimited JSON on
  stdout, `{"v":1,"type":...}`. Messages: `hello {pid}`, `adapters {adapters}` (probe),
  `selected {adapter, matchedBy}`, `measuring`, `progress {phase, fraction, flopsPerS}`,
  `done {result}`, `error {code, message, detail}`. Lines not starting with `{` are
  ignored; another version or an unparsable JSON line is a protocol error.
  Parent → worker: the line `cancel`, or EOF.
- **Cancellation:** write `cancel`, close stdin, wait up to 1 s, then kill. Measured: a
  cooperative worker stops in ~50 ms (test) / 171 ms from `cancel()` to `on_finished`
  (real GPU run, release); a worker that ignores the request is killed after the grace
  period (1.06 s in the test). Every path waits for the process and joins the two reader
  threads; tests assert the worker pid no longer exists when `on_finished` runs.
- **Abnormal end:** a worker that crashes, is killed, exits without `done`/`error`, sends
  a malformed line, or is silent for 20 s fails the run with "Il processo del benchmark
  GPU è terminato in modo anomalo (possibile reset del driver)"; `detail` carries the exit
  code (e.g. `0xC0000409` for an abort) and the tail of stderr.

**Which GPU.** The capability lists every **physical** DXGI adapter (neither software nor
virtual) in `gpuTargets`, in DXGI order, so the primary GPU (the UI's `primaryGpu` rule:
the first physical adapter) comes first and is the only one with `isDefault = true`. Each
target carries:

- `id` (`GpuInfo::id`, the DXGI LUID, e.g. `luid:00000000-00014DB8`) and `name`;
- its own `availability`: one probe-mode worker run lists every wgpu adapter, and each
  target is matched against that list with the rules below (one process start for all
  targets; the result is cached like the rest of the capability). `ProviderMissing` when
  no wgpu adapter matches that GPU, `Failed` when the probe itself failed;
- `stopTemperatureC`: the stop temperature from the threshold rule (Safety thresholds),
  only when the rule is in use (a threshold **and** a temperature reading at probe time);
  `null` means no temperature stop and the 30 s cap for that GPU.

The kind-level `availability` is `Available` if at least one target is, otherwise the
primary's reason (and its note, in the UI language, is appended to the description). The kind-level
`maxDurationS` is the longest any available target allows (60 s on the test machine);
`start` enforces the selected target's own cap (30 s without `stopTemperatureC`). The
description is generic: it no longer names the GPU or the stop temperature, the UI shows
them per target.

`BenchmarkRequest.gpuId` selects the target: `null` = the default target; an id that is
not a listed target, or one whose availability is `Unavailable`, is rejected with
`InvalidInput` ("the requested GPU is not among the physical GPUs available for the
benchmark", detail = the available ids and names; or "GPU <name> cannot be used for the
benchmark", detail = the reason). Like every `AppError` message these are technical
English; the UI shows a translated title per error code. Ids compare case-insensitively. A `gpuId` on any other
kind is `InvalidInput`. The resolved request stored in the result always carries the id
that was used. Since LUIDs change across reboots, `comparable_with` ignores `gpuId` and
matches the GPU through `context.gpuName` and `context.gpuDriverVersion` instead.

The worker receives the selected adapter's LUID and PCI ids and the LUIDs of **every
other** DXGI adapter as exclusions (software, virtual and the other physical GPUs), and
selects the wgpu adapter (`gpu_worker/select.rs`, pure and unit-tested):

1. Never a software adapter (wgpu `Cpu`, WARP / Microsoft Basic Render Driver, PCI vendor
   0x1414), a `VirtualGpu`, or any excluded LUID (the Parsec virtual display adapter
   copies the RTX's name and PCI ids, so only its LUID tells it apart).
2. A DX12 adapter whose DXGI LUID (read with `IDXGIAdapter2::GetDesc2` through
   `wgpu::Adapter::as_hal::<Dx12>`) equals the target → `matchedBy: luid`.
3. Otherwise exactly one eligible adapter per backend (DX12 first, then Vulkan) with the
   target's PCI vendor and device id and no conflicting LUID → `matchedBy: pciIds`.
   Two identical GPUs without LUIDs are ambiguous: no guess is made.

The adapter used is reported in `notes` (neutral information, not a warning), e.g.
"GPU adapter used: NVIDIA GeForce RTX 4070 SUPER (DX12, PCI 10DE:2783,
luid:00000000-00014DB8), selected by LUID." (Italian: "Adattatore GPU utilizzato: …,
selezionato per LUID."). Notes are only produced by completed runs;
other kinds have none. `BenchmarkContext.gpuName` and `gpuDriverVersion` describe the
**selected** adapter (looked up by the resolved `gpuId`), and `comparable_with` requires
both to match.

**Capability.** `GpuCompute` is available if DXGI reports at least one physical GPU
**and** wgpu (DX12/Vulkan) exposes a matching non-software, non-virtual adapter for at
least one of them. Otherwise `Unavailable` with the reason: `NotSupported` (no physical
GPU), `ProviderMissing("wgpu DX12/Vulkan adapter for <GPU>")` (Italian: "adattatore wgpu
DX12/Vulkan per <GPU>"; no matching wgpu adapter for the primary GPU), or `Failed(error)`
(DXGI or the probe process failed). The description explains it in the UI language. The
probe result is cached as structured data, so these texts follow a language change
without probing again. The worker reports its phases as stable keys (`select`,
`compile`, `warmup`, `measure`, `verify`); the parent words them.

**Workload** (`gpu_worker/kernel.rs`, WGSL compiled by wgpu 30.0.1; DX12 shaders through
naga → HLSL → FXC). Per invocation `i` (workgroup size 256):

1. Seed 8 independent `vec4<f32>` accumulators from an integer hash of `(i, lane)`
   (values in [0, 1)).
2. 512 iterations of `a = fma(a, M, C)` on each chain, with `M = (0.9990, 0.9992,
   0.9994, 0.9996)` and `C = (0.0010, 0.0007, 0.0004, 0.0001)`. The chains are
   independent (ILP). `M < 1` makes the map contractive, so values stay finite and
   rounding differences cannot grow.
3. Write the sum of all lanes to `out[i]`.

**FLOP accounting:** only FMAs are counted, 1 FMA = 2 FLOP:
8 chains × 4 lanes × 512 iterations × 2 = **32 768 FLOP per invocation**. The integer
seeding and the final 31 additions are not counted, so the figure slightly underestimates
the work. Unit: `FlopsPerSecond` (the formatter shows GFLOP/s / TFLOP/s). Earlier
builds stored the same values with `OperationsPerSecond`; the numbers and their definition
are unchanged (only the unit tag became specific), so the methodology version stays **1**
and old and new results remain comparable (comparisons match metrics by `id`).

**Dispatch and timing.**

- Instance flags are empty (no validation/debug layers, even in debug builds). Device
  limits = the adapter's limits; a 16 MiB output buffer (4 Mi invocations) is required.
- Warm-up 2 s (untimed): starting from 16 workgroups × 1 dispatch per submit, the
  dispatch grows (×2, up to 16 384 workgroups, while one dispatch stays under 20 ms),
  then the number of dispatches per submit grows (×2, up to 256) until a batch takes
  ≥ 20 ms (target ~40 ms). This keeps every GPU packet far below the Windows TDR limit
  (2 s) and cancellation fast. Frozen when the timed window starts.
- Timed window: `duration_s` (default 20, min 5, max 60; max 30 for a selected GPU without
  a usable driver threshold or without a temperature reading). Each batch = one command buffer, one `queue.submit`, then
  `device.poll(Wait, timeout 10 s)`; the wall clock is read around it. A poll timeout,
  device loss or any uncaptured wgpu error fails the run.
- Headline `throughput` = FLOPs of all timed batches / timed wall time. The CPU-side gap
  between batches (submit + wait, one batch in flight) is included, so the figure is
  slightly below the kernel-only rate. GPU timestamp queries are not used.

**Verification.** After the timed window, 64 output elements (both ends, workgroup
boundaries, middle, pseudo-random spread) are copied to a staging buffer, mapped and
compared with a CPU reference of the same kernel (`f32::mul_add`). Tolerance: relative
2e-3 (the contractive map bounds the fused/unfused FMA and summation-order differences to
~1.5e-4). A NaN, a missing value or any mismatch fails the run ("GPU result verification
failed …").

**Metrics** (headline first):

| id | Meaning |
|---|---|
| `throughput` | FP32 FLOP/s over the timed window (`FlopsPerSecond`) |
| `throughput_drop_percent` | Same definition as sustained CPU: positive = slower at the end, from the per-second points, `higher_is_better = false` |
| `throughput_cv` | Coefficient of variation of the per-second points (%) |

A drop ≥ 10 % adds "Possibile riduzione sostenuta delle prestazioni GPU …, causa non
determinabile".

**Series:** `throughput` (FLOP/s per second of the timed window, `FlopsPerSecond`), and
— whenever readings exist — `gpu_temp_c`, `gpu_clock_mhz`, `gpu_power_w` every second
during `Running` (from the live monitor's `latest()` sample for the selected GPU id, or a
direct sensor read when the monitor has no fresh value). Temperature and clock come from
NVML on NVIDIA and from the vendor-neutral WDDM performance data (D3DKMT) on AMD/Intel;
power only from NVML, so AMD/Intel runs have no `gpu_power_w` series.

## Results on the test machine

Release build, Ryzen 9 7900X, 31 GiB RAM, AC power, plan "Prestazioni elevate",
background CPU 2–5 %, limit = max = 4701 MHz.

| Kind | Result |
|---|---|
| CPU single-core (15 s) | 19 288 units/s. Per-second points 19 215–19 404; core ~5.2 GHz |
| CPU multi-core (24 threads, 15 s) | 371 546 units/s (15 481 per thread) |
| CPU sustained (24 threads, 60 s) | Mean 369 320 units/s; drop 0.93 %; CV 1.11 %; limit stayed at 4701 MHz |
| Memory bandwidth (1 GiB, 24 threads) | Read 60.85 GB/s, write 27.65 GB/s, copy 35.38 GB/s (median; best is within 2 %) |
| Memory latency | 97.6 ns (256 MiB, includes TLB); 0.91 ns at 32 KiB |
| Seagate ST2000DM008, F:, 1024 MiB, sequential | Write 179 MB/s, read 185 MB/s, 1024 MiB written |
| Seagate ST2000DM008, F:, 1024 MiB, random | Read QD1 119 IOPS / 8.4 ms; read with 4 threads 311 IOPS; write QD1 74 IOPS / 13.5 ms; 1025 MiB written |
| Samsung SSD 980 1TB, C:, **256 MiB**, sequential | Write 2.57 GB/s, read 2.22 GB/s (QD1, 1 MiB) |
| Samsung SSD 980 1TB, C:, **256 MiB**, random | Read QD1 15 850 IOPS (65 MB/s, 62.9 µs); read with 4 threads 63 459 IOPS (260 MB/s); write QD1 53 254 IOPS (218 MB/s, 18.5 µs); 512 MiB written |

The default 1024 MiB run on C: was **refused** by the free-space rule: 93.4 GiB were free
and 94.0 GiB were required (1 GiB file + 93.0 GiB, which is 10 % of 930 GiB). That is why
the C: figures above use 256 MiB.

The reserve was later capped at 20 GiB (orchestrator decision), so the default 1024 MiB run
on C: is allowed again (needs 21 GiB free).

GPU, release build, RTX 4070 SUPER (driver 617.42), DX12, selected by LUID (the Parsec
virtual adapter and the AMD iGPU were not used), default 20 s:

| Run | Result |
|---|---|
| GPU compute (20 s) | **23 228 GFLOP/s**; per-second points 23 190–23 287 GFLOP/s; drop 0.26 %; CV 0.11 %; total 25.7 s (3 s context window + process start + 2 s warm-up + 20 s + verification). An earlier run: 23 140 GFLOP/s, drop 0.50 %, CV 0.24 % |
| Temperature / clock / power during the run | 44 → 63 → 70 °C (stop point 90 °C, never reached); 2655–2700 MHz; 219–220 W (the board power limit) |
| Capability probe | 0.54–1.10 s (first `capabilities()` call). wgpu reports **two** DX12 adapters named "NVIDIA GeForce RTX 4070 SUPER" with PCI 10DE:2783 (LUIDs `…00014DB8` and `…000238F7`, the second being the virtual display adapter's), plus the AMD iGPU, WARP and the Vulkan RTX/AMD; the LUID match picked the physical one |
| Cancel after 6 s (10 s run) | `Cancelled`, 171 ms from `cancel()` to `on_finished`; no child process left |
| Child processes after the run / after the probe | none |
| GPU selector build, default target (RTX 4070 SUPER, `gpuId` resolved to `luid:00000000-00014DB8`), 20 s | **23 137 GFLOP/s**; points 23 055–23 236; drop 0.63 %; CV 0.23 %; 49 → 73 °C (stop 90 °C, NVML rule); 2640–2655 MHz; 219–220 W; total 25.7 s; no warnings; note "Adattatore GPU utilizzato: … selezionato per LUID"; context driver 32.0.16.1742 |
| Installed app (NSIS, per-user), started from the UI, 20 s | **21 770 GFLOP/s**; points 21 710–21 857; drop 0.44 %; CV 0.18 %; 51 → 74 °C; up to 2700 MHz; 220 W; background CPU 6.3 %; context: RTX 4070 SUPER, driver 32.0.16.1742; worker gone 4 s after the result. About 6 % below the runs above at the same power limit; cause not determined (run-to-run and thermal state differences are plausible) |

AMD Radeon(TM) Graphics iGPU (Ryzen 9 7900X, 2 CU RDNA 2, PCI 1002:164E, driver
32.0.21042.62), selected with `gpuId = luid:00000000-0001622A`, release, DX12, matched by
LUID. The driver reports no WDDM temperature thresholds but a readable temperature, so
`stopTemperatureC = null`, the 30 s cap applies and the warning is "Soglia termica non
fornita dal driver: applicati limiti di durata conservativi":

| Run | Result |
|---|---|
| iGPU compute (20 s) | **450.0 GFLOP/s**; per-second points 449–450 GFLOP/s; drop −0.03 % (no slowdown); CV 0.07 %; total 25.7 s |
| Temperature / clock during the run (WDDM) | 49 → 54 °C (no stop temperature); 600 MHz idle → 2200 MHz under load; no power series (NVML only) |
| Context | `gpuName` "AMD Radeon(TM) Graphics", `gpuDriverVersion` 32.0.21042.62 |
| Cancel after 6 s (20 s run) | `Cancelled`, 120 ms from `cancel()` to `on_finished`; partial temperature/clock series kept, no metrics |
| Invalid `gpuId` (`luid:00000000-DEADBEEF`) | `InvalidInput` "La GPU richiesta non è tra le GPU fisiche disponibili per il benchmark" (the message at the time; now English: "the requested GPU is not among …"), detail listing both targets |
| Capability probe (two targets) | 0.54–0.58 s; RTX `stopTemperatureC` 90, iGPU `null`; both available |
| Processes after the runs | `Get-CimInstance Win32_Process`: no `gpu_bench_probe.exe` and no `--pcdiag-gpu-worker` process left (only the separately installed app, without the worker flag) |

450 GFLOP/s is ~80 % of the iGPU's theoretical FP32 FMA peak at the observed clock (2 CU ×
64 lanes × 2 FLOP × 2.2 GHz ≈ 563 GFLOP/s); it is below peak, and the CPU-verified output
rules out elided work.

23.1 TFLOP/s is ~61 % of the theoretical FP32 peak at the observed clock (7168 lanes × 2
× 2.65 GHz ≈ 38 TFLOP/s): the card sat at its 220 W power limit, and FXC-compiled
`vec4` FMA chains do not reach peak issue rate. A value above peak would indicate elided
work; the CPU-verified output rules that out as well.

Binary size: `pc-diagnostics.exe` (release, LTO, stripped) grew from 13 217 280 to
18 677 760 bytes (**+5.2 MiB, +41 %**) with wgpu 30.0.1 (DX12 + Vulkan + WGSL/naga).

## Known limitations

- Sequential I/O is QD1 and synchronous, so NVMe results are lower than with high-QD tools
  (the 980 is rated ~3.5 GB/s at high QD).
- On volumes under 200 GiB the reserve is 10 % (minimum 2 GiB); above that it is capped at
  20 GiB.
- Memory write and copy use standard stores, not non-temporal ones.
- Latency includes TLB misses (no large pages, which would need a privilege).
- Systems with more than 64 logical processors are untested.
- The CPU temperature is not observable; only duration caps and the frequency-limit check
  protect sustained runs.
- GPU: one batch in flight (submit + wait), no GPU timestamp queries; FXC (not DXC)
  compiles the DX12 shader. The Vulkan fallback can only match by PCI ids.
- GPU: the capability probe (targets, availability, thresholds) is cached per
  controller. A LUID that changes after a driver reset falls back to the PCI-id match for
  the cached target; a `gpuId` taken from a newer inventory than the cached probe is
  rejected as unknown until the app restarts.
- GPU: temperature protection needs a driver threshold: NVML (NVIDIA) or a WDDM
  `TemperatureWarning`. GPUs whose driver reports neither (the AMD iGPU on the test
  machine) get no temperature stop and the 30 s cap instead.
- GPU: the kind-level `maxDurationS` is the maximum over the available targets; a UI must
  cap the slider at 30 s for a target whose `stopTemperatureC` is `null` (the backend
  rejects longer requests).
