//! macOS GPU list for the GPU benchmark: every `IOAccelerator` service in the I/O Kit
//! registry (on Apple Silicon one `AGXAccelerator*` per SoC GPU; on Intel Macs one per
//! integrated/discrete GPU).
//!
//! Id scheme (`GpuInfo::id` on macOS): `ioreg:<16 upper-case hex digits>` of the
//! accelerator's registry entry id ([`crate::gpu_bench::registry_gpu_id`]). It is the value
//! Metal reports as `MTLDevice.registryID`, so the benchmark worker can match its Metal
//! adapter to this entry exactly; it is stable until reboot, like a Windows LUID.

use models::hardware::{GpuInfo, GpuVendor};
use models::metric::{Metric, Unavailability};

use super::bench_iokit as iokit;
use crate::gpu_bench::registry_gpu_id;

pub fn accelerators() -> Vec<GpuInfo> {
    iokit::services_of_class(c"IOAccelerator")
        .into_iter()
        .filter_map(|acc| {
            let id = acc.registry_id()?;
            // Apple Silicon: properties on the accelerator; PCI GPUs (Intel Macs): the
            // `IOPCIDevice` parent carries vendor-id/device-id/model.
            let parent = acc.parents().into_iter().next();
            let pci = parent.as_ref().filter(|p| p.conforms_to(c"IOPCIDevice"));
            let read_u32 = |key: &str| {
                acc.data_u32(key)
                    .or_else(|| pci.and_then(|p| p.data_u32(key)))
            };
            let name = acc
                .string("model")
                .or_else(|| pci.and_then(|p| p.data_string("model")))?;
            let vendor_id = read_u32("vendor-id").unwrap_or(0);
            Some(GpuInfo {
                id: registry_gpu_id(id),
                name,
                // Apple (0x106B) has no own `GpuVendor` variant: `Other`.
                vendor: GpuVendor::from_pci_id(vendor_id),
                vendor_id,
                // Apple Silicon GPUs are not PCI devices and have no device id; the
                // benchmark matches by registry id, never by this field.
                device_id: read_u32("device-id").unwrap_or(0),
                // Unified memory: there is no dedicated VRAM, and the shared budget is not
                // a fixed size. The model has no "unavailable" for these two `u64` fields
                // (integrator request filed); the benchmark never reads them, it uses the
                // id, name and vendor only.
                dedicated_vram_bytes: 0,
                shared_memory_bytes: 0,
                is_software: false,
                is_virtual: false,
                driver_version: Metric::unavailable(Unavailability::NotSupported),
                telemetry_sources: Vec::new(),
            })
        })
        .collect()
}
