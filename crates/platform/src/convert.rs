//! Pure conversion helpers shared by the OS providers.
//!
//! Nothing here touches the OS, so everything is unit-tested on every platform.

use std::collections::HashMap;

use models::hardware::{BusKind, DiskHealthStatus, MediaKind};

/// Adapter LUID as `(high, low)`. `high` is `LUID.HighPart` reinterpreted as `u32`.
pub type Luid = (u32, u32);

/// `GpuInfo::id` / `GpuSample::id` for an adapter LUID: `luid:<high>-<low>`, 8 upper-case
/// hex digits each. Both DXGI and PDH go through this function so the ids always match.
/// WDDM temperature (tenths of °C). `0` means "not reported"; values above 150 °C are
/// treated as invalid rather than shown.
pub fn kmt_temperature_c(deci_c: u32) -> Option<f64> {
    let c = f64::from(deci_c) / 10.0;
    (deci_c > 0 && c <= 150.0).then_some(c)
}

/// WDDM power draw (tenths of a percent of the adapter's maximum). `0` means "not
/// reported"; above 200 % is treated as invalid.
pub fn kmt_power_percent(deci_percent: u32) -> Option<f64> {
    let p = f64::from(deci_percent) / 10.0;
    (deci_percent > 0 && p <= 200.0).then_some(p)
}

/// WDDM node frequency (Hz) to MHz. A node whose maximum is 0 does not report clocks.
#[allow(clippy::cast_precision_loss)]
pub fn kmt_clock_mhz(freq_hz: u64, max_hz: u64) -> Option<f64> {
    (freq_hz > 0 && max_hz > 0).then(|| freq_hz as f64 / 1_000_000.0)
}

pub fn gpu_id(luid: Luid) -> String {
    format!("luid:{:08X}-{:08X}", luid.0, luid.1)
}

/// Extracts the adapter LUID from a PDH GPU instance name, e.g.
/// `pid_1234_luid_0x00000000_0x0000D1B3_phys_0_eng_0_engtype_3D` or
/// `luid_0x00000000_0x0000D1B3_phys_0`.
pub fn parse_pdh_luid(instance: &str) -> Option<Luid> {
    let start = instance.find("luid_0x")? + "luid_0x".len();
    let rest = &instance[start..];
    let (high, rest) = rest.split_once("_0x")?;
    let low = rest.split('_').next()?;
    let high = u32::from_str_radix(high, 16).ok()?;
    let low = u32::from_str_radix(low, 16).ok()?;
    Some((high, low))
}

/// Engine type of a `GPU Engine` instance (`3D`, `Copy`, `VideoDecode`, `Compute_0`, …).
/// It is always the last component, and may itself contain `_` or spaces.
pub fn parse_engine_type(instance: &str) -> Option<&str> {
    let start = instance.find("engtype_")? + "engtype_".len();
    Some(&instance[start..])
}

/// Task Manager convention for adapter utilization: sum the per-process engine
/// instances per `(adapter, engine type)`, then take the busiest engine type.
/// Instances whose name does not parse are ignored. Result is clamped to 0–100.
pub fn aggregate_engine_utilization<'a>(
    items: impl IntoIterator<Item = (&'a str, f64)>,
) -> HashMap<Luid, f64> {
    let mut per_engine: HashMap<(Luid, &'a str), f64> = HashMap::new();
    for (name, value) in items {
        if !value.is_finite() {
            continue;
        }
        let (Some(luid), Some(eng)) = (parse_pdh_luid(name), parse_engine_type(name)) else {
            continue;
        };
        *per_engine.entry((luid, eng)).or_insert(0.0) += value;
    }
    let mut per_adapter: HashMap<Luid, f64> = HashMap::new();
    for ((luid, _), sum) in per_engine {
        let e = per_adapter.entry(luid).or_insert(0.0);
        *e = e.max(sum);
    }
    for v in per_adapter.values_mut() {
        *v = v.clamp(0.0, 100.0);
    }
    per_adapter
}

/// Sums instance values per adapter LUID (e.g. `GPU Adapter Memory` has one instance per
/// physical adapter part: `luid_..._phys_0`).
pub fn sum_per_luid<'a>(items: impl IntoIterator<Item = (&'a str, f64)>) -> HashMap<Luid, f64> {
    let mut out = HashMap::new();
    for (name, value) in items {
        if !value.is_finite() {
            continue;
        }
        if let Some(luid) = parse_pdh_luid(name) {
            *out.entry(luid).or_insert(0.0) += value;
        }
    }
    out
}

/// Clamps a percentage to 0–100. Non-finite input yields `None` (never a made-up 0).
pub fn clamp_percent(v: f64) -> Option<f64> {
    v.is_finite().then(|| v.clamp(0.0, 100.0))
}

/// `100 - idle`, clamped. Used for disk active time from `% Idle Time`.
pub fn active_from_idle(idle_percent: f64) -> Option<f64> {
    clamp_percent(100.0 - idle_percent)
}

/// CPU share of the whole machine from CPU time consumed over a wall-clock window.
/// `cpu_ms` is summed across all threads, so it can exceed `wall_ms` on multi-core hosts.
pub fn cpu_share_percent(cpu_ms: u64, wall_ms: u64, logical_cpus: usize) -> Option<f64> {
    if wall_ms == 0 || logical_cpus == 0 {
        return None;
    }
    #[allow(clippy::cast_precision_loss)] // ms counts are far below 2^52
    let pct = cpu_ms as f64 * 100.0 / (wall_ms as f64 * logical_cpus as f64);
    clamp_percent(pct)
}

/// Arithmetic mean of non-zero readings. Zero means "not reported" for
/// `PROCESSOR_POWER_INFORMATION` fields, so zeros are excluded rather than averaged in.
pub fn average_nonzero(values: impl IntoIterator<Item = u32>) -> Option<f64> {
    let (sum, n) = values
        .into_iter()
        .filter(|v| *v > 0)
        .fold((0u64, 0u64), |(s, n), v| (s + u64::from(v), n + 1));
    #[allow(clippy::cast_precision_loss)]
    (n > 0).then(|| sum as f64 / n as f64)
}

/// Formats the user-mode driver version returned by
/// `IDXGIAdapter::CheckInterfaceSupport` (four 16-bit fields, most significant first).
pub fn format_umd_version(v: i64) -> String {
    #[allow(clippy::cast_sign_loss)]
    let v = v as u64;
    format!(
        "{}.{}.{}.{}",
        (v >> 48) & 0xFFFF,
        (v >> 32) & 0xFFFF,
        (v >> 16) & 0xFFFF,
        v & 0xFFFF
    )
}

/// `MSFT_PhysicalDisk.MediaType`.
pub fn media_kind_from_msft(media_type: u16) -> MediaKind {
    match media_type {
        3 => MediaKind::Hdd,
        4 => MediaKind::Ssd,
        _ => MediaKind::Unknown,
    }
}

/// `MSFT_PhysicalDisk.BusType` (STORAGE_BUS_TYPE values).
pub fn bus_kind_from_msft(bus_type: u16) -> BusKind {
    match bus_type {
        7 => BusKind::Usb,
        8 => BusKind::Raid,
        10 => BusKind::Sas,
        11 => BusKind::Sata,
        14 | 15 => BusKind::Virtual,
        17 => BusKind::Nvme,
        _ => BusKind::Other,
    }
}

/// `MSFT_PhysicalDisk.HealthStatus`.
pub fn health_from_msft(status: u16) -> DiskHealthStatus {
    match status {
        0 => DiskHealthStatus::Healthy,
        1 => DiskHealthStatus::Warning,
        2 => DiskHealthStatus::Unhealthy,
        _ => DiskHealthStatus::Unknown,
    }
}

/// `Win32_PhysicalMemory.FormFactor`. `0` (Unknown) yields `None`.
pub fn memory_form_factor(code: u16) -> Option<&'static str> {
    Some(match code {
        1 => "Other",
        2 => "SIP",
        3 => "DIP",
        4 => "ZIP",
        5 => "SOJ",
        6 => "Proprietary",
        7 => "SIMM",
        8 => "DIMM",
        9 => "TSOP",
        10 => "PGA",
        11 => "RIMM",
        12 => "SODIMM",
        13 => "SRIMM",
        14 => "SMD",
        15 => "SSMP",
        16 => "QFP",
        17 => "TQFP",
        18 => "SOIC",
        19 => "LCC",
        20 => "PLCC",
        21 => "BGA",
        22 => "FPBGA",
        23 => "LGA",
        _ => return None,
    })
}

/// Trims a WMI/driver string; empty or whitespace-only becomes `None`.
pub fn non_empty(s: Option<String>) -> Option<String> {
    s.map(|s| s.trim().to_owned()).filter(|s| !s.is_empty())
}

/// `\Processor Information(*)` instance `"<group>,<index>"` → `(group, index)`.
/// Totals (`_Total`, `0,_Total`) yield `None`.
pub fn parse_processor_instance(name: &str) -> Option<(u32, u32)> {
    match name.split_once(',') {
        Some((g, i)) => Some((g.trim().parse().ok()?, i.trim().parse().ok()?)),
        None => Some((0, name.trim().parse().ok()?)),
    }
}

/// Splits `\Processor Information(*)\% Processor Time` items into the machine total
/// (`_Total`) and per-logical-processor values ordered by `(group, index)`, all clamped
/// to 0–100. Per-core is `None` if any core lacks a valid value this round (a partial
/// vector would misalign core indices).
pub fn split_processor_items(items: &[(String, Option<f64>)]) -> (Option<f64>, Option<Vec<f64>>) {
    let total = items
        .iter()
        .find(|(n, _)| n == "_Total")
        .and_then(|(_, v)| clamp_percent((*v)?));
    let mut cores: Vec<((u32, u32), Option<f64>)> = items
        .iter()
        .filter_map(|(n, v)| Some((parse_processor_instance(n)?, *v)))
        .collect();
    cores.sort_by_key(|(k, _)| *k);
    let per_core: Option<Vec<f64>> = cores.iter().map(|(_, v)| clamp_percent((*v)?)).collect();
    (total, per_core.filter(|c| !c.is_empty()))
}

/// Busy % from two raw `PERF_100NSEC_TIMER_INV` readings `(idle, time base)`, both in
/// 100 ns units: `100 × (1 − Δidle / Δtime)`, clamped to 0–100. Δidle may slightly exceed
/// Δtime on an idle core (tick-quantized idle accounting): that window clamps to 0 %.
/// `None` if time did not advance or a counter went backwards (reset).
pub fn busy_percent_from_raw(prev: (i64, i64), cur: (i64, i64)) -> Option<f64> {
    let d_idle = cur.0.checked_sub(prev.0)?;
    let d_time = cur.1.checked_sub(prev.1)?;
    if d_time <= 0 || d_idle < 0 {
        return None;
    }
    #[allow(clippy::cast_precision_loss)]
    clamp_percent(100.0 * (1.0 - d_idle as f64 / d_time as f64))
}

/// PCI `(bus, device, function)`.
pub type PciLoc = (u32, u32, u32);

/// A DXGI adapter as seen by the NVML matcher.
#[derive(Debug, Clone, Copy)]
pub struct DxgiGpuKey {
    pub luid: Luid,
    /// `(device_id << 16) | vendor_id`, the layout of NVML's `pci_device_id`.
    pub pci_id: u32,
    pub location: Option<PciLoc>,
    /// Indirect-display (virtual display) adapters have no hardware of their own.
    pub indirect_display: bool,
}

/// An NVML device as seen by the matcher.
#[derive(Debug, Clone, Copy)]
pub struct NvmlGpuKey {
    pub index: u32,
    pub pci_id: u32,
    pub location: Option<PciLoc>,
}

/// Matches NVML devices to DXGI adapters. Primary key: PCI location (bus/device/function,
/// from D3DKMT on the DXGI side). Fallback when the location is unknown: the PCI
/// vendor+device id, only if it is unique on both sides. Anything ambiguous stays
/// unmatched; an NVML device is never assigned to two adapters.
pub fn match_nvml_devices(dxgi: &[DxgiGpuKey], nvml: &[NvmlGpuKey]) -> HashMap<Luid, u32> {
    let mut out = HashMap::new();
    let mut used = std::collections::HashSet::new();
    let candidates: Vec<&DxgiGpuKey> = dxgi.iter().filter(|d| !d.indirect_display).collect();
    for d in &candidates {
        let hits: Vec<&NvmlGpuKey> = match d.location {
            Some(loc) => nvml
                .iter()
                .filter(|n| n.location == Some(loc) && n.pci_id == d.pci_id)
                .collect(),
            None => {
                let same_id_dxgi = candidates.iter().filter(|o| o.pci_id == d.pci_id).count();
                if same_id_dxgi == 1 {
                    nvml.iter().filter(|n| n.pci_id == d.pci_id).collect()
                } else {
                    Vec::new()
                }
            }
        };
        if let [n] = hits.as_slice()
            && used.insert(n.index)
        {
            out.insert(d.luid, n.index);
        }
    }
    out
}

/// PCI function from an NVML bus id such as `00000000:01:00.0`.
pub fn parse_pci_function(bus_id: &str) -> Option<u32> {
    bus_id
        .rsplit_once('.')?
        .1
        .trim_end_matches('\0')
        .parse()
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kmt_conversions_reject_unreported_and_implausible_values() {
        // Values read on the test machine (RTX 4070 SUPER, Radeon iGPU).
        assert_eq!(kmt_temperature_c(477), Some(47.7));
        assert_eq!(kmt_temperature_c(500), Some(50.0));
        assert_eq!(kmt_temperature_c(0), None);
        assert_eq!(kmt_temperature_c(1500), Some(150.0));
        assert_eq!(kmt_temperature_c(1501), None);
        assert_eq!(kmt_power_percent(123), Some(12.3));
        assert_eq!(kmt_power_percent(0), None);
        assert_eq!(kmt_power_percent(2001), None);
        assert_eq!(kmt_clock_mhz(600_000_000, 2_200_000_000), Some(600.0));
        assert_eq!(kmt_clock_mhz(0, 0), None);
        assert_eq!(kmt_clock_mhz(210_000_000, 0), None);
    }

    #[test]
    fn nvml_matching_by_location_ignores_indirect_display() {
        let id = (0x2783 << 16) | 0x10DE;
        let dxgi = [
            DxgiGpuKey {
                luid: (0, 1),
                pci_id: id,
                location: Some((1, 0, 0)),
                indirect_display: false,
            },
            // Virtual display adapter reported with the same name/ids, no PCI location.
            DxgiGpuKey {
                luid: (0, 2),
                pci_id: id,
                location: None,
                indirect_display: true,
            },
        ];
        let nvml = [NvmlGpuKey {
            index: 0,
            pci_id: id,
            location: Some((1, 0, 0)),
        }];
        let m = match_nvml_devices(&dxgi, &nvml);
        assert_eq!(m.len(), 1);
        assert_eq!(m[&(0, 1)], 0);
    }

    #[test]
    fn nvml_matching_refuses_ambiguity() {
        let id = (0x2684 << 16) | 0x10DE;
        // Two identical cards, locations unknown: no guess.
        let two = [
            DxgiGpuKey {
                luid: (0, 1),
                pci_id: id,
                location: None,
                indirect_display: false,
            },
            DxgiGpuKey {
                luid: (0, 2),
                pci_id: id,
                location: None,
                indirect_display: false,
            },
        ];
        let nvml = [
            NvmlGpuKey {
                index: 0,
                pci_id: id,
                location: Some((1, 0, 0)),
            },
            NvmlGpuKey {
                index: 1,
                pci_id: id,
                location: Some((2, 0, 0)),
            },
        ];
        assert!(match_nvml_devices(&two, &nvml).is_empty());
        // With locations both match exactly.
        let located = [
            DxgiGpuKey {
                location: Some((2, 0, 0)),
                ..two[0]
            },
            DxgiGpuKey {
                location: Some((1, 0, 0)),
                ..two[1]
            },
        ];
        let m = match_nvml_devices(&located, &nvml);
        assert_eq!(m[&(0, 1)], 1);
        assert_eq!(m[&(0, 2)], 0);
        // A single card without location falls back to the unique PCI id.
        let m = match_nvml_devices(&two[..1], &nvml[..1]);
        assert_eq!(m[&(0, 1)], 0);
    }

    #[test]
    fn processor_items_split_and_order() {
        let items: Vec<(String, Option<f64>)> = [
            ("0,10", Some(5.0)),
            ("_Total", Some(12.5)),
            ("0,2", Some(101.0)),
            ("1,0", Some(7.0)),
            ("0,_Total", Some(12.0)),
            ("0,0", Some(-1.0)),
        ]
        .into_iter()
        .map(|(n, v)| (n.to_owned(), v))
        .collect();
        let (total, cores) = split_processor_items(&items);
        assert_eq!(total, Some(12.5));
        assert_eq!(
            cores,
            Some(vec![0.0, 100.0, 5.0, 7.0]),
            "(group, index) order, clamped"
        );
        assert_eq!(parse_processor_instance("0,_Total"), None);
        let bad = vec![("0,0".to_owned(), Some(f64::NAN))];
        assert_eq!(split_processor_items(&bad), (None, None));
        let missing = vec![
            ("_Total".to_owned(), Some(3.0)),
            ("0,0".to_owned(), Some(1.0)),
            ("0,1".to_owned(), None),
        ];
        assert_eq!(split_processor_items(&missing), (Some(3.0), None));
    }

    #[test]
    fn busy_percent_from_raw_values() {
        // Real readings from a host (core 3): Δidle 3_906_250, Δtime 5_031_020 → 22.36 %.
        let prev = (2_245_989_375_000, 134_382_108_525_826_647);
        let cur = (2_245_993_281_250, 134_382_108_530_857_667);
        let v = busy_percent_from_raw(prev, cur).expect("value");
        assert!((v - 22.3567).abs() < 1e-3, "{v}");
        // Quantized idle overshoot clamps to 0 %.
        assert_eq!(busy_percent_from_raw((0, 0), (1_100, 1_000)), Some(0.0));
        assert_eq!(busy_percent_from_raw((0, 0), (0, 1_000)), Some(100.0));
        assert_eq!(
            busy_percent_from_raw((0, 5), (10, 5)),
            None,
            "no time elapsed"
        );
        assert_eq!(busy_percent_from_raw((10, 0), (5, 1_000)), None, "reset");
    }

    #[test]
    fn pci_function_parsing() {
        assert_eq!(parse_pci_function("00000000:01:00.0"), Some(0));
        assert_eq!(parse_pci_function("00000000:01:00.1"), Some(1));
        assert_eq!(parse_pci_function("garbage"), None);
    }

    const ENG_3D: &str = "pid_1234_luid_0x00000000_0x0000D1B3_phys_0_eng_0_engtype_3D";

    #[test]
    fn parses_engine_instance() {
        assert_eq!(parse_pdh_luid(ENG_3D), Some((0, 0xD1B3)));
        assert_eq!(parse_engine_type(ENG_3D), Some("3D"));
        let vd = "pid_8_luid_0x00000001_0xABCDEF01_phys_0_eng_3_engtype_Video Decode";
        assert_eq!(parse_pdh_luid(vd), Some((1, 0xABCD_EF01)));
        assert_eq!(parse_engine_type(vd), Some("Video Decode"));
    }

    #[test]
    fn parses_adapter_memory_instance() {
        assert_eq!(
            parse_pdh_luid("luid_0x00000000_0x00011C4A_phys_0"),
            Some((0, 0x11C4A))
        );
    }

    #[test]
    fn rejects_malformed_instances() {
        assert_eq!(parse_pdh_luid("_Total"), None);
        assert_eq!(parse_pdh_luid("luid_0xZZ_0x01"), None);
        assert_eq!(parse_pdh_luid("luid_0x0000"), None);
        assert_eq!(parse_engine_type("luid_0x0_0x1_phys_0"), None);
    }

    #[test]
    fn id_is_consistent_between_dxgi_and_pdh() {
        // DXGI: LUID { LowPart: 0xD1B3, HighPart: 0 }.
        let dxgi_high: i32 = 0;
        #[allow(clippy::cast_sign_loss)]
        let from_dxgi = gpu_id((dxgi_high as u32, 0xD1B3));
        let from_pdh = gpu_id(parse_pdh_luid(ENG_3D).expect("parse"));
        assert_eq!(from_dxgi, from_pdh);
        assert_eq!(from_dxgi, "luid:00000000-0000D1B3");
    }

    #[test]
    fn engine_aggregation_sums_per_type_and_takes_max() {
        let items = [
            (
                "pid_1_luid_0x00000000_0x0000D1B3_phys_0_eng_0_engtype_3D",
                20.0,
            ),
            (
                "pid_2_luid_0x00000000_0x0000D1B3_phys_0_eng_0_engtype_3D",
                15.0,
            ),
            (
                "pid_2_luid_0x00000000_0x0000D1B3_phys_0_eng_1_engtype_Copy",
                30.0,
            ),
            (
                "pid_3_luid_0x00000000_0x0000AAAA_phys_0_eng_0_engtype_3D",
                70.0,
            ),
            (
                "pid_3_luid_0x00000000_0x0000AAAA_phys_0_eng_0_engtype_3D",
                60.0,
            ),
            ("garbage", 99.0),
            (
                "pid_4_luid_0x00000000_0x0000D1B3_phys_0_eng_0_engtype_3D",
                f64::NAN,
            ),
        ];
        let agg = aggregate_engine_utilization(items);
        assert_eq!(agg.len(), 2);
        assert!((agg[&(0, 0xD1B3)] - 35.0).abs() < 1e-9);
        assert!((agg[&(0, 0xAAAA)] - 100.0).abs() < 1e-9, "clamped");
    }

    #[test]
    fn sums_memory_per_luid() {
        let items = [
            ("luid_0x00000000_0x0000D1B3_phys_0", 100.0),
            ("luid_0x00000000_0x0000D1B3_phys_1", 50.0),
            ("luid_0x00000000_0x00000001_phys_0", 7.0),
        ];
        let m = sum_per_luid(items);
        assert!((m[&(0, 0xD1B3)] - 150.0).abs() < 1e-9);
        assert!((m[&(0, 1)] - 7.0).abs() < 1e-9);
    }

    #[test]
    fn percent_clamping() {
        assert_eq!(clamp_percent(-3.0), Some(0.0));
        assert_eq!(clamp_percent(42.0), Some(42.0));
        assert_eq!(clamp_percent(130.0), Some(100.0));
        assert_eq!(clamp_percent(f64::NAN), None);
        assert_eq!(clamp_percent(f64::INFINITY), None);
        assert_eq!(active_from_idle(97.5), Some(2.5));
        assert_eq!(active_from_idle(104.0), Some(0.0));
    }

    #[test]
    fn cpu_share_normalizes_by_logical_count() {
        // 2 s of CPU over 1 s on 8 logical CPUs = 25 % of the machine.
        assert_eq!(cpu_share_percent(2_000, 1_000, 8), Some(25.0));
        assert_eq!(cpu_share_percent(0, 5_000, 16), Some(0.0));
        assert_eq!(cpu_share_percent(100_000, 1_000, 2), Some(100.0));
        assert_eq!(cpu_share_percent(10, 0, 8), None);
        assert_eq!(cpu_share_percent(10, 1_000, 0), None);
    }

    #[test]
    fn mhz_average_skips_zero() {
        assert_eq!(average_nonzero([3600, 3600, 4000, 4000]), Some(3800.0));
        assert_eq!(average_nonzero([0, 3000]), Some(3000.0));
        assert_eq!(average_nonzero([0, 0]), None);
        assert_eq!(average_nonzero(std::iter::empty()), None);
    }

    #[test]
    fn umd_version_format() {
        let v: i64 = (32 << 48) | (15 << 16) | 6094;
        assert_eq!(format_umd_version(v), "32.0.15.6094");
    }

    #[test]
    fn msft_disk_mappings() {
        assert_eq!(media_kind_from_msft(3), MediaKind::Hdd);
        assert_eq!(media_kind_from_msft(4), MediaKind::Ssd);
        assert_eq!(media_kind_from_msft(0), MediaKind::Unknown);
        assert_eq!(bus_kind_from_msft(17), BusKind::Nvme);
        assert_eq!(bus_kind_from_msft(11), BusKind::Sata);
        assert_eq!(bus_kind_from_msft(7), BusKind::Usb);
        assert_eq!(bus_kind_from_msft(15), BusKind::Virtual);
        assert_eq!(bus_kind_from_msft(3), BusKind::Other);
        assert_eq!(health_from_msft(0), DiskHealthStatus::Healthy);
        assert_eq!(health_from_msft(2), DiskHealthStatus::Unhealthy);
        assert_eq!(health_from_msft(5), DiskHealthStatus::Unknown);
        assert_eq!(memory_form_factor(8), Some("DIMM"));
        assert_eq!(memory_form_factor(12), Some("SODIMM"));
        assert_eq!(memory_form_factor(0), None);
        assert_eq!(non_empty(Some("  ".into())), None);
        assert_eq!(non_empty(Some(" A ".into())), Some("A".into()));
    }
}
