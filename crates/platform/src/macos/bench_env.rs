//! macOS implementation of `crate::bench_env`. Read-only queries only (`statfs`,
//! `getfsstat`, I/O Kit registry, IOPowerSources, `NSProcessInfo`), plus the two file
//! controls the storage benchmark needs on its own temporary file (`F_NOCACHE`,
//! `F_FULLFSYNC`).
//!
//! Volumes are identified by their mount point. The system volume is `/`: since macOS
//! 10.15 it is the sealed, read-only system snapshot of a volume group whose writable half
//! (user data, `$TMPDIR`) is mounted at `/System/Volumes/Data` in the same APFS container.
//! Both report the container's free space, so `/` stands for the pair and the storage
//! benchmark writes its file on the data volume (see [`volume_of`]).

use std::ffi::{CStr, CString};
use std::fs::File;
use std::os::fd::AsRawFd;
use std::os::unix::ffi::OsStringExt;
use std::path::PathBuf;

use models::benchmark_run::PowerSource;
use models::{AppError, ErrorCode};
use objc2_core_foundation::CFString;
use objc2_io_kit::{IOPSCopyPowerSourcesInfo, IOPSGetProvidingPowerSourceType};

use super::bench_iokit as iokit;
use crate::bench_env::VolumeSpace;

/// Mount point of the data volume of the booted system volume group.
pub(crate) const DATA_VOLUME: &str = "/System/Volumes/Data";
/// `MNAMELEN` (longest mount path).
const MAX_MOUNT_PATH: usize = 1024;

/// One mounted file system (from `getfsstat`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Mount {
    pub on: String,
    pub from: String,
    pub fs_type: String,
    pub flags: u32,
}

fn c_chars(buf: &[libc::c_char]) -> String {
    let bytes: Vec<u8> = buf
        .iter()
        .take_while(|c| **c != 0)
        .map(|c| *c as u8)
        .collect();
    String::from_utf8_lossy(&bytes).into_owned()
}

fn mount_of(s: &libc::statfs) -> Mount {
    Mount {
        on: c_chars(&s.f_mntonname),
        from: c_chars(&s.f_mntfromname),
        fs_type: c_chars(&s.f_fstypename),
        flags: s.f_flags,
    }
}

fn statfs(path: &str) -> Result<libc::statfs, AppError> {
    let c = CString::new(path).map_err(|_| AppError::invalid_input("path contains NUL"))?;
    // SAFETY: `statfs` is a plain C struct; all-zero is a valid value.
    let mut st: libc::statfs = unsafe { std::mem::zeroed() };
    // SAFETY: `c` is NUL-terminated and `st` is a valid, writable statfs.
    if unsafe { libc::statfs(c.as_ptr(), &mut st) } != 0 {
        let e = std::io::Error::last_os_error();
        let code = if e.kind() == std::io::ErrorKind::PermissionDenied {
            ErrorCode::PermissionDenied
        } else {
            ErrorCode::Platform
        };
        return Err(AppError::new(code, "statfs failed").with_detail(format!("{path}: {e}")));
    }
    Ok(st)
}

/// Every mounted file system (`getfsstat(MNT_NOWAIT)`: no blocking on network mounts).
pub(crate) fn mounts() -> Vec<Mount> {
    // SAFETY: a NULL buffer only returns the count.
    let n = unsafe { libc::getfsstat(std::ptr::null_mut(), 0, libc::MNT_NOWAIT) };
    let Ok(n) = usize::try_from(n) else {
        return Vec::new();
    };
    // Room for mounts added between the two calls.
    let cap = n + 8;
    // SAFETY: `statfs` is a plain C struct; all-zero is a valid value.
    let mut buf: Vec<libc::statfs> = (0..cap).map(|_| unsafe { std::mem::zeroed() }).collect();
    let Ok(bytes) = libc::c_int::try_from(cap * std::mem::size_of::<libc::statfs>()) else {
        return Vec::new();
    };
    // SAFETY: `buf` holds `cap` writable statfs records, `bytes` long.
    let got = unsafe { libc::getfsstat(buf.as_mut_ptr(), bytes, libc::MNT_NOWAIT) };
    let Ok(got) = usize::try_from(got) else {
        return Vec::new();
    };
    buf.iter().take(got.min(cap)).map(mount_of).collect()
}

/// `/dev/disk3s5` → `disk3s5`.
fn bsd_name(from: &str) -> Option<&str> {
    from.strip_prefix("/dev/")
        .filter(|n| n.starts_with("disk") && n.bytes().all(|b| b.is_ascii_alphanumeric()))
}

/// `/dev/disk3s5` → `disk3` (the whole disk: for APFS, the container).
fn whole_disk(from: &str) -> Option<&str> {
    let name = bsd_name(from)?;
    let digits = name["disk".len()..]
        .bytes()
        .take_while(u8::is_ascii_digit)
        .count();
    (digits > 0).then(|| &name[..4 + digits])
}

/// Pure part of the volume filter: a local, browsable, block-device-backed file system.
/// `/` is kept although it is read-only (it is the sealed system snapshot; see the module
/// docs); any other read-only mount is skipped (nothing can be written there). Hidden
/// (`nobrowse`) mounts are skipped: the system's own APFS role volumes (Preboot, VM,
/// Update, Recovery, xART, Hardware) and the data volume, which `/` already stands for.
pub(crate) fn is_candidate(m: &Mount) -> bool {
    let flags = i64::from(m.flags);
    let local = flags & i64::from(libc::MNT_LOCAL) != 0;
    let hidden = flags & i64::from(libc::MNT_DONTBROWSE) != 0;
    let read_only = flags & i64::from(libc::MNT_RDONLY) != 0;
    local
        && !hidden
        && bsd_name(&m.from).is_some()
        && (m.on == "/" || (!read_only && m.on.starts_with('/')))
}

/// How the device behind a mount is attached, from I/O Kit.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct Attachment {
    /// `IOMedia` "Removable" (SD cards, USB sticks): Windows `DRIVE_REMOVABLE`.
    removable: bool,
    /// A mounted disk image (`Physical Interconnect Location` = `File`).
    disk_image: bool,
}

fn attachment(from: &str) -> Option<Attachment> {
    let media = iokit::media_for_bsd_name(bsd_name(from)?)?;
    let removable = media.boolean("Removable")?;
    let disk_image = iokit::find_in_ancestors(media, |e| {
        let loc = e.dict_string("Protocol Characteristics", "Physical Interconnect Location");
        let bus = e.dict_string("Protocol Characteristics", "Physical Interconnect");
        (loc.is_some() || bus.is_some())
            .then(|| loc.as_deref() == Some("File") || bus.as_deref() == Some("Virtual Interface"))
    })
    .unwrap_or(false);
    Some(Attachment {
        removable,
        disk_image,
    })
}

/// Mount points of the fixed volumes, `/` first. Equivalent of Windows `DRIVE_FIXED`:
/// local, writable, not removable media. Excluded as well: network and hidden system
/// mounts, read-only mounts other than `/`, and disk images (their "device" is a file on
/// another volume, so a benchmark would not measure a drive). A volume whose I/O Kit media
/// cannot be inspected is left out (it cannot be shown to be fixed).
pub fn fixed_volumes() -> Vec<String> {
    let mut out: Vec<String> = mounts()
        .into_iter()
        .filter(is_candidate)
        .filter(|m| {
            m.on == "/" || attachment(&m.from).is_some_and(|a| !a.removable && !a.disk_image)
        })
        .map(|m| m.on)
        .collect();
    out.sort_by(|a, b| (a != "/").cmp(&(b != "/")).then_with(|| a.cmp(b)));
    out.dedup();
    out
}

/// `/` (the booted system volume group), when it is mounted locally.
pub fn system_volume() -> Option<String> {
    let st = statfs("/").ok()?;
    let m = mount_of(&st);
    (m.on == "/" && i64::from(m.flags) & i64::from(libc::MNT_LOCAL) != 0).then(|| "/".to_owned())
}

/// The fixed volume (as listed by [`fixed_volumes`]) that holds `path`. A path on the data
/// volume of the booted volume group maps to `/` when both are in the same APFS container.
pub fn volume_of(path: &str) -> Option<String> {
    let m = mount_of(&statfs(path).ok()?);
    let fixed = fixed_volumes();
    if fixed.contains(&m.on) {
        return Some(m.on);
    }
    if m.on == DATA_VOLUME && fixed.iter().any(|v| v == "/") {
        let root = mount_of(&statfs("/").ok()?);
        if whole_disk(&root.from).is_some() && whole_disk(&root.from) == whole_disk(&m.from) {
            return Some("/".to_owned());
        }
    }
    None
}

/// The per-user temporary directory the OS assigned to this user
/// (`confstr(_CS_DARWIN_USER_TEMP_DIR)`, e.g. `/var/folders/xy/…/T/`). It is what `$TMPDIR`
/// normally points to, but it is read from the OS, so it is right even when `$TMPDIR` was
/// redirected. Owned by the user, mode `0700`.
pub fn user_temp_dir() -> Option<PathBuf> {
    let mut buf = vec![0u8; MAX_MOUNT_PATH + 1];
    // SAFETY: `buf` is writable for `buf.len()` bytes; confstr NUL-terminates within it and
    // returns the needed size including the NUL (0 on error).
    let n = unsafe {
        libc::confstr(
            libc::_CS_DARWIN_USER_TEMP_DIR,
            buf.as_mut_ptr().cast(),
            buf.len(),
        )
    };
    if n == 0 || n > buf.len() {
        return None;
    }
    buf.truncate(n - 1);
    (!buf.is_empty()).then(|| PathBuf::from(std::ffi::OsString::from_vec(buf)))
}

/// `statfs`: `f_bavail` (blocks available to non-superusers) and `f_blocks`, times
/// `f_bsize`. On APFS both describe the shared container.
pub fn volume_space(path: &str) -> Result<VolumeSpace, AppError> {
    let st = statfs(path)?;
    let bsize = u64::from(st.f_bsize);
    let mul = |blocks: u64| {
        blocks.checked_mul(bsize).ok_or_else(|| {
            AppError::new(ErrorCode::Platform, "statfs size overflow").with_detail(path.to_owned())
        })
    };
    Ok(VolumeSpace {
        free_to_caller_bytes: mul(st.f_bavail)?,
        total_bytes: mul(st.f_blocks)?,
    })
}

/// `IOMedia` "Preferred Block Size" of the device mounted at `mount_point`: the natural
/// block size of the media (4096 on Apple SSDs), the closest equivalent of the logical
/// sector size the Windows provider reports. `None` if I/O Kit does not report it.
pub fn volume_sector_size(mount_point: &str) -> Option<u32> {
    let m = mount_of(&statfs(mount_point).ok()?);
    let media = iokit::media_for_bsd_name(bsd_name(&m.from)?)?;
    let size = u32::try_from(media.number("Preferred Block Size")?).ok()?;
    (size > 0).then_some(size)
}

/// "Product Name" from the `Device Characteristics` of the block storage device under the
/// volume (e.g. "APPLE SSD AP1024Z"). `None` when the volume sits on more than one device
/// or no device reports a name.
pub fn volume_disk_model(mount_point: &str) -> Option<String> {
    let m = mount_of(&statfs(mount_point).ok()?);
    let media = iokit::media_for_bsd_name(bsd_name(&m.from)?)?;
    iokit::find_in_ancestors(media, |e| {
        e.dict_string("Device Characteristics", "Product Name")
    })
}

/// IOPowerSources: the source currently powering the Mac. "UPS Power" counts as battery,
/// as Windows does (AC line offline).
pub fn power_source() -> PowerSource {
    let Some(blob) = IOPSCopyPowerSourcesInfo() else {
        return PowerSource::Unknown;
    };
    // SAFETY: `blob` is the snapshot returned by IOPSCopyPowerSourcesInfo, alive here.
    let Some(kind) = (unsafe { IOPSGetProvidingPowerSourceType(Some(&blob)) }) else {
        return PowerSource::Unknown;
    };
    power_source_from(&kind)
}

fn power_source_from(kind: &CFString) -> PowerSource {
    let k = kind.to_string();
    let is = |c: &CStr| c.to_str().is_ok_and(|c| c == k);
    if is(objc2_io_kit::kIOPMACPowerKey) {
        PowerSource::Ac
    } else if is(objc2_io_kit::kIOPMBatteryPowerKey) || k == "UPS Power" {
        PowerSource::Battery
    } else {
        PowerSource::Unknown
    }
}

/// macOS has no power plans. The closest OS-wide energy policy readable without
/// privileges is Low Power Mode (`NSProcessInfo.isLowPowerModeEnabled`, macOS 12+); it is
/// reported as "Low Power Mode: on/off". Other energy modes (e.g. High Power) are not
/// exposed by a public API and are not guessed.
pub fn active_power_plan() -> Option<String> {
    let on = objc2_foundation::NSProcessInfo::processInfo().isLowPowerModeEnabled();
    Some(if on {
        "Low Power Mode: on".to_owned()
    } else {
        "Low Power Mode: off".to_owned()
    })
}

fn fcntl_err(op: &str, e: std::io::Error) -> AppError {
    AppError::new(ErrorCode::Platform, format!("{op} failed")).with_detail(e.to_string())
}

/// `fcntl(F_NOCACHE, 1)`: reads and writes on `file` bypass the unified buffer cache
/// (direct I/O for aligned requests). Applies to every descriptor sharing the open file.
pub fn disable_cache(file: &File) -> Result<(), AppError> {
    // SAFETY: valid open descriptor owned by `file`; F_NOCACHE takes an int argument.
    if unsafe { libc::fcntl(file.as_raw_fd(), libc::F_NOCACHE, 1) } == -1 {
        return Err(fcntl_err(
            "fcntl(F_NOCACHE)",
            std::io::Error::last_os_error(),
        ));
    }
    Ok(())
}

/// `fcntl(F_FULLFSYNC)`: writes the file's data to the device and asks the drive to flush
/// its own volatile cache (plain `fsync` on macOS stops at the drive).
pub fn full_fsync(file: &File) -> Result<(), AppError> {
    // SAFETY: valid open descriptor owned by `file`; F_FULLFSYNC takes no argument.
    if unsafe { libc::fcntl(file.as_raw_fd(), libc::F_FULLFSYNC) } == -1 {
        return Err(fcntl_err(
            "fcntl(F_FULLFSYNC)",
            std::io::Error::last_os_error(),
        ));
    }
    Ok(())
}

/// A mount point the user may name: an absolute path without `.`/`..` or empty
/// components, at most `MNAMELEN` bytes; trailing slashes are removed (`/` stays `/`).
pub fn normalize_mount_point(input: &str) -> Option<String> {
    let s = input.trim();
    if !s.starts_with('/') || s.len() > MAX_MOUNT_PATH || s.contains('\0') {
        return None;
    }
    let trimmed = s.trim_end_matches('/');
    if trimmed.is_empty() {
        return Some("/".to_owned());
    }
    let ok = trimmed
        .split('/')
        .skip(1)
        .all(|c| !c.is_empty() && c != "." && c != "..");
    ok.then(|| trimmed.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(on: &str, from: &str, flags: i32) -> Mount {
        Mount {
            on: on.into(),
            from: from.into(),
            fs_type: "apfs".into(),
            flags: flags as u32,
        }
    }

    #[test]
    fn mount_filter_and_names() {
        let local = libc::MNT_LOCAL;
        let ro = libc::MNT_RDONLY;
        let hidden = libc::MNT_DONTBROWSE;
        assert!(is_candidate(&m("/", "/dev/disk3s1s1", local | ro)));
        assert!(is_candidate(&m("/Volumes/Work", "/dev/disk5s1", local)));
        assert!(!is_candidate(&m(
            "/Volumes/Installer",
            "/dev/disk6s1",
            local | ro
        )));
        assert!(!is_candidate(&m(
            DATA_VOLUME,
            "/dev/disk3s5",
            local | hidden
        )));
        assert!(!is_candidate(&m(
            "/System/Volumes/VM",
            "/dev/disk3s6",
            local | hidden
        )));
        assert!(!is_candidate(&m("/Volumes/share", "//u@nas/share", 0)));
        assert!(!is_candidate(&m("/dev", "devfs", local | hidden)));
        assert!(!is_candidate(&m("/Volumes/x", "map auto_home", local)));
        assert_eq!(bsd_name("/dev/disk3s5"), Some("disk3s5"));
        assert_eq!(bsd_name("/dev/../disk3"), None);
        assert_eq!(whole_disk("/dev/disk3s1s1"), Some("disk3"));
        assert_eq!(whole_disk("/dev/disk12s2"), Some("disk12"));
        assert_eq!(whole_disk("/dev/diskX"), None);
    }

    #[test]
    fn mount_points_normalize() {
        assert_eq!(normalize_mount_point("/").as_deref(), Some("/"));
        assert_eq!(normalize_mount_point(" // ").as_deref(), Some("/"));
        assert_eq!(
            normalize_mount_point("/Volumes/Data SSD/").as_deref(),
            Some("/Volumes/Data SSD")
        );
        for bad in [
            "",
            "C:\\",
            "c",
            "Volumes/x",
            "/Volumes/../etc",
            "/a//b",
            "/a/./b",
        ] {
            assert_eq!(normalize_mount_point(bad), None, "{bad}");
        }
    }

    #[test]
    fn power_source_keys() {
        assert_eq!(
            power_source_from(&CFString::from_str("AC Power")),
            PowerSource::Ac
        );
        assert_eq!(
            power_source_from(&CFString::from_str("Battery Power")),
            PowerSource::Battery
        );
        assert_eq!(
            power_source_from(&CFString::from_str("Off Line")),
            PowerSource::Unknown
        );
    }

    /// Host test on this Mac: real values, printed for the report.
    #[test]
    fn host_volumes_are_plausible() {
        let vols = fixed_volumes();
        eprintln!("fixed volumes: {vols:?}");
        assert_eq!(vols.first().map(String::as_str), Some("/"));
        assert_eq!(system_volume().as_deref(), Some("/"));
        let root = volume_space("/").expect("space /");
        let data = volume_space(DATA_VOLUME).expect("space data");
        eprintln!("/: {root:?}; data: {data:?}");
        assert!(root.total_bytes > 1 << 30 && root.free_to_caller_bytes <= root.total_bytes);
        // Same APFS container: same total.
        assert_eq!(root.total_bytes, data.total_bytes);
        let tmp = std::env::temp_dir();
        assert_eq!(
            volume_of(tmp.to_str().expect("utf-8")).as_deref(),
            Some("/")
        );
        let user_tmp = user_temp_dir().expect("user temp dir");
        eprintln!("user temp dir: {}", user_tmp.display());
        assert!(user_tmp.is_absolute() && user_tmp.is_dir());
        assert_eq!(
            volume_of(user_tmp.to_str().expect("utf-8")).as_deref(),
            Some("/")
        );
        let sector = volume_sector_size("/").expect("block size");
        eprintln!("preferred block size: {sector}");
        assert!(sector.is_power_of_two() && sector >= 512);
        let model = volume_disk_model("/");
        eprintln!("disk model: {model:?}");
        assert!(model.is_some_and(|m| !m.is_empty()));
        eprintln!("power source: {:?}", power_source());
        eprintln!("power plan: {:?}", active_power_plan());
        assert_ne!(power_source(), PowerSource::Unknown);
    }
}
