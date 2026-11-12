//! Adapter selection (pure). The parent names the GPU to test by its inventory identity
//! (`GpuInfo`: id + PCI ids); the worker picks the matching wgpu adapter. The id is the
//! DXGI LUID on Windows and the I/O Kit registry id on macOS (`MTLDevice.registryID`);
//! both are called "LUID" below.
//!
//! Rules, in order:
//! 1. Never a software adapter (wgpu `Cpu` type, WARP / Microsoft Basic Render Driver,
//!    PCI vendor 0x1414), a `VirtualGpu`, or any adapter whose LUID the parent flagged as
//!    software/virtual (e.g. a Parsec virtual display that copies the real GPU's ids).
//! 2. A DX12 or Metal adapter whose LUID equals the requested one ([`MatchKind::Luid`]).
//! 3. Otherwise exactly one eligible adapter per backend (DX12 first, then Vulkan) with
//!    the requested PCI vendor and device id and no conflicting LUID
//!    ([`MatchKind::PciIds`]). Two identical GPUs on a backend without LUIDs are
//!    ambiguous: no guess is made. Metal reports no PCI ids, so on macOS only rule 2
//!    applies: the registry id is exact and stable until reboot.

use models::settings::Language;

use super::protocol::{AdapterReport, MatchKind};

/// The GPU APIs the worker uses, as named in user-facing texts.
#[cfg(not(target_os = "macos"))]
pub(crate) const COMPUTE_API: &str = "DX12/Vulkan";
#[cfg(target_os = "macos")]
pub(crate) const COMPUTE_API: &str = "Metal";

/// Backends whose adapters carry an exact id (`AdapterReport::luid`).
const ID_BACKENDS: [&str; 2] = ["dx12", "metal"];

/// The GPU to test, as the parent identified it (DXGI).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Target {
    /// `luid:<high>-<low>` (`GpuInfo::id`).
    pub luid: Option<String>,
    pub vendor_id: u32,
    pub device_id: u32,
}

/// Why no adapter was selected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum NoMatch {
    /// `count` identical eligible adapters on `backend`: no guess is made.
    Ambiguous {
        count: usize,
        backend: &'static str,
        vendor_id: u32,
        device_id: u32,
    },
    /// No eligible physical adapter of the worker's backends ([`COMPUTE_API`]) has the
    /// requested identity.
    NoAdapter {
        vendor_id: u32,
        device_id: u32,
        luid: Option<String>,
    },
}

impl NoMatch {
    /// Plain-language reason (the parent shows it; the worker logs it in English).
    pub(crate) fn text(&self, lang: Language) -> String {
        match self {
            Self::Ambiguous {
                count,
                backend,
                vendor_id,
                device_id,
            } => lang.pick(
                format!(
                    "{count} adattatori {backend} identici (PCI {vendor_id:04X}:{device_id:04X}): \
                     impossibile distinguerli"
                ),
                format!(
                    "{count} identical {backend} adapters (PCI {vendor_id:04X}:{device_id:04X}): \
                     they can't be told apart"
                ),
            ),
            Self::NoAdapter {
                vendor_id,
                device_id,
                luid,
            } => {
                let luid = luid
                    .as_deref()
                    .map(|l| format!(", {l}"))
                    .unwrap_or_default();
                lang.pick(
                    format!(
                        "nessun adattatore {COMPUTE_API} fisico corrisponde alla GPU richiesta \
                         (PCI {vendor_id:04X}:{device_id:04X}{luid})"
                    ),
                    format!(
                        "no physical {COMPUTE_API} adapter matches the requested GPU \
                         (PCI {vendor_id:04X}:{device_id:04X}{luid})"
                    ),
                )
            }
        }
    }
}

const MICROSOFT_PCI_VENDOR: u32 = 0x1414;

pub(crate) fn is_eligible(a: &AdapterReport, excluded_luids: &[String]) -> bool {
    !matches!(a.device_type.as_str(), "cpu" | "virtual")
        && a.vendor_id != MICROSOFT_PCI_VENDOR
        && !a.name.contains("Microsoft Basic Render")
        && !a
            .luid
            .as_ref()
            .is_some_and(|l| excluded_luids.iter().any(|x| x.eq_ignore_ascii_case(l)))
}

pub(crate) fn select(
    adapters: &[AdapterReport],
    target: &Target,
    excluded_luids: &[String],
) -> Result<(usize, MatchKind), NoMatch> {
    let eligible = |a: &AdapterReport| is_eligible(a, excluded_luids);
    if let Some(luid) = &target.luid
        && let Some(i) = adapters.iter().position(|a| {
            ID_BACKENDS.contains(&a.backend.as_str())
                && eligible(a)
                && a.luid
                    .as_ref()
                    .is_some_and(|l| l.eq_ignore_ascii_case(luid))
        })
    {
        return Ok((i, MatchKind::Luid));
    }
    for backend in ["dx12", "vulkan"] {
        let hits: Vec<usize> = adapters
            .iter()
            .enumerate()
            .filter(|(_, a)| {
                a.backend == backend
                    && eligible(a)
                    && a.vendor_id == target.vendor_id
                    && a.device_id == target.device_id
                    // A DX12 adapter with a different LUID is a different device.
                    && (target.luid.is_none() || a.luid.is_none())
            })
            .map(|(i, _)| i)
            .collect();
        match hits.as_slice() {
            [i] => return Ok((*i, MatchKind::PciIds)),
            [] => {}
            _ => {
                return Err(NoMatch::Ambiguous {
                    count: hits.len(),
                    backend,
                    vendor_id: target.vendor_id,
                    device_id: target.device_id,
                });
            }
        }
    }
    Err(NoMatch::NoAdapter {
        vendor_id: target.vendor_id,
        device_id: target.device_id,
        luid: target.luid.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a(
        name: &str,
        backend: &str,
        ty: &str,
        vendor: u32,
        device: u32,
        luid: Option<&str>,
    ) -> AdapterReport {
        AdapterReport {
            name: name.into(),
            vendor_id: vendor,
            device_id: device,
            backend: backend.into(),
            device_type: ty.into(),
            luid: luid.map(Into::into),
            driver: String::new(),
            driver_info: String::new(),
        }
    }

    const RTX: (u32, u32) = (0x10DE, 0x2783);
    const RTX_LUID: &str = "luid:00000000-0000D1B3";
    const PARSEC_LUID: &str = "luid:00000000-0000E000";

    /// The test machine: RTX (DX12 + Vulkan), AMD iGPU, a Parsec virtual adapter that
    /// copies the RTX ids, WARP.
    fn machine() -> Vec<AdapterReport> {
        vec![
            a(
                "Parsec Virtual Display Adapter",
                "dx12",
                "discrete",
                RTX.0,
                RTX.1,
                Some(PARSEC_LUID),
            ),
            a(
                "Microsoft Basic Render Driver",
                "dx12",
                "cpu",
                0x1414,
                0x8C,
                Some("luid:00000000-00000100"),
            ),
            a(
                "AMD Radeon(TM) Graphics",
                "dx12",
                "integrated",
                0x1002,
                0x164E,
                Some("luid:00000000-0000A000"),
            ),
            a(
                "NVIDIA GeForce RTX 4070 SUPER",
                "dx12",
                "discrete",
                RTX.0,
                RTX.1,
                Some(RTX_LUID),
            ),
            a(
                "NVIDIA GeForce RTX 4070 SUPER",
                "vulkan",
                "discrete",
                RTX.0,
                RTX.1,
                None,
            ),
            a("llvmpipe", "vulkan", "cpu", 0x10005, 0, None),
        ]
    }

    fn target(luid: Option<&str>) -> Target {
        Target {
            luid: luid.map(Into::into),
            vendor_id: RTX.0,
            device_id: RTX.1,
        }
    }

    #[test]
    fn exact_luid_wins_and_virtual_is_skipped() {
        let m = machine();
        let excl = vec![PARSEC_LUID.to_owned(), "luid:00000000-00000100".to_owned()];
        let (i, how) = select(&m, &target(Some(RTX_LUID)), &excl).expect("select");
        assert_eq!(how, MatchKind::Luid);
        assert_eq!(m[i].luid.as_deref(), Some(RTX_LUID));
        // LUID comparison ignores hex case.
        let (j, _) = select(&m, &target(Some(&RTX_LUID.to_lowercase())), &excl).expect("select");
        assert_eq!(i, j);
    }

    #[test]
    fn excluded_and_software_adapters_are_never_chosen() {
        let m = machine();
        let excl = vec![PARSEC_LUID.to_owned()];
        assert!(!is_eligible(&m[0], &excl), "virtual display");
        assert!(!is_eligible(&m[1], &excl), "WARP");
        assert!(!is_eligible(&m[5], &excl), "cpu");
        assert!(is_eligible(&m[2], &excl) && is_eligible(&m[3], &excl));
        // Even when the parent asks for the virtual adapter's LUID it is not selected;
        // the PCI fallback cannot pick a DX12 adapter with another LUID, so Vulkan is used.
        let (i, how) = select(&m, &target(Some(PARSEC_LUID)), &excl).expect("select");
        assert_eq!((m[i].backend.as_str(), how), ("vulkan", MatchKind::PciIds));
        // Only software/virtual adapters → no selection.
        let only_bad = vec![m[0].clone(), m[1].clone(), m[5].clone()];
        assert!(select(&only_bad, &target(None), &excl).is_err());
        let warp = Target {
            luid: None,
            vendor_id: 0x1414,
            device_id: 0x8C,
        };
        assert!(select(&m, &warp, &excl).is_err());
    }

    #[test]
    fn pci_fallback_and_ambiguity() {
        let m = machine();
        let excl = vec![PARSEC_LUID.to_owned()];
        // LUID gone stale (e.g. after a driver reset): Vulkan by PCI ids.
        let (i, how) = select(&m, &target(Some("luid:00000000-DEADBEEF")), &excl).expect("select");
        assert_eq!((m[i].backend.as_str(), how), ("vulkan", MatchKind::PciIds));
        // No LUID known: the single eligible DX12 adapter with those ids.
        let (i, how) = select(&m, &target(None), &excl).expect("select");
        assert_eq!(
            (m[i].luid.as_deref(), how),
            (Some(RTX_LUID), MatchKind::PciIds)
        );
        // Two identical GPUs without LUIDs are ambiguous.
        let twins = vec![m[4].clone(), m[4].clone()];
        let e = select(&twins, &target(None), &[]).expect_err("ambiguous");
        assert!(e.text(Language::Italian).contains("identici"));
        assert!(e.text(Language::English).contains("can't be told apart"));
        // Unknown GPU.
        let other = Target {
            luid: None,
            vendor_id: 0x8086,
            device_id: 0x1,
        };
        let e = select(&m, &other, &excl).expect_err("unknown");
        let api = if cfg!(target_os = "macos") {
            "Metal"
        } else {
            "DX12/Vulkan"
        };
        assert_eq!(
            e.text(Language::English),
            format!("no physical {api} adapter matches the requested GPU (PCI 8086:0001)")
        );
        assert_eq!(
            e.text(Language::Italian),
            format!(
                "nessun adattatore {api} fisico corrisponde alla GPU richiesta (PCI 8086:0001)"
            )
        );
    }

    /// An Apple Silicon Mac as the worker sees it (Metal: no PCI ids) and as the inventory
    /// lists it (vendor 0x106B, no device id, registry id).
    #[test]
    fn metal_adapter_is_selected_by_registry_id() {
        let id = "ioreg:00000001000004A1";
        let m = vec![a("Apple M5", "metal", "integrated", 0, 0, Some(id))];
        let want = Target {
            luid: Some(id.into()),
            vendor_id: 0x106B,
            device_id: 0,
        };
        assert_eq!(select(&m, &want, &[]), Ok((0, MatchKind::Luid)));
        // Hex case is irrelevant, as for LUIDs.
        let lower = Target {
            luid: Some(id.to_lowercase()),
            ..want.clone()
        };
        assert_eq!(select(&m, &lower, &[]), Ok((0, MatchKind::Luid)));
        // Another registry id (e.g. a different GPU) never falls back to a guess.
        let other = Target {
            luid: Some("ioreg:00000001000004B2".into()),
            ..want.clone()
        };
        assert!(matches!(
            select(&m, &other, &[]),
            Err(NoMatch::NoAdapter { .. })
        ));
        // Excluded by id.
        assert!(select(&m, &want, &[id.to_owned()]).is_err());
    }
}
