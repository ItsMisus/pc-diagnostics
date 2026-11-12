//! Storage workloads (methodology v1). Only ever touches one new temporary file that this
//! module creates; never user data, never raw devices.
//!
//! - Location: on the system volume the user's `%TEMP%` (which lives there); on any other
//!   fixed volume `<X:>\PCDiagnosticsBench\` (created if missing).
//! - Name: `PCDiagnosticsBench-<run id>.tmp`. Only files matching exactly this pattern in
//!   exactly those directories are ever deleted by the stale-file sweep.
//! - Opened with `FILE_FLAG_NO_BUFFERING | FILE_FLAG_WRITE_THROUGH | FILE_FLAG_DELETE_ON_CLOSE`:
//!   no OS cache, writes reach the device, and the OS deletes the file when the last handle
//!   closes, even if the process crashes. `BenchFile`'s `Drop` also deletes it explicitly.
//! - Buffers are aligned to `max(4096, logical sector size)`; data is incompressible
//!   xorshift output with a unique (nonce, block) stamp per block (no dedup).
//! - Every write is charged against a per-run byte budget (`validate::write_budget`).
//!
//! macOS (same guarantees, native equivalents):
//! - Location: on `/` the user's `$TMPDIR` when it is on the data volume of the booted
//!   volume group (it always is unless redirected); on any other fixed volume
//!   `<mount>/PCDiagnosticsBench/`. Same name pattern, created `0600` with `O_EXCL`.
//! - The file is unlinked right after it is opened (the equivalent of
//!   `FILE_FLAG_DELETE_ON_CLOSE`): the kernel frees it when the last descriptor closes,
//!   including when the process crashes; no name is left behind on any exit path.
//! - `F_NOCACHE` on the open file (equivalent of `FILE_FLAG_NO_BUFFERING`): reads and writes
//!   bypass the unified buffer cache. Random-read threads use duplicated descriptors of the
//!   same open file (positional `pread`, no shared offset).
//! - macOS has no per-write write-through for unprivileged processes. `F_NOCACHE` writes go
//!   to the drive but may sit in its volatile cache, so every timed write phase ends with
//!   `F_FULLFSYNC` (drive cache flush) inside the timed window: when the phase's clock
//!   stops, the data is on stable storage, as with `FILE_FLAG_WRITE_THROUGH`. Per-write
//!   latencies do not include that final flush.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use models::{AppError, ErrorCode, Unit};

use crate::ids;
use crate::runtime::{Outcome, RunCtx, RunError, metric};
use crate::validate::MIB;

pub(crate) const FILE_PREFIX: &str = "PCDiagnosticsBench-";
pub(crate) const FILE_SUFFIX: &str = ".tmp";
pub(crate) const DIR_NAME: &str = "PCDiagnosticsBench";
pub(crate) const SEQ_BLOCK: usize = MIB as usize;
pub(crate) const RANDOM_BLOCK: usize = 4096;
/// Length of each timed random phase.
pub(crate) const RANDOM_PHASE: Duration = Duration::from_secs(5);
pub(crate) const RANDOM_THREADS: usize = 4;
const POOL_BLOCKS: usize = 8;

pub(crate) fn bench_file_name(run_id: &str) -> String {
    format!("{FILE_PREFIX}{run_id}{FILE_SUFFIX}")
}

/// True only for names this module creates: prefix + run id shape + suffix.
pub(crate) fn is_bench_file_name(name: &str) -> bool {
    name.strip_prefix(FILE_PREFIX)
        .and_then(|r| r.strip_suffix(FILE_SUFFIX))
        .is_some_and(ids::is_run_id)
}

/// Directory for the test file. System volume → `%TEMP%` if it is on that volume.
#[cfg(not(target_os = "macos"))]
pub(crate) fn test_dir(volume: &str, system_volume: Option<&str>, temp: &Path) -> PathBuf {
    let temp_on_volume = temp
        .to_str()
        .and_then(|t| t.get(..3))
        .is_some_and(|p| p.eq_ignore_ascii_case(volume));
    if system_volume.is_some_and(|s| s.eq_ignore_ascii_case(volume)) && temp_on_volume {
        temp.to_path_buf()
    } else {
        Path::new(volume).join(DIR_NAME)
    }
}

/// Directory for the test file. System volume (`/`) → `temp` (`$TMPDIR`) if it is on that
/// volume group, else the per-user temporary directory the OS assigned
/// (`confstr(_CS_DARWIN_USER_TEMP_DIR)`, normally the same directory) if that one is. The
/// volume of each candidate is asked of the OS (`platform::bench_env::volume_of`).
#[cfg(target_os = "macos")]
pub(crate) fn test_dir(volume: &str, system_volume: Option<&str>, temp: &Path) -> PathBuf {
    let candidates: Vec<(PathBuf, Option<String>)> = std::iter::once(temp.to_path_buf())
        .chain(platform::bench_env::user_temp_dir())
        .map(|d| {
            let v = d.to_str().and_then(platform::bench_env::volume_of);
            (d, v)
        })
        .collect();
    test_dir_for(volume, system_volume, &candidates)
}

/// Pure part of the macOS [`test_dir`]: `temps` are the candidate temporary directories in
/// order of preference, each with the fixed volume that holds it. Any other case →
/// `<volume>/PCDiagnosticsBench`. On `/` that path is on the sealed read-only system
/// volume, so creating it fails with a clear error instead of writing elsewhere.
#[cfg(target_os = "macos")]
pub(crate) fn test_dir_for(
    volume: &str,
    system_volume: Option<&str>,
    temps: &[(PathBuf, Option<String>)],
) -> PathBuf {
    let on_volume = temps
        .iter()
        .find(|(_, v)| v.as_deref() == Some(volume))
        .map(|(d, _)| d.clone());
    match on_volume {
        Some(d) if system_volume == Some(volume) => d,
        _ => Path::new(volume).join(DIR_NAME),
    }
}

/// Directories the sweep looks in: `%TEMP%` (macOS: `$TMPDIR` and the OS per-user
/// temporary directory) and `<X:>\PCDiagnosticsBench` per fixed volume.
pub(crate) fn sweep_dirs() -> Vec<PathBuf> {
    let mut dirs = vec![std::env::temp_dir()];
    #[cfg(target_os = "macos")]
    if let Some(d) = platform::bench_env::user_temp_dir()
        && !dirs.iter().any(|x| same_dir(x, &d))
    {
        dirs.push(d);
    }
    dirs.extend(
        platform::bench_env::fixed_volumes()
            .into_iter()
            .map(|v| Path::new(&v).join(DIR_NAME)),
    );
    dirs
}

/// `a` and `b` name the same directory (trailing slashes and symlinks such as
/// `/var` → `/private/var` resolved).
#[cfg(target_os = "macos")]
fn same_dir(a: &Path, b: &Path) -> bool {
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

/// Deletes leftover test files (from a crash or power loss) in `dirs`. Only regular files
/// whose name matches [`is_bench_file_name`] are removed; a file still open by a running
/// benchmark cannot be deleted (no `FILE_SHARE_DELETE`) and is skipped. On macOS a running
/// benchmark's file has no name (unlinked right after it is opened), so only a leftover
/// from a crash between creation and unlink can match. Returns the count.
pub(crate) fn sweep_stale(dirs: &[PathBuf]) -> usize {
    let mut removed = 0;
    for dir in dirs {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        for e in entries.flatten() {
            let name = e.file_name();
            let Some(name) = name.to_str() else { continue };
            if !is_bench_file_name(name) {
                continue;
            }
            let path = e.path();
            let is_file = std::fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_file());
            if is_file && std::fs::remove_file(&path).is_ok() {
                tracing::info!(path = %path.display(), "removed stale benchmark file");
                removed += 1;
            }
        }
        if dir.file_name().is_some_and(|n| n == DIR_NAME) {
            // Only succeeds when empty.
            let _ = std::fs::remove_dir(dir);
        }
    }
    removed
}

/// Heap buffer whose usable slice starts at an `align`-byte boundary (safe code: the
/// allocation is over-sized and the slice offset chosen with `align_offset`).
pub(crate) struct AlignedBuf {
    raw: Vec<u8>,
    off: usize,
    len: usize,
}

impl AlignedBuf {
    pub fn new(len: usize, align: usize) -> Result<Self, AppError> {
        let raw = vec![0u8; len + align];
        let off = raw.as_ptr().align_offset(align);
        if off >= align {
            return Err(AppError::internal("could not align the I/O buffer"));
        }
        Ok(Self { raw, off, len })
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.raw[self.off..self.off + self.len]
    }

    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut self.raw[self.off..self.off + self.len]
    }
}

fn xorshift(x: &mut u64) -> u64 {
    *x ^= *x << 13;
    *x ^= *x >> 7;
    *x ^= *x << 17;
    *x
}

fn fill_random(buf: &mut [u8], seed: &mut u64) {
    for c in buf.as_chunks_mut::<8>().0 {
        c.copy_from_slice(&xorshift(seed).to_le_bytes());
    }
}

/// Writes the (nonce, block) stamp in the first 16 bytes.
fn stamp(buf: &mut [u8], nonce: u64, block: u64) {
    buf[..8].copy_from_slice(&nonce.to_le_bytes());
    buf[8..16].copy_from_slice(&block.to_le_bytes());
}

fn read_stamp(buf: &[u8]) -> (u64, u64) {
    let mut a = [0u8; 8];
    let mut b = [0u8; 8];
    a.copy_from_slice(&buf[..8]);
    b.copy_from_slice(&buf[8..16]);
    (u64::from_le_bytes(a), u64::from_le_bytes(b))
}

/// Bytes a run may still write.
pub(crate) struct WriteBudget {
    limit: u64,
    used: u64,
}

impl WriteBudget {
    pub fn new(limit: u64) -> Self {
        Self { limit, used: 0 }
    }

    pub fn charge(&mut self, n: u64) -> Result<(), AppError> {
        let next = self.used.saturating_add(n);
        if next > self.limit {
            return Err(AppError::internal("storage write budget exceeded")
                .with_detail(format!("used {} + {} > limit {}", self.used, n, self.limit)));
        }
        self.used = next;
        Ok(())
    }

    pub fn used(&self) -> u64 {
        self.used
    }
}

/// Rate in bytes/s with a floor on the elapsed time (avoids division by ~0).
fn rate(bytes: u64, elapsed: Duration) -> f64 {
    bytes as f64 / elapsed.as_secs_f64().max(1e-9)
}

/// The workloads (shared by Windows and macOS). The OS-specific parts (opening the test
/// file, positional I/O, flushing) live in `win` / `mac` behind the same names.
#[cfg(any(windows, target_os = "macos"))]
mod imp {
    use super::*;
    use std::fs::File;

    use models::benchmark::BenchmarkKind;
    use models::benchmark_run::{BenchmarkSeries, SeriesPoint};
    use models::settings::Language;

    use crate::runtime::join_all;
    use crate::validate::{self, Plan, RANDOM_WRITE_CAP_BYTES};

    #[cfg(target_os = "macos")]
    pub(super) use mac::{BenchFile, flush_to_device, read_exact_at, write_all_at};
    #[cfg(windows)]
    use win::{BenchFile, flush_to_device, read_exact_at, write_all_at};

    #[cfg(target_os = "macos")]
    mod mac {
        use super::*;
        use std::fs::OpenOptions;
        use std::os::unix::fs::{FileExt, OpenOptionsExt};

        fn io_err(op: &str, path: &Path, e: &std::io::Error) -> AppError {
            let code = if e.kind() == std::io::ErrorKind::PermissionDenied {
                ErrorCode::PermissionDenied
            } else {
                ErrorCode::Platform
            };
            AppError::new(code, format!("storage benchmark: {op} failed"))
                .with_detail(format!("{}: {e}", path.display()))
        }

        /// Our own `PCDiagnosticsBench` directory is removed again when empty; `$TMPDIR`
        /// never is.
        fn own_dir(dir: &Path) -> Option<PathBuf> {
            dir.file_name()
                .is_some_and(|n| n == DIR_NAME)
                .then(|| dir.to_path_buf())
        }

        fn ensure_dir(dir: &Path) -> Result<(), AppError> {
            match std::fs::create_dir(dir) {
                Ok(()) => Ok(()),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
                Err(e) => Err(io_err("create directory", dir, &e)),
            }
        }

        /// The temporary test file, already unlinked (see the module docs). Dropping it
        /// closes the descriptor (the kernel frees the blocks), deletes the name again as a
        /// fallback and removes the `PCDiagnosticsBench` directory if this run created it
        /// and it is empty.
        pub(crate) struct BenchFile {
            /// Where the file was created (no longer linked while the run is going).
            pub path: PathBuf,
            file: Option<File>,
            created_dir: Option<PathBuf>,
        }

        impl BenchFile {
            pub fn create(dir: &Path, run_id: &str, size: u64) -> Result<Self, AppError> {
                // Constructed before opening so the directory is cleaned up on any error.
                let mut me = Self {
                    path: dir.join(bench_file_name(run_id)),
                    file: None,
                    created_dir: own_dir(dir),
                };
                let mut attempt = 0;
                let file = loop {
                    attempt += 1;
                    ensure_dir(dir)?;
                    let opened = OpenOptions::new()
                        .read(true)
                        .write(true)
                        .create_new(true)
                        .mode(0o600)
                        .custom_flags(libc::O_NOFOLLOW)
                        .open(&me.path);
                    match opened {
                        Ok(f) => break f,
                        // The empty directory was removed concurrently (another run's
                        // cleanup or the stale-file sweep): recreate it and retry.
                        Err(e) if e.kind() == std::io::ErrorKind::NotFound && attempt < 3 => {}
                        Err(e) => return Err(io_err("create test file", &me.path, &e)),
                    }
                };
                // Owned from here: `Drop` closes it on every error path below.
                me.file = Some(file);
                std::fs::remove_file(&me.path)
                    .map_err(|e| io_err("unlink test file", &me.path, &e))?;
                platform::bench_env::disable_cache(me.file())?;
                me.file()
                    .set_len(size)
                    .map_err(|e| io_err("set test file size", &me.path, &e))?;
                Ok(me)
            }

            pub fn file(&self) -> &File {
                // Always Some after `create` stored it.
                self.file
                    .as_ref()
                    .unwrap_or_else(|| unreachable!("file is open"))
            }

            /// An extra descriptor of the same open file (one per random-read thread). The
            /// name is already unlinked, so it cannot be reopened by path; `F_NOCACHE`
            /// belongs to the open file and applies to this descriptor too.
            pub fn open_reader(&self) -> Result<File, AppError> {
                self.file()
                    .try_clone()
                    .map_err(|e| io_err("duplicate reader descriptor", &self.path, &e))
            }
        }

        impl Drop for BenchFile {
            fn drop(&mut self) {
                drop(self.file.take());
                match std::fs::remove_file(&self.path) {
                    Ok(()) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                    Err(e) => {
                        tracing::warn!(path = %self.path.display(), error = %e, "could not delete benchmark file")
                    }
                }
                if let Some(d) = self.created_dir.take() {
                    let _ = std::fs::remove_dir(d);
                }
            }
        }

        pub(crate) fn write_all_at(
            f: &File,
            buf: &[u8],
            offset: u64,
            path: &Path,
        ) -> Result<(), AppError> {
            f.write_all_at(buf, offset)
                .map_err(|e| io_err("write", path, &e))
        }

        pub(crate) fn read_exact_at(
            f: &File,
            buf: &mut [u8],
            offset: u64,
            path: &Path,
        ) -> Result<(), AppError> {
            f.read_exact_at(buf, offset)
                .map_err(|e| io_err("read", path, &e))
        }

        /// `F_FULLFSYNC`: the written data and the drive's cache reach stable storage.
        pub(crate) fn flush_to_device(f: &File, _path: &Path) -> Result<(), AppError> {
            platform::bench_env::full_fsync(f)
        }
    }

    #[cfg(windows)]
    mod win {
        use super::*;
        use std::fs::OpenOptions;
        use std::os::windows::fs::{FileExt, OpenOptionsExt};

        const FILE_FLAG_WRITE_THROUGH: u32 = 0x8000_0000;
        const FILE_FLAG_NO_BUFFERING: u32 = 0x2000_0000;
        const FILE_FLAG_DELETE_ON_CLOSE: u32 = 0x0400_0000;
        const FILE_ATTRIBUTE_TEMPORARY: u32 = 0x0000_0100;
        const GENERIC_READ: u32 = 0x8000_0000;
        const GENERIC_WRITE: u32 = 0x4000_0000;
        const DELETE: u32 = 0x0001_0000;
        const FILE_SHARE_READ: u32 = 0x1;
        const FILE_SHARE_WRITE: u32 = 0x2;
        const FILE_SHARE_DELETE: u32 = 0x4;

        fn io_err(op: &str, path: &Path, e: &std::io::Error) -> AppError {
            let code = if e.kind() == std::io::ErrorKind::PermissionDenied {
                ErrorCode::PermissionDenied
            } else {
                ErrorCode::Platform
            };
            AppError::new(code, format!("storage benchmark: {op} failed"))
                .with_detail(format!("{}: {e}", path.display()))
        }

        /// The temporary test file. Dropping it closes every handle (the OS deletes the file
        /// because of `FILE_FLAG_DELETE_ON_CLOSE`), then deletes it explicitly as a fallback
        /// and removes the `PCDiagnosticsBench` directory if this run created it and it is empty.
        pub(crate) struct BenchFile {
            pub path: PathBuf,
            file: Option<File>,
            created_dir: Option<PathBuf>,
        }

        impl BenchFile {
            pub fn create(dir: &Path, run_id: &str, size: u64) -> Result<Self, AppError> {
                // Our own `PCDiagnosticsBench` directory is removed again when empty. `%TEMP%`
                // is never removed.
                let own_dir = dir.file_name().is_some_and(|n| n == DIR_NAME);
                // Constructed before opening so the directory is cleaned up on any error below.
                let mut me = Self {
                    path: dir.join(bench_file_name(run_id)),
                    file: None,
                    created_dir: own_dir.then(|| dir.to_path_buf()),
                };
                let mut attempt = 0;
                let file = loop {
                    attempt += 1;
                    match std::fs::create_dir(dir) {
                        Ok(()) => {}
                        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
                        Err(e) => return Err(io_err("create directory", dir, &e)),
                    }
                    let opened = OpenOptions::new()
                        .read(true)
                        .write(true)
                        .create_new(true)
                        .access_mode(GENERIC_READ | GENERIC_WRITE | DELETE)
                        .share_mode(FILE_SHARE_READ)
                        .custom_flags(
                            FILE_FLAG_NO_BUFFERING
                                | FILE_FLAG_WRITE_THROUGH
                                | FILE_FLAG_DELETE_ON_CLOSE,
                        )
                        .attributes(FILE_ATTRIBUTE_TEMPORARY)
                        .open(&me.path);
                    match opened {
                        Ok(f) => break f,
                        // The empty directory was removed concurrently (another run's cleanup
                        // or the stale-file sweep): recreate it and retry.
                        Err(e) if e.kind() == std::io::ErrorKind::NotFound && attempt < 3 => {}
                        Err(e) => return Err(io_err("create test file", &me.path, &e)),
                    }
                };
                file.set_len(size)
                    .map_err(|e| io_err("set test file size", &me.path, &e))?;
                me.file = Some(file);
                Ok(me)
            }

            pub fn file(&self) -> &File {
                // Always Some after `create` returned Ok.
                self.file
                    .as_ref()
                    .unwrap_or_else(|| unreachable!("file is open"))
            }

            /// An extra unbuffered read-only handle (one per random-read thread: synchronous
            /// handles serialize I/O, so threads need their own).
            pub fn open_reader(&self) -> Result<File, AppError> {
                OpenOptions::new()
                    .read(true)
                    .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
                    .custom_flags(FILE_FLAG_NO_BUFFERING)
                    .open(&self.path)
                    .map_err(|e| io_err("open reader handle", &self.path, &e))
            }
        }

        impl Drop for BenchFile {
            fn drop(&mut self) {
                drop(self.file.take());
                match std::fs::remove_file(&self.path) {
                    Ok(()) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                    Err(e) => {
                        tracing::warn!(path = %self.path.display(), error = %e, "could not delete benchmark file")
                    }
                }
                if let Some(d) = self.created_dir.take() {
                    let _ = std::fs::remove_dir(d);
                }
            }
        }

        pub(super) fn write_all_at(
            f: &File,
            buf: &[u8],
            offset: u64,
            path: &Path,
        ) -> Result<(), AppError> {
            let mut done = 0usize;
            while done < buf.len() {
                let n = f
                    .seek_write(&buf[done..], offset + done as u64)
                    .map_err(|e| io_err("write", path, &e))?;
                if n == 0 {
                    return Err(io_err(
                        "write",
                        path,
                        &std::io::Error::other("wrote 0 bytes"),
                    ));
                }
                done += n;
            }
            Ok(())
        }

        pub(super) fn read_exact_at(
            f: &File,
            buf: &mut [u8],
            offset: u64,
            path: &Path,
        ) -> Result<(), AppError> {
            let mut done = 0usize;
            while done < buf.len() {
                let n = f
                    .seek_read(&mut buf[done..], offset + done as u64)
                    .map_err(|e| io_err("read", path, &e))?;
                if n == 0 {
                    return Err(io_err(
                        "read",
                        path,
                        &std::io::Error::other("unexpected end of file"),
                    ));
                }
                done += n;
            }
            Ok(())
        }

        /// Every write is `FILE_FLAG_WRITE_THROUGH`: nothing is left to flush.
        pub(super) fn flush_to_device(_f: &File, _path: &Path) -> Result<(), AppError> {
            Ok(())
        }
    }

    /// I/O alignment for the volume: `max(4096, logical sector)`, power of two ≤ 64 KiB.
    fn io_alignment(volume: &str) -> Result<usize, AppError> {
        let sector = platform::bench_env::volume_sector_size(volume).unwrap_or(4096) as usize;
        let align = sector.max(4096);
        if !align.is_power_of_two() || align > 64 * 1024 {
            return Err(
                AppError::new(ErrorCode::NotSupported, "unsupported sector size")
                    .with_detail(format!("{volume}: {sector} bytes per sector")),
            );
        }
        Ok(align)
    }

    struct Setup {
        file: BenchFile,
        pool: Vec<AlignedBuf>,
        align: usize,
        nonce: u64,
        size: u64,
    }

    fn setup(ctx: &RunCtx, plan: &Plan, run_id: &str) -> Result<Setup, RunError> {
        let volume = plan
            .volume
            .as_deref()
            .ok_or_else(|| AppError::internal("storage plan without volume"))?;
        let size = u64::from(plan.file_mib) * MIB;
        let space = platform::bench_env::volume_space(volume)?;
        validate::check_free_space(space.free_to_caller_bytes, space.total_bytes, size)?;
        let align = io_alignment(volume)?;
        let dir = test_dir(
            volume,
            platform::bench_env::system_volume().as_deref(),
            &std::env::temp_dir(),
        );
        ctx.check()?;
        let file = BenchFile::create(&dir, run_id, size)?;
        let mut seed = 0x9E37_79B9_7F4A_7C15 ^ size ^ run_id.len() as u64;
        for b in run_id.bytes() {
            seed = seed.rotate_left(5) ^ u64::from(b);
        }
        let nonce = xorshift(&mut seed);
        let mut pool = Vec::with_capacity(POOL_BLOCKS);
        for _ in 0..POOL_BLOCKS {
            let mut b = AlignedBuf::new(SEQ_BLOCK, align)?;
            fill_random(b.as_mut_slice(), &mut seed);
            pool.push(b);
        }
        Ok(Setup {
            file,
            pool,
            align,
            nonce,
            size,
        })
    }

    /// Sequential write of the whole file in 1 MiB blocks. Returns elapsed time and the
    /// per-second rate series. `f0..f1` is the progress range of this phase.
    #[allow(clippy::too_many_arguments)]
    fn write_sequential(
        ctx: &RunCtx,
        su: &mut Setup,
        budget: &mut WriteBudget,
        phase: &str,
        f0: f64,
        f1: f64,
        interim_id: Option<&str>,
    ) -> Result<(Duration, Vec<SeriesPoint>), RunError> {
        let blocks = su.size / SEQ_BLOCK as u64;
        let mut points = Vec::new();
        ctx.set_phase(phase, f0);
        let t0 = Instant::now();
        let (mut tick, mut tick_bytes) = (t0, 0u64);
        for i in 0..blocks {
            ctx.check()?;
            let buf = &mut su.pool[(i as usize) % POOL_BLOCKS];
            stamp(buf.as_mut_slice(), su.nonce, i);
            budget.charge(SEQ_BLOCK as u64)?;
            write_all_at(
                su.file.file(),
                buf.as_slice(),
                i * SEQ_BLOCK as u64,
                &su.file.path,
            )?;
            tick_bytes += SEQ_BLOCK as u64;
            let now = Instant::now();
            if now - tick >= Duration::from_secs(1) {
                let r = rate(tick_bytes, now - tick);
                points.push(SeriesPoint {
                    elapsed_ms: ctx.elapsed_ms(),
                    value: r,
                });
                if let Some(id) = interim_id {
                    ctx.set_interim(vec![metric(id, phase, r, Unit::BytesPerSecond, true)]);
                }
                (tick, tick_bytes) = (now, 0);
            }
            ctx.set_fraction(f0 + (f1 - f0) * (i + 1) as f64 / blocks as f64);
        }
        // Inside the timed window (no-op on Windows, where every write is write-through).
        flush_to_device(su.file.file(), &su.file.path)?;
        Ok((t0.elapsed(), points))
    }

    fn phase_prepare(lang: Language) -> &'static str {
        lang.pick("Preparazione file", "Preparing the file")
    }

    fn seq_write_text(lang: Language) -> &'static str {
        lang.pick("Scrittura sequenziale", "Sequential write")
    }

    fn seq_read_text(lang: Language) -> &'static str {
        lang.pick("Lettura sequenziale", "Sequential read")
    }

    fn bytes_written_text(lang: Language) -> &'static str {
        lang.pick("Dati scritti su disco", "Data written to disk")
    }

    pub(crate) fn run_sequential(
        ctx: &RunCtx,
        plan: &Plan,
        run_id: &str,
    ) -> Result<Outcome, RunError> {
        let lang = ctx.lang;
        ctx.set_phase(phase_prepare(lang), 0.0);
        let mut su = setup(ctx, plan, run_id)?;
        let mut budget = WriteBudget::new(validate::write_budget(plan.kind, plan.file_mib));
        let (write_time, write_points) = write_sequential(
            ctx,
            &mut su,
            &mut budget,
            seq_write_text(lang),
            0.0,
            0.5,
            Some("seq_write"),
        )?;

        ctx.set_phase(seq_read_text(lang), 0.5);
        let blocks = su.size / SEQ_BLOCK as u64;
        let mut rbuf = AlignedBuf::new(SEQ_BLOCK, su.align)?;
        let mut read_points = Vec::new();
        let t0 = Instant::now();
        let (mut tick, mut tick_bytes) = (t0, 0u64);
        for i in 0..blocks {
            ctx.check()?;
            read_exact_at(
                su.file.file(),
                rbuf.as_mut_slice(),
                i * SEQ_BLOCK as u64,
                &su.file.path,
            )?;
            if read_stamp(rbuf.as_slice()) != (su.nonce, i) {
                return Err(RunError::Failed(
                    AppError::new(
                        ErrorCode::Platform,
                        "read-back data does not match what was written",
                    )
                    .with_detail(format!("block {i}")),
                ));
            }
            tick_bytes += SEQ_BLOCK as u64;
            let now = Instant::now();
            if now - tick >= Duration::from_secs(1) {
                let r = rate(tick_bytes, now - tick);
                read_points.push(SeriesPoint {
                    elapsed_ms: ctx.elapsed_ms(),
                    value: r,
                });
                ctx.set_interim(vec![metric(
                    "seq_read",
                    seq_read_text(lang),
                    r,
                    Unit::BytesPerSecond,
                    true,
                )]);
                (tick, tick_bytes) = (now, 0);
            }
            ctx.set_fraction(0.5 + 0.5 * (i + 1) as f64 / blocks as f64);
        }
        let read_time = t0.elapsed();
        let written = budget.used();
        drop(su);

        let mut out = Outcome::default();
        out.metrics.push(metric(
            "seq_write",
            lang.pick(
                "Scrittura sequenziale (blocchi 1 MiB, QD1)",
                "Sequential write (1 MiB blocks, QD1)",
            ),
            rate(written, write_time),
            Unit::BytesPerSecond,
            true,
        ));
        out.metrics.push(metric(
            "seq_read",
            lang.pick(
                "Lettura sequenziale (blocchi 1 MiB, QD1)",
                "Sequential read (1 MiB blocks, QD1)",
            ),
            rate(blocks * SEQ_BLOCK as u64, read_time),
            Unit::BytesPerSecond,
            true,
        ));
        out.metrics.push(metric(
            "bytes_written",
            bytes_written_text(lang),
            written as f64,
            Unit::Bytes,
            false,
        ));
        out.series.push(BenchmarkSeries {
            id: "seq_write_rate".into(),
            label: lang
                .pick(
                    "Scrittura sequenziale al secondo",
                    "Sequential write per second",
                )
                .into(),
            unit: Unit::BytesPerSecond,
            points: write_points,
        });
        out.series.push(BenchmarkSeries {
            id: "seq_read_rate".into(),
            label: lang
                .pick(
                    "Lettura sequenziale al secondo",
                    "Sequential read per second",
                )
                .into(),
            unit: Unit::BytesPerSecond,
            points: read_points,
        });
        Ok(out)
    }

    #[derive(Default, Clone, Copy)]
    struct RandomStats {
        ops: u64,
        latency_sum: Duration,
        elapsed: Duration,
    }

    /// Random reads of `block` bytes from `f` for `phase` or until cancelled.
    #[allow(clippy::too_many_arguments)]
    fn random_reads(
        ctx: &RunCtx,
        f: &File,
        path: &Path,
        size: u64,
        block: usize,
        align: usize,
        phase: Duration,
        mut seed: u64,
        progress: Option<(f64, f64)>,
    ) -> Result<RandomStats, RunError> {
        let slots = size / block as u64;
        let mut buf = AlignedBuf::new(block, align)?;
        let mut st = RandomStats::default();
        let mut fold = 0u64;
        let t0 = Instant::now();
        while t0.elapsed() < phase {
            ctx.check()?;
            let off = (xorshift(&mut seed) % slots) * block as u64;
            let s = Instant::now();
            read_exact_at(f, buf.as_mut_slice(), off, path)?;
            st.latency_sum += s.elapsed();
            st.ops += 1;
            fold ^= u64::from(buf.as_slice()[block - 1]);
            if let Some((f0, f1)) = progress
                && st.ops.is_multiple_of(64)
            {
                ctx.set_fraction(
                    f0 + (f1 - f0) * (t0.elapsed().as_secs_f64() / phase.as_secs_f64()),
                );
            }
        }
        std::hint::black_box(fold);
        st.elapsed = t0.elapsed();
        Ok(st)
    }

    pub(crate) fn run_random(ctx: &RunCtx, plan: &Plan, run_id: &str) -> Result<Outcome, RunError> {
        let lang = ctx.lang;
        let phase = ctx.hooks.storage_phase.unwrap_or(RANDOM_PHASE);
        ctx.set_phase(phase_prepare(lang), 0.0);
        let mut su = setup(ctx, plan, run_id)?;
        let mut budget = WriteBudget::new(validate::write_budget(
            BenchmarkKind::StorageRandom,
            plan.file_mib,
        ));
        write_sequential(
            ctx,
            &mut su,
            &mut budget,
            phase_prepare(lang),
            0.0,
            0.4,
            None,
        )?;
        let block = RANDOM_BLOCK.max(su.align);
        let size = su.size;

        // 1. Random read, QD1.
        ctx.set_phase(
            lang.pick("Lettura casuale 4 KiB (QD1)", "4 KiB random read (QD1)"),
            0.4,
        );
        let qd1 = random_reads(
            ctx,
            su.file.file(),
            &su.file.path,
            size,
            block,
            su.align,
            phase,
            su.nonce ^ 1,
            Some((0.4, 0.6)),
        )?;
        ctx.set_interim(vec![metric(
            "rand_read_qd1_iops",
            lang.pick("Lettura casuale QD1", "Random read QD1"),
            qd1.ops as f64 / qd1.elapsed.as_secs_f64(),
            Unit::OperationsPerSecond,
            true,
        )]);

        // 2. Random read, 4 threads (each its own handle, QD1 per thread).
        ctx.set_phase(
            lang.pick(
                "Lettura casuale 4 KiB (4 thread)",
                "4 KiB random read (4 threads)",
            ),
            0.6,
        );
        let mut readers = Vec::with_capacity(RANDOM_THREADS);
        for _ in 0..RANDOM_THREADS {
            readers.push(su.file.open_reader()?);
        }
        let path = su.file.path.clone();
        let align = su.align;
        let nonce = su.nonce;
        let multi: Vec<RandomStats> =
            std::thread::scope(|s| -> Result<Vec<RandomStats>, RunError> {
                let mut handles = Vec::with_capacity(RANDOM_THREADS);
                for (i, r) in readers.iter().enumerate() {
                    let path = &path;
                    let progress = (i == 0).then_some((0.6, 0.8));
                    let h = ctx.spawn_worker(s, &format!("bench-io-{i}"), move || {
                        random_reads(
                            ctx,
                            r,
                            path,
                            size,
                            block,
                            align,
                            phase,
                            nonce ^ (0x100 + i as u64),
                            progress,
                        )
                    });
                    match h {
                        Ok(h) => handles.push(h),
                        Err(e) => {
                            // Started readers stop on their own after `phase`.
                            join_all(handles);
                            return Err(e.into());
                        }
                    }
                }
                join_all(handles).into_iter().collect()
            })?;
        drop(readers);
        let multi_ops: u64 = multi.iter().map(|s| s.ops).sum();
        let multi_elapsed = multi.iter().map(|s| s.elapsed).max().unwrap_or_default();

        // 3. Random write, QD1, bounded by time and by RANDOM_WRITE_CAP_BYTES.
        ctx.set_phase(
            lang.pick("Scrittura casuale 4 KiB (QD1)", "4 KiB random write (QD1)"),
            0.8,
        );
        let slots = size / block as u64;
        let mut seed = su.nonce ^ 2;
        let mut w = RandomStats::default();
        let mut wbuf = AlignedBuf::new(block, su.align)?;
        let t0 = Instant::now();
        let mut written_random = 0u64;
        while t0.elapsed() < phase && written_random + block as u64 <= RANDOM_WRITE_CAP_BYTES {
            ctx.check()?;
            let slot = xorshift(&mut seed) % slots;
            let src = &su.pool[(slot as usize) % POOL_BLOCKS];
            let start = ((slot as usize) * block) % (SEQ_BLOCK - block + 1) / su.align * su.align;
            wbuf.as_mut_slice()
                .copy_from_slice(&src.as_slice()[start..start + block]);
            stamp(wbuf.as_mut_slice(), su.nonce, slot);
            budget.charge(block as u64)?;
            let s = Instant::now();
            write_all_at(
                su.file.file(),
                wbuf.as_slice(),
                slot * block as u64,
                &su.file.path,
            )?;
            w.latency_sum += s.elapsed();
            w.ops += 1;
            written_random += block as u64;
            if w.ops.is_multiple_of(64) {
                ctx.set_fraction(
                    0.8 + 0.2 * (t0.elapsed().as_secs_f64() / phase.as_secs_f64()).min(1.0),
                );
            }
        }
        // Inside the timed window (no-op on Windows, where every write is write-through).
        flush_to_device(su.file.file(), &su.file.path)?;
        w.elapsed = t0.elapsed();
        let written = budget.used();
        drop(su);

        let bs = block as f64;
        let iops = |st: &RandomStats| st.ops as f64 / st.elapsed.as_secs_f64().max(1e-9);
        let lat_us = |st: &RandomStats| st.latency_sum.as_secs_f64() * 1e6 / st.ops.max(1) as f64;
        if qd1.ops == 0 || multi_ops == 0 || w.ops == 0 {
            return Err(RunError::Failed(AppError::internal(
                "a random I/O phase completed no operation",
            )));
        }
        let multi_iops = multi_ops as f64 / multi_elapsed.as_secs_f64().max(1e-9);
        let mut out = Outcome::default();
        out.metrics.extend([
            metric(
                "rand_read_qd1_iops",
                lang.pick(
                    "Lettura casuale 4 KiB QD1 (IOPS)",
                    "4 KiB random read QD1 (IOPS)",
                ),
                iops(&qd1),
                Unit::OperationsPerSecond,
                true,
            ),
            metric(
                "rand_read_qd1",
                lang.pick("Lettura casuale 4 KiB QD1", "4 KiB random read QD1"),
                iops(&qd1) * bs,
                Unit::BytesPerSecond,
                true,
            ),
            metric(
                "rand_read_qd1_latency",
                lang.pick(
                    "Latenza media lettura casuale QD1",
                    "Average random read latency QD1",
                ),
                lat_us(&qd1),
                Unit::Microseconds,
                false,
            ),
            metric(
                "rand_read_t4_iops",
                lang.pick(
                    "Lettura casuale 4 KiB, 4 thread (IOPS)",
                    "4 KiB random read, 4 threads (IOPS)",
                ),
                multi_iops,
                Unit::OperationsPerSecond,
                true,
            ),
            metric(
                "rand_read_t4",
                lang.pick(
                    "Lettura casuale 4 KiB, 4 thread",
                    "4 KiB random read, 4 threads",
                ),
                multi_iops * bs,
                Unit::BytesPerSecond,
                true,
            ),
            metric(
                "rand_write_qd1_iops",
                lang.pick(
                    "Scrittura casuale 4 KiB QD1 (IOPS)",
                    "4 KiB random write QD1 (IOPS)",
                ),
                iops(&w),
                Unit::OperationsPerSecond,
                true,
            ),
            metric(
                "rand_write_qd1",
                lang.pick("Scrittura casuale 4 KiB QD1", "4 KiB random write QD1"),
                iops(&w) * bs,
                Unit::BytesPerSecond,
                true,
            ),
            metric(
                "rand_write_qd1_latency",
                lang.pick(
                    "Latenza media scrittura casuale QD1",
                    "Average random write latency QD1",
                ),
                lat_us(&w),
                Unit::Microseconds,
                false,
            ),
            metric(
                "bytes_written",
                bytes_written_text(lang),
                written as f64,
                Unit::Bytes,
                false,
            ),
        ]);
        if block != RANDOM_BLOCK {
            out.warnings.push(lang.pick(
                format!(
                    "Dimensione dei blocchi casuali aumentata a {block} byte per allinearsi ai settori del volume."
                ),
                format!(
                    "Random block size raised to {block} bytes to match the volume's sectors."
                ),
            ));
        }
        Ok(out)
    }
}

#[cfg(any(windows, target_os = "macos"))]
pub(crate) use imp::{run_random, run_sequential};

#[cfg(not(any(windows, target_os = "macos")))]
pub(crate) fn run_sequential(
    _ctx: &RunCtx,
    _plan: &crate::validate::Plan,
    _run_id: &str,
) -> Result<Outcome, RunError> {
    Err(RunError::Failed(AppError::new(
        ErrorCode::NotSupported,
        "storage benchmarks are implemented for Windows and macOS only",
    )))
}

#[cfg(not(any(windows, target_os = "macos")))]
pub(crate) fn run_random(
    ctx: &RunCtx,
    plan: &crate::validate::Plan,
    run_id: &str,
) -> Result<Outcome, RunError> {
    run_sequential(ctx, plan, run_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_name_pattern_is_exact() {
        let id = ids::new_run_id();
        let name = bench_file_name(&id);
        assert!(is_bench_file_name(&name), "{name}");
        for bad in [
            "PCDiagnosticsBench-.tmp",
            "PCDiagnosticsBench-notanid.tmp",
            &format!("PCDiagnosticsBench-{id}.tmp.bak"),
            &format!("pcdiagnosticsbench-{id}.tmp"),
            &format!("xPCDiagnosticsBench-{id}.tmp"),
            &format!("PCDiagnosticsBench-{id}.TMP"),
            &format!("PCDiagnosticsBench-{id}"),
            "document.docx",
        ] {
            assert!(!is_bench_file_name(bad), "{bad}");
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn test_dir_selection() {
        let temp = PathBuf::from("/var/folders/xy/abc/T");
        let on = |d: &Path, v: &str| (d.to_path_buf(), Some(v.to_owned()));
        let root = Some("/");
        // `$TMPDIR` on the system volume group: used for `/`.
        assert_eq!(test_dir_for("/", root, &[on(&temp, "/")]), temp);
        // Any other volume: its own directory, never `$TMPDIR`.
        assert_eq!(
            test_dir_for("/Volumes/Work", root, &[on(&temp, "/")]),
            Path::new("/Volumes/Work/PCDiagnosticsBench")
        );
        // `$TMPDIR` redirected to another volume: the OS per-user directory instead.
        let redirected = PathBuf::from("/Volumes/Work/tmp");
        assert_eq!(
            test_dir_for(
                "/",
                root,
                &[on(&redirected, "/Volumes/Work"), on(&temp, "/")]
            ),
            temp
        );
        // A temporary directory on a non-system volume is not used for that volume.
        assert_eq!(
            test_dir_for("/Volumes/Work", root, &[on(&redirected, "/Volumes/Work")]),
            Path::new("/Volumes/Work/PCDiagnosticsBench")
        );
        // No candidate on `/`: the (read-only) `/PCDiagnosticsBench`, which fails cleanly.
        assert_eq!(
            test_dir_for("/", root, &[(temp.clone(), None)]),
            Path::new("/PCDiagnosticsBench")
        );
        // On this Mac `$TMPDIR` is on the system volume group.
        let tmp = std::env::temp_dir();
        assert_eq!(test_dir("/", root, &tmp), tmp);
        assert!(sweep_dirs().contains(&tmp));
    }

    /// The test file has no name while it is open (unlinked right after creation, the
    /// macOS equivalent of `FILE_FLAG_DELETE_ON_CLOSE`), bypasses the cache, and leaves
    /// nothing behind, including its own `PCDiagnosticsBench` directory.
    #[cfg(target_os = "macos")]
    #[test]
    fn bench_file_is_unlinked_while_open_and_cleaned_up() {
        use std::os::unix::fs::MetadataExt;
        let base = std::env::temp_dir().join(format!("bench-file-test-{}", ids::new_run_id()));
        std::fs::create_dir(&base).expect("mkdir");
        let dir = base.join(DIR_NAME);
        let id = ids::new_run_id();
        let f = imp::BenchFile::create(&dir, &id, 8 * MIB).expect("create");
        assert!(!f.path.exists(), "no name while open");
        let meta = f.file().metadata().expect("fstat");
        assert_eq!(meta.nlink(), 0, "unlinked");
        assert_eq!(meta.len(), 8 * MIB);
        // Aligned unbuffered I/O works on the open descriptor and on a duplicate.
        let align = 4096;
        let mut w = AlignedBuf::new(align, align).expect("buf");
        let mut seed = 7;
        fill_random(w.as_mut_slice(), &mut seed);
        imp::write_all_at(f.file(), w.as_slice(), 4096, &f.path).expect("write");
        imp::flush_to_device(f.file(), &f.path).expect("F_FULLFSYNC");
        let reader = f.open_reader().expect("reader");
        let mut r = AlignedBuf::new(align, align).expect("buf");
        imp::read_exact_at(&reader, r.as_mut_slice(), 4096, &f.path).expect("read");
        assert_eq!(r.as_slice(), w.as_slice());
        drop(reader);
        drop(f);
        assert!(!dir.exists(), "own directory removed when empty");
        std::fs::remove_dir(&base).expect("cleanup");
    }

    #[cfg(windows)]
    #[test]
    fn test_dir_selection() {
        let temp = Path::new("C:\\Users\\u\\AppData\\Local\\Temp");
        assert_eq!(test_dir("C:\\", Some("C:\\"), temp), temp);
        assert_eq!(
            test_dir("D:\\", Some("C:\\"), temp),
            Path::new("D:\\PCDiagnosticsBench")
        );
        // %TEMP% redirected to another volume: do not write to it for a C: test.
        let other = Path::new("E:\\Temp");
        assert_eq!(
            test_dir("C:\\", Some("C:\\"), other),
            Path::new("C:\\PCDiagnosticsBench")
        );
    }

    #[test]
    fn sweep_removes_only_matching_files() {
        let dir = std::env::temp_dir().join(format!("bench-sweep-test-{}", ids::new_run_id()));
        std::fs::create_dir_all(&dir).expect("mkdir");
        let stale = dir.join(bench_file_name(&ids::new_run_id()));
        let keep = [
            dir.join("PCDiagnosticsBench-user.tmp"),
            dir.join("notes.txt"),
            dir.join(format!("{}.bak", bench_file_name(&ids::new_run_id()))),
        ];
        std::fs::write(&stale, b"x").expect("write");
        for k in &keep {
            std::fs::write(k, b"x").expect("write");
        }
        // A matching *directory* is never removed.
        let matching_dir = dir.join(bench_file_name(&ids::new_run_id()));
        std::fs::create_dir(&matching_dir).expect("mkdir");
        assert_eq!(sweep_stale(std::slice::from_ref(&dir)), 1);
        assert!(!stale.exists());
        assert!(keep.iter().all(|k| k.exists()));
        assert!(matching_dir.exists());
        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn budget_is_enforced() {
        let mut b = WriteBudget::new(10);
        b.charge(6).expect("ok");
        b.charge(4).expect("ok");
        assert!(b.charge(1).is_err());
        assert_eq!(b.used(), 10);
    }

    #[test]
    fn aligned_buffer_is_aligned() {
        for align in [512, 4096, 65536] {
            let mut b = AlignedBuf::new(8192, align).expect("buf");
            assert_eq!(b.as_slice().as_ptr() as usize % align, 0);
            assert_eq!(b.as_mut_slice().len(), 8192);
        }
    }

    #[test]
    fn stamps_roundtrip_and_random_data_is_not_trivially_compressible() {
        let mut buf = vec![0u8; 4096];
        let mut seed = 42u64;
        fill_random(&mut buf, &mut seed);
        stamp(&mut buf, 7, 9);
        assert_eq!(read_stamp(&buf), (7, 9));
        let distinct: std::collections::HashSet<u8> = buf.iter().copied().collect();
        assert!(distinct.len() > 200, "byte values look random");
    }
}
