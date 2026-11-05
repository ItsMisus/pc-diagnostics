//! GPU adapter enumeration via DXGI. No COM apartment is required for DXGI factories.

use ::windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory1, DXGI_ADAPTER_FLAG_SOFTWARE, DXGI_ERROR_NOT_FOUND, IDXGIDevice,
    IDXGIFactory1,
};
use ::windows::core::Interface;
use models::metric::{Metric, Source, Unavailability, Unit};
use models::{AppError, ErrorCode};

use super::kmt::{self, KmtInfo};
use crate::convert::{Luid, format_umd_version};

#[derive(Debug, Clone)]
pub struct Adapter {
    pub luid: Luid,
    pub name: String,
    pub vendor_id: u32,
    pub device_id: u32,
    pub dedicated_vram_bytes: u64,
    pub shared_memory_bytes: u64,
    pub is_software: bool,
    pub driver_version: Metric<String>,
    /// PCI location and adapter-type flags from D3DKMT (best effort).
    pub kmt: KmtInfo,
}

fn hr_error(op: &str, e: &::windows::core::Error) -> AppError {
    AppError::new(ErrorCode::Platform, format!("{op} failed")).with_detail(format!(
        "HRESULT 0x{:08X}: {}",
        e.code().0,
        e.message()
    ))
}

/// Every adapter DXGI reports, in DXGI order (including software adapters, flagged).
pub fn enumerate_adapters() -> Result<Vec<Adapter>, AppError> {
    // SAFETY: plain factory creation; the returned interface is reference counted.
    let factory: IDXGIFactory1 =
        unsafe { CreateDXGIFactory1() }.map_err(|e| hr_error("CreateDXGIFactory1", &e))?;
    let mut out = Vec::new();
    for index in 0u32.. {
        // SAFETY: valid factory; DXGI_ERROR_NOT_FOUND terminates the enumeration.
        let adapter = match unsafe { factory.EnumAdapters1(index) } {
            Ok(a) => a,
            Err(e) if e.code() == DXGI_ERROR_NOT_FOUND => break,
            Err(e) => return Err(hr_error("IDXGIFactory1::EnumAdapters1", &e)),
        };
        // SAFETY: valid adapter interface.
        let desc = match unsafe { adapter.GetDesc1() } {
            Ok(d) => d,
            Err(e) => {
                tracing::warn!(index, error = %e, "IDXGIAdapter1::GetDesc1 failed; adapter skipped");
                continue;
            }
        };
        let name_len = desc
            .Description
            .iter()
            .position(|c| *c == 0)
            .unwrap_or(desc.Description.len());
        let name = String::from_utf16_lossy(&desc.Description[..name_len])
            .trim()
            .to_owned();
        // SAFETY: valid adapter; IDXGIDevice::IID is a static GUID.
        let driver_version = match unsafe { adapter.CheckInterfaceSupport(&IDXGIDevice::IID) } {
            Ok(v) => Metric::available(format_umd_version(v), Unit::None, Source::Dxgi),
            Err(e) => Metric::unavailable(Unavailability::Failed(hr_error(
                "IDXGIAdapter::CheckInterfaceSupport",
                &e,
            ))),
        };
        #[allow(clippy::cast_sign_loss)]
        let luid = (desc.AdapterLuid.HighPart as u32, desc.AdapterLuid.LowPart);
        let kmt = kmt::adapter_info(desc.AdapterLuid);
        #[allow(clippy::cast_sign_loss)]
        let is_software = desc.Flags & (DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32) != 0
            || kmt.is_software()
            // Microsoft Basic Render Driver (WARP) is reported with vendor 0x1414 / device 0x8C.
            || (desc.VendorId == 0x1414 && desc.DeviceId == 0x8C);
        tracing::debug!(?luid, name, ?kmt, is_software, "DXGI adapter");
        out.push(Adapter {
            luid,
            name,
            vendor_id: desc.VendorId,
            device_id: desc.DeviceId,
            dedicated_vram_bytes: desc.DedicatedVideoMemory as u64,
            shared_memory_bytes: desc.SharedSystemMemory as u64,
            is_software,
            driver_version,
            kmt,
        });
    }
    Ok(out)
}
