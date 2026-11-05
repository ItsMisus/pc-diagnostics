# Hardware inventory and live monitoring

Phase 2 backend: `crates/platform` (OS access) and `crates/monitor` (sampler). Every
value is either measured or `Metric::Unavailable { reason }`. Nothing is simulated, and
no admin rights are needed.

## `monitor` API

```rust
pub struct MonitorConfig { pub interval_ms: u32, pub history_capacity: usize }
impl Monitor {
    pub fn start<F>(config: MonitorConfig, on_sample: F) -> Result<Monitor, AppError>
        where F: Fn(&LiveSample) + Send + 'static;
    pub fn set_interval(&self, interval_ms: u32) -> Result<(), AppError>;
    pub fn history(&self) -> Vec<LiveSample>;   // oldest first, at most history_capacity
    pub fn status(&self) -> MonitorStatus;
    pub fn inventory(&self, refresh: bool) -> Result<HardwareInventory, AppError>;
    pub fn app_overhead(&self) -> AppOverhead;
    pub fn shutdown(self);                       // Drop also stops and joins
}
```

- `interval_ms` must be within `SAMPLING_INTERVAL_MIN_MS..=SAMPLING_INTERVAL_MAX_MS`
  (500–10 000). `history_capacity` must be within `1..=86_400`. Otherwise `InvalidInput`.
- `on_sample` runs on the sampler thread after the sample is in the ring buffer. Keep it
  short. A panic inside it is caught and logged.
- `inventory(false)` returns the cached inventory, or collects it on the first call.
  `inventory(true)` collects it again. Collection takes about 40 ms on the development
  machine (WMI dominates, and it can be much slower on a cold WMI service). It blocks the
  caller, so call it from `spawn_blocking`.
- `Monitor` is `Send + Sync`.

### Sampler thread

- A dedicated `std::thread` blocks in `recv_timeout` on a control channel (`Stop`,
  `SetInterval`). It never busy-waits, and stop and interval changes take effect
  immediately.
- Deadlines are absolute (`scheduled + interval`), so the cost of a sample does not
  drift the cadence. A deadline that has already passed is skipped, not bursted.
- Provider initialization runs on the sampler thread and takes about 0.4 s on Windows
  (PDH loads the processor objects). The first sample comes `min(interval, 1 s)` after
  initialization, because rate counters need a baseline window.
- `seq` starts at 1 and increases monotonically. `at` comes from
  `models::time::now_unix_ms()`.
- The loop never panics. A panicking collection is caught, logged, and the tick is
  skipped. A failed metric becomes `Unavailable(Failed(..))`.
- App overhead is measured on the same thread every `OVERHEAD_WINDOW` (5 s), at the next
  tick after the window elapses. `window_ms` is the real window length.

## Data sources

| Metric | Source | Notes |
|---|---|---|
| CPU usage, total and per logical processor | PDH `\Processor(*)\% Processor Time`, from **raw** values | See "CPU usage" below |
| CPU current MHz | PDH `\Processor Information(_Total)\% Processor Performance` × average `MaxMhz` | Same method as Task Manager |
| CPU max / limit MHz | `CallNtPowerInformation(ProcessorInformation)`, averaged across logical processors | Inventory `max_mhz` uses the highest `MaxMhz` |
| CPU temperature, power | — | `ProviderMissing("CPU sensor provider (requires a kernel-mode driver; not bundled)")`. `MSAcpi_ThermalZoneTemperature` is deliberately not used: it is a motherboard zone, not the CPU |
| RAM total / used / available | `sysinfo` (GlobalMemoryStatusEx) | used = total − available |
| Commit used / limit | PDH `\Memory\Committed Bytes`, `\Memory\Commit Limit` | Same values as `GetPerformanceInfo`; PDH is used because `Source` has no Win32 variant |
| Pages/sec | PDH `\Memory\Pages/sec` | |
| GPU usage | PDH `\GPU Engine(*)\Utilization Percentage` | Summed per (adapter LUID, engine type), then the maximum across engine types (Task Manager convention), clamped to 0–100 |
| GPU dedicated memory used | PDH `\GPU Adapter Memory(*)\Dedicated Usage` | Summed per LUID |
| NVIDIA temperature, graphics clock, power | NVML (`nvml.dll`, loaded at runtime) | See "NVML" below |
| GPU temperature, clock (any vendor), power share, fan | D3DKMT `D3DKMTQueryAdapterInfo` (`ADAPTERPERFDATA`, `NODEPERFDATA` node 0, `ADAPTERPERFDATA_CAPS`), the WDDM 2.4+ data Task Manager shows; no elevation | Source `d3dkmt`. Primary source for AMD/Intel; on NVIDIA, NVML stays primary for temperature/clock/watts and D3DKMT is the fallback when NVML is missing. See "WDDM performance data" below |
| AMD / Intel GPU power in watts | — | `NotImplemented`: WDDM reports power only as a share of the adapter maximum (`powerPercent`); watts would need ADLX / IGCL |
| Disk read/write bytes per second, active time | PDH `\PhysicalDisk(_Total)\…`; active time = 100 − `% Idle Time` | |
| GPU inventory | DXGI `EnumAdapters1` + `CheckInterfaceSupport` (UMD version) + D3DKMT (PCI location, adapter type) | All adapters are listed |
| RAM modules | WMI `Win32_PhysicalMemory` | Serial numbers are not collected |
| Physical disks | WMI `ROOT\Microsoft\Windows\Storage` `MSFT_PhysicalDisk` | Access denied → `PermissionDenied` |
| Volumes | `sysinfo` Disks | |
| App overhead | Toolhelp32 process tree + `sysinfo` for our own pids | See "App overhead" below |

All PDH counters are added with `PdhAddEnglishCounterW`, so they work on localized
Windows (for example Italian). A counter object missing from the OS maps to
`NotSupported`. Any other PDH error maps to `Failed` with the PDH status in `detail`.

### GPU ids

`GpuInfo::id` and `GpuSample::id` are both `luid:<HIGH>-<LOW>` (8 upper-case hex digits
each). DXGI `LUID { HighPart, LowPart }` and PDH instance names
(`…_luid_0x<high>_0x<low>_…`) go through the same formatter, `convert::gpu_id`.

An adapter that PDH knows (it has a `GPU Adapter Memory` instance) but that has no
`GPU Engine` instances has no client processes, so it reports 0 %. An adapter that PDH
does not know reports `NotSupported`.

### CPU usage

PDH's formatted value for `% Processor Time` is `PDH_CSTATUS_INVALID_DATA` for
roughly 10–15 % of per-core readings on the development machine (Ryzen 9 7900X, at both
0.5 s and 1 s intervals). Windows accounts idle time in scheduler-tick quanta, so on an
idle core the idle delta can exceed the elapsed delta. The monitor reads raw values
instead and computes `100 × (1 − Δidle / Δtime)`, clamped to 0–100. A window whose idle
time overshoots was fully idle, so it clamps to 0 % instead of being dropped. The
formula was checked against PDH's own formatted values for valid instances. `sysinfo`
is not used for CPU on Windows, because it reports `0.0` when PDH returns an error.

### CPU frequency

`PROCESSOR_POWER_INFORMATION.CurrentMhz` stays at `MaxMhz` (4701 MHz on the
development machine) on Windows 11, whether the CPU is idle or boosting, so it is not
used for `current_mhz`. The monitor uses `% Processor Performance` × `MaxMhz` instead,
which is how Task Manager computes "Speed". It measured about 4.9–5.2 GHz on the same
machine. `limit_mhz` below `max_mhz` signals a limit, not its cause.

### NVML

- `nvml.dll` is loaded only from absolute paths: `GetSystemDirectoryW()\nvml.dll`, then
  `%ProgramW6432%\NVIDIA Corporation\NVSMI\nvml.dll`. A DLL planted next to the
  executable is never loaded. If no NVIDIA adapter is present, NVML is not loaded at all.
- NVML devices are matched to DXGI adapters by **PCI bus/device/function**. On the DXGI
  side the location comes from `D3DKMTQueryAdapterInfo(KMTQAITYPE_ADAPTERADDRESS)`. If an
  adapter has no location, the fallback is a vendor+device id that is unique on both
  sides. Anything ambiguous stays unmatched and reports
  `Failed(NotFound "No unambiguous NVML device matches this DXGI adapter")`.
  The matcher never guesses (`convert::match_nvml_devices`, unit-tested).
- NVML errors map as follows: `NotSupported` → `NotSupported`, `NoPermission` →
  `PermissionDenied`, anything else → `Failed`. If the DLL is missing, the reason is
  `ProviderMissing("nvml.dll")`.

### Virtual adapters

- The Microsoft Basic Render Driver (WARP) has `is_software = true` (DXGI software flag,
  D3DKMT software bit, or vendor 0x1414 / device 0x8C). Its sensors are `NotSupported`.
- Indirect-display adapters (D3DKMT `IndirectDisplayDevice` bit, for example the Parsec
  Virtual Display) are listed, because DXGI reports them. They often carry the
  description and PCI ids of the GPU that renders for them. On the development machine,
  a second "NVIDIA GeForce RTX 4070 SUPER" appears this way. They get no telemetry
  sources and all live metrics are `NotSupported`. NVML matching ignores them.

### App overhead

- `core` is this process. `webview` is every descendant process (WebView2
  `msedgewebview2.exe` children and grandchildren), found with a Toolhelp32 snapshot,
  which opens no handles. Parent links are checked against start times, so a child is
  never attributed to a recycled parent pid.
- CPU % = Δ CPU time / (window × logical processors): a share of the whole machine,
  0–100.
- A descendant that exits inside a window is not counted for that window. A descendant
  first seen inside a window contributes all of its CPU time.
- Before the first full window, CPU is `Failed(NotFound "No completed measurement window
  yet")`, with `retriable = true`.

## Measured cost (development machine, Windows 11, Ryzen 9 7900X, 24 logical CPUs)

| | debug | release |
|---|---|---|
| `LiveCollector::sample()`: min / median / max | 2.10 / 2.40 / 3.88 ms | 1.37 / 1.54 / 3.33 ms |
| `LiveCollector::new()` (provider init) | ~0.39 s | ~0.37–0.40 s |
| `OverheadTracker::measure()` (every 5 s) | ~14–20 ms | ~16–19 ms (Toolhelp snapshot ≈ 10 ms) |
| Whole process at 1 s interval, 11.5 s run (monitor's own overhead window) | 0.022 % of machine | 0.011 % of machine |

Reproduce with `cargo run [--release] -p monitor --example probe -- <interval_ms> <count>`.

## WDDM performance data

Raw values and conversions (`convert::kmt_*`, unit-tested):

| Field | Raw unit | Rule |
|---|---|---|
| `Temperature` | tenths of °C | `0` = not reported (`NotSupported`); above 150 °C rejected |
| Node 0 `Frequency` | Hz | Reported only when the node's `MaxFrequency` is non-zero |
| `Power` | tenths of a percent of the adapter's maximum | `0` = not reported; above 200 % rejected. Not convertible to watts (the reference is not exposed) |
| `FanRPM` | rpm | Only when caps `MaxFanRPM > 0`; then `0` is a real reading (fan stopped) |
| caps `TemperatureWarning` / `TemperatureMax` | tenths of °C | Used by the GPU benchmark as a driver threshold when NVML has none |

Handles are opened once per physical adapter when the sampler starts (software and
indirect-display adapters are skipped). Readings on the development machine, idle:

| Adapter | Temperature | Clock | Power share | Fan | Caps |
|---|---|---|---|---|---|
| RTX 4070 SUPER | 49.0 °C (NVML: 49 °C) | 210 MHz (NVML: 210 MHz) | 12.1 % (NVML: 26.6 W, which is 12.1 % of the 220 W limit) | 0 rpm (fan stop) | warning 84 °C, max 90 °C, max fan 3200 rpm |
| Radeon iGPU (Raphael) | 49–50 °C | 600 MHz (max 2200) | 0.1–5.8 % | not reported | no thresholds |

The temperature is the value the driver chooses to report (typically the GPU hotspot or
edge sensor; the driver does not say which). Intel GPUs use the same code path but were
not available for testing.

## Limitations

- CPU temperature and package power need a kernel-mode driver, which is not bundled.
- AMD/Intel GPU power is a share of the maximum, not watts (no ADLX / IGCL). Intel GPU
  sensors use the vendor-neutral WDDM path but were not tested on Intel hardware.
- Two identical NVIDIA GPUs without a D3DKMT PCI location are left unmatched on purpose.
- The DXGI adapter list is read when the sampler starts. A GPU hot-plugged later
  appears only after the monitor restarts. The inventory picks it up on
  `inventory(true)`.
- Per-core CPU covers processor group 0 and any further groups that the `Processor`
  object exposes. Systems with more than 64 logical processors have not been tested.
- `MemoryModule.capacity_bytes` is 0 if WMI omits `Capacity`. This was not seen in
  testing.
- Non-Windows builds compile the cross-platform paths (sysinfo CPU and RAM). Every
  Windows-only value there is `NotSupported`. This was not built on a non-Windows target
  in this phase, because no such target is installed.
