//! Physical disks from the I/O Kit storage stack, no privileges needed.
//!
//! Each physical disk is an `IOBlockStorageDriver` whose provider is the block device
//! (`IOEmbeddedNVMeBlockDevice`, an AHCI/SCSI/USB device, …) and whose first client is
//! the whole-disk `IOMedia`. Disk images (provider `Physical Interconnect` =
//! `Virtual Interface`) are listed in the inventory with bus `Virtual`, but excluded from
//! the live throughput: their I/O also reaches the disk that holds the image, so adding
//! both would count it twice.
//!
//! Live throughput: the driver's `Statistics` counters `Bytes (Read)` / `Bytes (Write)`
//! (cumulative since the disk attached), summed over physical disks as a per-disk delta
//! over the elapsed time. Disks that appear or disappear between two readings contribute
//! nothing for that window.
//!
//! Active time is not reported: the driver's `Total Time (Read/Write)` is the sum of
//! per-request service times, so with several requests in flight (NVMe queues) it grows
//! faster than wall time and is not a busy share like Windows' `100 - % Idle Time`.

use std::collections::HashMap;
use std::time::Instant;

use models::hardware::{BusKind, DiskHealthStatus, MediaKind, PhysicalDiskInfo};
use models::metric::{Metric, Source, Unavailability, Unit};
use models::monitor::StorageSample;
use models::{AppError, ErrorCode};

use super::iokit::{self, IoObject};
use super::mem::counter_rate;
use super::nvme_smart;

/// Re-enumerate the drivers every this many samples to pick up newly attached disks.
const RESCAN_EVERY: u32 = 30;

/// `Protocol Characteristics` → `Physical Interconnect` of a block device. NVMe drives
/// are recognised by their driver class (Apple's internal NVMe reports `Apple Fabric`).
pub fn bus_kind(interconnect: Option<&str>, device_class: &str) -> BusKind {
    let i = interconnect.unwrap_or_default();
    if i.eq_ignore_ascii_case("Virtual Interface") {
        return BusKind::Virtual;
    }
    if device_class.contains("NVMe") {
        return BusKind::Nvme;
    }
    match i {
        "SATA" => BusKind::Sata,
        "USB" => BusKind::Usb,
        "SAS" => BusKind::Sas,
        _ => BusKind::Other,
    }
}

/// `Device Characteristics` → `Medium Type`.
pub fn media_kind(medium_type: Option<&str>) -> MediaKind {
    match medium_type {
        Some("Solid State") => MediaKind::Ssd,
        Some("Rotational") => MediaKind::Hdd,
        _ => MediaKind::Unknown,
    }
}

fn interconnect(device: &IoObject) -> Option<String> {
    device
        .dictionary("Protocol Characteristics")?
        .string("Physical Interconnect")
}

fn is_virtual(driver: &IoObject) -> bool {
    driver
        .parent()
        .and_then(|d| interconnect(&d))
        .is_some_and(|i| i.eq_ignore_ascii_case("Virtual Interface"))
}

fn non_empty(s: Option<String>) -> Option<String> {
    crate::convert::non_empty(s)
}

pub fn physical_disks() -> Result<Vec<PhysicalDiskInfo>, AppError> {
    let drivers = iokit::services_of_class(c"IOBlockStorageDriver")?;
    let mut out = Vec::new();
    for driver in drivers {
        let Some(device) = driver.parent() else {
            continue;
        };
        let Some(media) = driver.first_child().filter(|m| m.conforms_to(c"IOMedia")) else {
            continue; // no medium inserted (e.g. an empty card reader)
        };
        let Some(bsd) = non_empty(media.string("BSD Name")) else {
            continue;
        };
        let chars = device.dictionary("Device Characteristics");
        let field = |k: &str| non_empty(chars.as_ref().and_then(|c| c.string(k)));
        let model = match (field("Vendor Name"), field("Product Name")) {
            (Some(v), Some(p)) if !p.starts_with(&v) => Some(format!("{v} {p}")),
            (_, Some(p)) => Some(p),
            (v, None) => v,
        };
        let class = device.class_name().unwrap_or_default();
        out.push(PhysicalDiskInfo {
            id: bsd,
            model,
            firmware: field("Product Revision Level"),
            media: media_kind(field("Medium Type").as_deref()),
            bus: bus_kind(interconnect(&device).as_deref(), &class),
            size_bytes: media
                .number("Size")
                .and_then(|v| u64::try_from(v).ok())
                .unwrap_or(0),
            health: disk_health(&device),
        });
    }
    out.sort_by(|a, b| natural_disk_order(&a.id, &b.id));
    Ok(out)
}

/// NVMe drives: the SMART / Health Information log (see `nvme_smart`). Other drives
/// (SATA, USB): ATA SMART is not read, so their health stays not implemented.
fn disk_health(device: &IoObject) -> Metric<DiskHealthStatus> {
    if !nvme_smart::is_capable(device) {
        return Metric::unavailable(Unavailability::NotImplemented);
    }
    match nvme_smart::read(device) {
        Ok(log) => Metric::available(log.health(), Unit::None, Source::IoKit),
        Err(e) => Metric::unavailable(Unavailability::Failed(e)),
    }
}

/// `disk2` before `disk10`.
fn natural_disk_order(a: &str, b: &str) -> std::cmp::Ordering {
    let n = |s: &str| s.trim_start_matches("disk").parse::<u32>().ok();
    n(a).cmp(&n(b)).then_with(|| a.cmp(b))
}

struct Driver {
    entry_id: u64,
    service: IoObject,
}

/// `(bytes read, bytes written)` of one driver.
fn byte_counters(d: &IoObject) -> Option<(u64, u64)> {
    let s = d.dictionary("Statistics")?;
    let r = u64::try_from(s.number("Bytes (Read)")?).ok()?;
    let w = u64::try_from(s.number("Bytes (Write)")?).ok()?;
    Some((r, w))
}

/// Per-tick disk throughput over a cached driver list.
pub struct StorageLive {
    drivers: Result<Vec<Driver>, AppError>,
    prev: HashMap<u64, (u64, u64)>,
    prev_at: Option<Instant>,
    since_scan: u32,
}

fn scan() -> Result<Vec<Driver>, AppError> {
    Ok(iokit::services_of_class(c"IOBlockStorageDriver")?
        .into_iter()
        .filter(|d| !is_virtual(d))
        .filter_map(|service| {
            Some(Driver {
                entry_id: service.entry_id()?,
                service,
            })
        })
        .collect())
}

impl StorageLive {
    /// Enumerates the disks and takes the baseline reading.
    pub fn new() -> Self {
        let mut s = Self {
            drivers: scan(),
            prev: HashMap::new(),
            prev_at: None,
            since_scan: 0,
        };
        let _ = s.sample();
        s
    }

    pub fn sample(&mut self) -> StorageSample {
        self.since_scan += 1;
        if self.since_scan >= RESCAN_EVERY || self.drivers.is_err() {
            self.drivers = scan();
            self.since_scan = 0;
        }
        let now = Instant::now();
        let drivers = match &self.drivers {
            Ok(d) => d,
            Err(e) => return all_unavailable(&Unavailability::Failed(e.clone())),
        };
        let mut current = HashMap::with_capacity(drivers.len());
        let mut lost = false;
        for d in drivers {
            match byte_counters(&d.service) {
                Some(c) => {
                    current.insert(d.entry_id, c);
                }
                None => lost = true, // detached since the scan
            }
        }
        if lost {
            self.since_scan = RESCAN_EVERY; // rescan on the next tick
        }
        let prev = std::mem::replace(&mut self.prev, current);
        let prev_at = self.prev_at.replace(now);
        if self.prev.is_empty() {
            return all_unavailable(&Unavailability::Failed(AppError::new(
                ErrorCode::NotFound,
                "no block storage statistics found",
            )));
        }
        let Some(prev_at) = prev_at else {
            let mut e = AppError::new(
                ErrorCode::NotFound,
                "No completed measurement window yet (needs two readings)",
            );
            e.retriable = true;
            return all_unavailable(&Unavailability::Failed(e));
        };
        let elapsed = now.duration_since(prev_at).as_secs_f64();
        let (mut read, mut write, mut paired) = (0.0, 0.0, 0usize);
        for (id, (r, w)) in &self.prev {
            let Some((pr, pw)) = prev.get(id) else {
                continue; // new disk: no baseline yet
            };
            // A counter going backwards means the disk was replaced under the same id.
            if let (Some(dr), Some(dw)) = (
                counter_rate(*pr, *r, elapsed),
                counter_rate(*pw, *w, elapsed),
            ) {
                read += dr;
                write += dw;
                paired += 1;
            }
        }
        if paired == 0 {
            let mut e = AppError::new(
                ErrorCode::NotFound,
                "disk list changed; no comparable readings in this window",
            );
            e.retriable = true;
            return all_unavailable(&Unavailability::Failed(e));
        }
        StorageSample {
            read_bytes_per_sec: Metric::available(read, Unit::BytesPerSecond, Source::IoKit),
            write_bytes_per_sec: Metric::available(write, Unit::BytesPerSecond, Source::IoKit),
            active_time_percent: Metric::unavailable(Unavailability::NotSupported),
        }
    }

    /// Cumulative `(read, write)` byte totals over the physical disks at the last sample.
    #[cfg(test)]
    fn totals(&self) -> (u64, u64) {
        self.prev
            .values()
            .fold((0, 0), |(r, w), (dr, dw)| (r + dr, w + dw))
    }
}

fn all_unavailable(r: &Unavailability) -> StorageSample {
    StorageSample {
        read_bytes_per_sec: Metric::unavailable(r.clone()),
        write_bytes_per_sec: Metric::unavailable(r.clone()),
        active_time_percent: Metric::unavailable(Unavailability::NotSupported),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bus_and_media_mapping() {
        assert_eq!(
            bus_kind(Some("Apple Fabric"), "IOEmbeddedNVMeBlockDevice"),
            BusKind::Nvme
        );
        assert_eq!(
            bus_kind(Some("PCI-Express"), "IONVMeBlockStorageDevice"),
            BusKind::Nvme
        );
        assert_eq!(
            bus_kind(Some("SATA"), "IOAHCIBlockStorageDevice"),
            BusKind::Sata
        );
        assert_eq!(
            bus_kind(Some("USB"), "IOSCSIPeripheralDeviceType00"),
            BusKind::Usb
        );
        assert_eq!(
            bus_kind(Some("Virtual Interface"), "IOHDIXHDDriveOutKernel"),
            BusKind::Virtual
        );
        assert_eq!(bus_kind(None, "X"), BusKind::Other);
        assert_eq!(media_kind(Some("Solid State")), MediaKind::Ssd);
        assert_eq!(media_kind(Some("Rotational")), MediaKind::Hdd);
        assert_eq!(media_kind(None), MediaKind::Unknown);
        assert_eq!(
            natural_disk_order("disk2", "disk10"),
            std::cmp::Ordering::Less
        );
    }

    #[test]
    fn host_physical_disks_include_the_boot_disk() {
        let disks = physical_disks().expect("disk inventory");
        let d0 = disks.iter().find(|d| d.id == "disk0").expect("disk0");
        assert!(d0.size_bytes > 10_000_000_000);
        assert!(d0.model.is_some());
        if cfg!(target_arch = "aarch64") {
            assert_eq!(d0.bus, BusKind::Nvme);
            assert_eq!(d0.media, MediaKind::Ssd);
        }
    }

    #[test]
    fn host_storage_counters_do_not_decrease() {
        let mut s = StorageLive::new();
        let t0 = s.totals();
        // Generate some real I/O so the window has something to measure.
        let path = std::env::temp_dir().join(format!("pcd-storage-{}", std::process::id()));
        std::fs::write(&path, vec![7u8; 4 << 20]).expect("write temp file");
        let _ = std::fs::File::open(&path).and_then(|f| f.sync_all());
        std::thread::sleep(std::time::Duration::from_millis(500));
        let x = s.sample();
        let _ = std::fs::remove_file(&path);
        let t1 = s.totals();
        assert!(t1.0 >= t0.0 && t1.1 >= t0.1, "{t0:?} -> {t1:?}");
        let r = *x.read_bytes_per_sec.value().expect("read rate");
        let w = *x.write_bytes_per_sec.value().expect("write rate");
        assert!(r >= 0.0 && w >= 0.0);
        assert!(!x.active_time_percent.is_available());
    }
}
