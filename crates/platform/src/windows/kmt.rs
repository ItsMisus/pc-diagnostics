//! Kernel-mode thunk queries (user-mode `gdi32` D3DKMT API; no elevation needed): PCI
//! location and adapter type flags for a DXGI LUID. Used to match NVML devices to DXGI
//! adapters by PCI bus location and to recognize indirect-display (virtual) adapters.

use ::windows::Wdk::Graphics::Direct3D::{
    D3DKMT_ADAPTER_PERFDATA, D3DKMT_ADAPTER_PERFDATACAPS, D3DKMT_ADAPTERADDRESS,
    D3DKMT_ADAPTERTYPE, D3DKMT_CLOSEADAPTER, D3DKMT_NODE_PERFDATA, D3DKMT_OPENADAPTERFROMLUID,
    D3DKMT_QUERYADAPTERINFO, D3DKMTCloseAdapter, D3DKMTOpenAdapterFromLuid, D3DKMTQueryAdapterInfo,
    KMTQAITYPE_ADAPTERADDRESS, KMTQAITYPE_ADAPTERPERFDATA, KMTQAITYPE_ADAPTERPERFDATA_CAPS,
    KMTQAITYPE_ADAPTERTYPE, KMTQAITYPE_NODEPERFDATA, KMTQUERYADAPTERINFOTYPE,
};
use ::windows::Win32::Foundation::LUID;
use models::{AppError, ErrorCode};

use crate::convert::{self, Luid};

/// `D3DKMT_ADAPTERTYPE` bit: the adapter is an indirect display (IddCx) device, e.g. a
/// virtual display driver from a remote-desktop tool.
const INDIRECT_DISPLAY_DEVICE_BIT: u32 = 1 << 6;
/// `D3DKMT_ADAPTERTYPE` bit: software device (WARP / Basic Render Driver).
const SOFTWARE_DEVICE_BIT: u32 = 1 << 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PciLocation {
    pub bus: u32,
    pub device: u32,
    pub function: u32,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct KmtInfo {
    pub pci: Option<PciLocation>,
    pub type_flags: Option<u32>,
}

impl KmtInfo {
    pub fn is_indirect_display(&self) -> bool {
        self.type_flags
            .is_some_and(|f| f & INDIRECT_DISPLAY_DEVICE_BIT != 0)
    }

    pub fn is_software(&self) -> bool {
        self.type_flags
            .is_some_and(|f| f & SOFTWARE_DEVICE_BIT != 0)
    }
}

struct Adapter(u32);

impl Drop for Adapter {
    fn drop(&mut self) {
        let close = D3DKMT_CLOSEADAPTER { hAdapter: self.0 };
        // SAFETY: handle from D3DKMTOpenAdapterFromLuid, closed exactly once.
        let _ = unsafe { D3DKMTCloseAdapter(&close) };
    }
}

fn query<T: Default>(adapter: &Adapter, kind: KMTQUERYADAPTERINFOTYPE) -> Option<T> {
    query_with(adapter, kind, T::default())
}

/// Like `query`, for in/out structs whose input fields must be set first.
fn query_with<T>(adapter: &Adapter, kind: KMTQUERYADAPTERINFOTYPE, mut out: T) -> Option<T> {
    let mut q = D3DKMT_QUERYADAPTERINFO {
        hAdapter: adapter.0,
        Type: kind,
        pPrivateDriverData: (&raw mut out).cast(),
        PrivateDriverDataSize: u32::try_from(std::mem::size_of::<T>()).ok()?,
    };
    // SAFETY: `out` is a properly sized, writable buffer of the type `kind` expects.
    let status = unsafe { D3DKMTQueryAdapterInfo(&mut q) };
    (status.0 == 0).then_some(out)
}

/// Best effort: any failure leaves the corresponding field `None`.
pub fn adapter_info(luid: LUID) -> KmtInfo {
    let mut open = D3DKMT_OPENADAPTERFROMLUID {
        AdapterLuid: luid,
        hAdapter: 0,
    };
    // SAFETY: `open` is a valid in/out struct.
    let status = unsafe { D3DKMTOpenAdapterFromLuid(&mut open) };
    if status.0 != 0 {
        return KmtInfo::default();
    }
    let adapter = Adapter(open.hAdapter);
    // Adapters without a PCI function (WARP, indirect display) report all-ones.
    let pci = query::<D3DKMT_ADAPTERADDRESS>(&adapter, KMTQAITYPE_ADAPTERADDRESS)
        .filter(|a| a.BusNumber != u32::MAX && a.DeviceNumber != 0xFFFF)
        .map(|a| PciLocation {
            bus: a.BusNumber,
            device: a.DeviceNumber,
            function: a.FunctionNumber,
        });
    let type_flags = query::<D3DKMT_ADAPTERTYPE>(&adapter, KMTQAITYPE_ADAPTERTYPE)
        // SAFETY: plain-data union; the bitfield view is always valid to read.
        .map(|t| unsafe { t.Anonymous.Value });
    KmtInfo { pci, type_flags }
}

/// An adapter handle kept open for repeated WDDM performance-data reads (live sampler,
/// GPU benchmark). Reads cost a few microseconds and need no elevation.
pub struct KmtAdapter(Adapter);

/// One WDDM performance-data reading, raw driver units already converted.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct KmtPerf {
    pub temperature_c: Option<f64>,
    pub core_clock_mhz: Option<f64>,
    pub power_percent: Option<f64>,
    pub fan_rpm: Option<f64>,
}

/// Static WDDM performance capabilities of the adapter.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct KmtCaps {
    /// Driver-reported warning temperature (`TemperatureWarning`).
    pub temperature_warning_c: Option<f64>,
    /// Driver-reported maximum temperature (`TemperatureMax`).
    pub temperature_max_c: Option<f64>,
    /// `MaxFanRPM > 0`: the adapter has a fan the driver reports on.
    pub has_fan: bool,
}

fn kmt_error(op: &str, status: i32) -> AppError {
    AppError::new(ErrorCode::Platform, format!("{op} failed"))
        .with_detail(format!("NTSTATUS 0x{status:08X}"))
}

impl KmtAdapter {
    pub fn open(luid: Luid) -> Result<Self, AppError> {
        let mut open = D3DKMT_OPENADAPTERFROMLUID {
            AdapterLuid: to_luid(luid),
            hAdapter: 0,
        };
        // SAFETY: `open` is a valid in/out struct.
        let status = unsafe { D3DKMTOpenAdapterFromLuid(&mut open) };
        if status.0 != 0 {
            return Err(kmt_error("D3DKMTOpenAdapterFromLuid", status.0));
        }
        Ok(Self(Adapter(open.hAdapter)))
    }

    pub fn caps(&self) -> KmtCaps {
        query::<D3DKMT_ADAPTER_PERFDATACAPS>(&self.0, KMTQAITYPE_ADAPTERPERFDATA_CAPS)
            .map(|c| KmtCaps {
                temperature_warning_c: convert::kmt_temperature_c(c.TemperatureWarning),
                temperature_max_c: convert::kmt_temperature_c(c.TemperatureMax),
                has_fan: c.MaxFanRPM > 0,
            })
            .unwrap_or_default()
    }

    /// Adapter-level perf data plus node 0 (the 3D engine node) frequency.
    pub fn perf(&self, caps: &KmtCaps) -> Result<KmtPerf, AppError> {
        let a = query::<D3DKMT_ADAPTER_PERFDATA>(&self.0, KMTQAITYPE_ADAPTERPERFDATA).ok_or_else(
            || {
                AppError::new(
                    ErrorCode::Platform,
                    "D3DKMTQueryAdapterInfo(ADAPTERPERFDATA) failed",
                )
                .with_detail("the driver does not report WDDM performance data")
            },
        )?;
        let node = query_with(
            &self.0,
            KMTQAITYPE_NODEPERFDATA,
            D3DKMT_NODE_PERFDATA::default(),
        );
        Ok(KmtPerf {
            temperature_c: convert::kmt_temperature_c(a.Temperature),
            core_clock_mhz: node.and_then(|n| convert::kmt_clock_mhz(n.Frequency, n.MaxFrequency)),
            power_percent: convert::kmt_power_percent(a.Power),
            fan_rpm: caps.has_fan.then_some(f64::from(a.FanRPM)),
        })
    }
}

fn to_luid(luid: Luid) -> LUID {
    #[allow(clippy::cast_possible_wrap)]
    LUID {
        LowPart: luid.1,
        HighPart: luid.0 as i32,
    }
}

#[cfg(test)]
mod probe {
    #[test]
    #[ignore = "hardware probe; prints converted WDDM perf data for every adapter"]
    fn print_perf_data() {
        for a in super::super::dxgi::enumerate_adapters().expect("dxgi") {
            match super::KmtAdapter::open(a.luid) {
                Ok(k) => {
                    let caps = k.caps();
                    println!("{}: {caps:?} {:?}", a.name, k.perf(&caps));
                }
                Err(e) => println!("{}: {e:?}", a.name),
            }
        }
    }
}
