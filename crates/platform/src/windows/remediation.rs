//! Win32 side of [`crate::remediation`]: shell launches with fixed targets, the Recycle
//! Bin, the user's temp/profile folders and the polite `WM_CLOSE` close of a program.
//!
//! Shell calls (`ShellExecute*`, `SHEmptyRecycleBinW`) run on a short-lived thread of
//! their own with an STA apartment, as the shell requires; the caller's COM state is never
//! touched. No process is ever terminated: `CloseProgram` only posts `WM_CLOSE`.

use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use std::path::PathBuf;
use std::time::Duration;

use ::windows::Win32::Foundation::{
    E_ABORT, E_ACCESSDENIED, ERROR_CANCELLED, ERROR_INVALID_PARAMETER, FILETIME, HWND, LPARAM,
    WAIT_OBJECT_0, WPARAM,
};
use ::windows::Win32::Storage::FileSystem::{GetDriveTypeW, GetTempPathW};
use ::windows::Win32::System::Com::{
    COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE, CoInitializeEx, CoTaskMemFree, CoUninitialize,
};
use ::windows::Win32::System::Threading::{
    GetProcessInformation, GetProcessTimes, IsProcessCritical, OpenProcess,
    PROCESS_PROTECTION_LEVEL_INFORMATION, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE,
    PROTECTION_LEVEL_NONE, ProcessProtectionLevelInfo, WaitForSingleObject,
};
use ::windows::Win32::System::WindowsProgramming::DRIVE_FIXED;
use ::windows::Win32::UI::Shell::{
    FOLDERID_Profile, KF_FLAG_DEFAULT, SEE_MASK_FLAG_NO_UI, SEE_MASK_NOASYNC, SHELLEXECUTEINFOW,
    SHEmptyRecycleBinW, SHGetKnownFolderPath, SHQUERYRBINFO, SHQueryRecycleBinW, ShellExecuteExW,
    ShellExecuteW,
};
use ::windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GW_OWNER, GetWindow, GetWindowThreadProcessId, IsWindowVisible, PostMessageW,
    SW_SHOWNORMAL, WM_CLOSE,
};
use ::windows::core::{BOOL, HRESULT, PCWSTR, w};
use models::settings::Language;
use models::{AppError, ErrorCode};

use super::process::{OwnedHandle, filetime_100ns, is_cloaked};
use crate::remediation::{SFC_PARAMETERS, check_close_target, gone_text};

/// `COPYENGINE_E_USER_CANCELLED`: the shell copy engine's "user cancelled".
const COPYENGINE_E_USER_CANCELLED: HRESULT = HRESULT(0x8027_0000_u32 as i32);

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Balances a successful `CoInitializeEx` on the shell worker thread.
struct ComGuard;

impl Drop for ComGuard {
    fn drop(&mut self) {
        // SAFETY: constructed only after CoInitializeEx succeeded on this same thread.
        unsafe { CoUninitialize() };
    }
}

/// Runs `f` on a fresh STA thread (the apartment shell APIs expect) and waits for it.
fn on_sta_thread<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Result<T, AppError> {
    std::thread::Builder::new()
        .name("shell-sta".into())
        .spawn(move || {
            // SAFETY: initializes COM on this new thread only; balanced by ComGuard.
            let hr =
                unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) };
            let _com = hr.is_ok().then_some(ComGuard);
            f()
        })
        .map_err(|e| {
            AppError::internal("could not start the shell thread").with_detail(e.to_string())
        })?
        .join()
        .map_err(|_| AppError::internal("the shell thread stopped unexpectedly"))
}

/// `%SystemRoot%\System32\<file>`: launches use full paths, never a search-path lookup.
pub fn system_file(file: &str) -> Result<String, AppError> {
    let dir = super::process::system_directory()
        .ok_or_else(|| AppError::new(ErrorCode::Platform, "Windows system folder not found"))?;
    Ok(format!("{}\\{file}", dir.trim_end_matches('\\')))
}

/// `drive` is a normalized `"C:"`. True for an existing fixed local volume.
pub fn is_fixed_drive(drive: &str) -> bool {
    let root = wide(&format!("{drive}\\"));
    // SAFETY: NUL-terminated string that outlives the call.
    unsafe { GetDriveTypeW(PCWSTR(root.as_ptr())) == DRIVE_FIXED }
}

/// `ShellExecuteW("open", file, params)`. `file` is always a fixed URI or System32 path.
pub fn shell_open(file: String, params: Option<String>) -> Result<(), AppError> {
    let shown = file.clone();
    let code = on_sta_thread(move || {
        let file_w = wide(&file);
        let params_w = params.as_deref().map(wide);
        let params_p = params_w
            .as_ref()
            .map_or(PCWSTR::null(), |p| PCWSTR(p.as_ptr()));
        // SAFETY: every string is NUL-terminated and outlives the call.
        let h = unsafe {
            ShellExecuteW(
                None,
                w!("open"),
                PCWSTR(file_w.as_ptr()),
                params_p,
                PCWSTR::null(),
                SW_SHOWNORMAL,
            )
        };
        h.0 as isize
    })?;
    // ShellExecuteW reports success as a value greater than 32.
    if code > 32 {
        Ok(())
    } else {
        Err(
            AppError::new(ErrorCode::Platform, format!("could not open {shown}"))
                .with_detail(format!("ShellExecuteW returned {code}")),
        )
    }
}

/// Starts `System32\cmd.exe /k sfc /scannow` elevated (Windows shows its UAC prompt).
pub fn start_system_file_check() -> Result<(), AppError> {
    let cmd = system_file("cmd.exe")?;
    let result = on_sta_thread(move || {
        let file_w = wide(&cmd);
        let params_w = wide(SFC_PARAMETERS);
        let mut info = SHELLEXECUTEINFOW {
            cbSize: u32::try_from(size_of::<SHELLEXECUTEINFOW>()).unwrap_or(0),
            fMask: SEE_MASK_NOASYNC | SEE_MASK_FLAG_NO_UI,
            lpVerb: w!("runas"),
            lpFile: PCWSTR(file_w.as_ptr()),
            lpParameters: PCWSTR(params_w.as_ptr()),
            nShow: SW_SHOWNORMAL.0,
            ..Default::default()
        };
        // SAFETY: `info` is fully initialized with `cbSize`; its strings outlive the call.
        unsafe { ShellExecuteExW(&mut info) }.map_err(|e| e.code())
    })?;
    match result {
        Ok(()) => Ok(()),
        Err(hr) if hr == HRESULT::from_win32(ERROR_CANCELLED.0) => Err(AppError::new(
            ErrorCode::PermissionDenied,
            "the administrator (UAC) request was cancelled",
        )),
        Err(hr) => Err(
            AppError::new(ErrorCode::Platform, "could not start System File Checker")
                .with_detail(format!("HRESULT 0x{:08X}", hr.0)),
        ),
    }
}

/// Recycle Bin totals across all drives.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RecycleBinInfo {
    pub items: u64,
    pub bytes: u64,
}

fn query_bin() -> Result<RecycleBinInfo, HRESULT> {
    let mut info = SHQUERYRBINFO {
        cbSize: u32::try_from(size_of::<SHQUERYRBINFO>()).unwrap_or(0),
        ..Default::default()
    };
    // SAFETY: `info` is a valid SHQUERYRBINFO with `cbSize` set; null root = all drives.
    unsafe { SHQueryRecycleBinW(PCWSTR::null(), &mut info) }.map_err(|e| e.code())?;
    Ok(RecycleBinInfo {
        items: u64::try_from(info.i64NumItems).unwrap_or(0),
        bytes: u64::try_from(info.i64Size).unwrap_or(0),
    })
}

/// Read-only Recycle Bin query.
pub fn query_recycle_bin() -> Result<RecycleBinInfo, AppError> {
    on_sta_thread(query_bin)?.map_err(|hr| {
        AppError::new(
            ErrorCode::Platform,
            "could not read the Recycle Bin contents",
        )
        .with_detail(format!("HRESULT 0x{:08X}", hr.0))
    })
}

/// What happened in [`empty_recycle_bin`].
#[derive(Debug, Clone, Copy)]
pub struct EmptyBinResult {
    pub before: RecycleBinInfo,
    /// `None`: the bin was already empty and nothing was called.
    pub after: Option<RecycleBinInfo>,
    /// The HRESULT of `SHEmptyRecycleBinW` when it reported failure (user cancel included).
    pub error: Option<HRESULT>,
}

/// `SHEmptyRecycleBinW(None, None, 0)`: Windows shows its own confirmation dialog.
/// Blocks until the user answers. Counts are taken before and after.
pub fn empty_recycle_bin() -> Result<EmptyBinResult, AppError> {
    let r = on_sta_thread(|| -> Result<EmptyBinResult, HRESULT> {
        let before = query_bin()?;
        if before.items == 0 {
            return Ok(EmptyBinResult {
                before,
                after: None,
                error: None,
            });
        }
        // SAFETY: plain call; no owner window, all drives, default flags (confirmation,
        // progress UI and sound as Windows shows them).
        let error = unsafe { SHEmptyRecycleBinW(None, PCWSTR::null(), 0) }
            .err()
            .map(|e| e.code());
        let after = query_bin().unwrap_or(before);
        Ok(EmptyBinResult {
            before,
            after: Some(after),
            error,
        })
    })?;
    r.map_err(|hr| {
        AppError::new(
            ErrorCode::Platform,
            "could not read the Recycle Bin contents",
        )
        .with_detail(format!("HRESULT 0x{:08X}", hr.0))
    })
}

/// HRESULTs the shell uses when the user declines.
pub fn is_cancel(hr: HRESULT) -> bool {
    hr == E_ABORT
        || hr == HRESULT::from_win32(ERROR_CANCELLED.0)
        || hr == COPYENGINE_E_USER_CANCELLED
}

/// The user's temp folder (`GetTempPathW`; for a non-SYSTEM process the same folder as
/// `GetTempPath2W`, without depending on that newer export).
pub fn user_temp_dir() -> Result<PathBuf, AppError> {
    let mut buf = vec![0u16; 32_768];
    // SAFETY: the slice carries its own length.
    let n = unsafe { GetTempPathW(Some(&mut buf)) } as usize;
    if n == 0 || n >= buf.len() {
        return Err(AppError::new(
            ErrorCode::Platform,
            "GetTempPathW returned no folder",
        ));
    }
    Ok(PathBuf::from(OsString::from_wide(&buf[..n])))
}

/// The user's profile folder (`FOLDERID_Profile`).
pub fn user_profile_dir() -> Result<PathBuf, AppError> {
    // SAFETY: valid known-folder id; the returned buffer is freed below with CoTaskMemFree.
    let p =
        unsafe { SHGetKnownFolderPath(&FOLDERID_Profile, KF_FLAG_DEFAULT, None) }.map_err(|e| {
            AppError::new(ErrorCode::Platform, "user profile folder not found")
                .with_detail(format!("HRESULT 0x{:08X}", e.code().0))
        })?;
    // SAFETY: SHGetKnownFolderPath returned a NUL-terminated string.
    let path = unsafe { p.as_wide() }.to_vec();
    // SAFETY: allocated by the shell with CoTaskMemAlloc, freed exactly once.
    unsafe { CoTaskMemFree(Some(p.0.cast_const().cast())) };
    Ok(PathBuf::from(OsString::from_wide(&path)))
}

/// A validated `CloseProgram` target: an open handle (pins the pid against reuse) and the
/// windows to ask.
pub struct CloseTarget {
    process: OwnedHandle,
    pub pid: u32,
    /// Image name as Windows reports it (e.g. `Notepad.exe`).
    pub name: String,
    windows: Vec<HWND>,
}

struct WindowSearch {
    pid: u32,
    found: Vec<HWND>,
}

unsafe extern "system" fn collect_pid_window(hwnd: HWND, lparam: LPARAM) -> BOOL {
    // SAFETY: `lparam` is the `&mut WindowSearch` passed by `top_level_windows`, which
    // outlives the synchronous EnumWindows call and is not aliased during it.
    let search = unsafe { &mut *(lparam.0 as *mut WindowSearch) };
    let mut pid = 0u32;
    // SAFETY: live window from EnumWindows; valid out pointer.
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    // SAFETY: live window handle.
    if pid == search.pid && unsafe { IsWindowVisible(hwnd) }.as_bool() && !is_cloaked(hwnd) {
        search.found.push(hwnd);
    }
    BOOL(1)
}

/// Visible, non-cloaked top-level windows of `pid`; unowned (main) windows if it has any,
/// so owned dialogs such as "Salvare le modifiche?" are not dismissed by the request.
fn top_level_windows(pid: u32) -> Vec<HWND> {
    let mut search = WindowSearch {
        pid,
        found: Vec::new(),
    };
    // SAFETY: the callback only dereferences `lparam` as `search`, which lives across the
    // synchronous call.
    if let Err(e) =
        unsafe { EnumWindows(Some(collect_pid_window), LPARAM((&raw mut search) as isize)) }
    {
        tracing::debug!(error = %e, "EnumWindows failed");
    }
    let unowned: Vec<HWND> = search
        .found
        .iter()
        .copied()
        // SAFETY: window handle from EnumWindows; GetWindow tolerates stale handles.
        .filter(|h| unsafe { GetWindow(*h, GW_OWNER) }.is_err())
        .collect();
    if unowned.is_empty() {
        search.found
    } else {
        unowned
    }
}

/// Applies every `CloseProgram` rule and opens the process (query + synchronize only).
/// `Err` carries the reason in `lang`.
pub fn prepare_close(pid: u32, name: &str, lang: Language) -> Result<CloseTarget, String> {
    let scan = crate::process::ProcessEnumerator::new()
        .scan()
        .map_err(|_| {
            lang.pick(
                "Impossibile leggere l'elenco dei processi",
                "Couldn't read the list of processes",
            )
            .to_string()
        })?;
    let target = check_close_target(&scan.processes, std::process::id(), pid, name, lang)?;
    let shown = target.name.clone();
    let gone = || gone_text(pid, lang);

    // SAFETY: plain call; the handle is owned (and closed) by OwnedHandle.
    let handle = match unsafe {
        OpenProcess(
            PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
            false,
            pid,
        )
    } {
        Ok(h) => OwnedHandle(h),
        Err(e) if e.code() == HRESULT::from_win32(ERROR_INVALID_PARAMETER.0) => {
            return Err(gone());
        }
        Err(e) if e.code() == E_ACCESSDENIED => {
            return Err(lang.pick(
                format!(
                    "«{shown}» non è accessibile con i permessi attuali (forse è in esecuzione come amministratore)"
                ),
                format!(
                    "“{shown}” can't be accessed with the current permissions (it may be running as administrator)"
                ),
            ));
        }
        Err(e) => {
            tracing::debug!(pid, error = %e, "OpenProcess failed");
            return Err(lang.pick(
                format!("«{shown}» non è accessibile"),
                format!("“{shown}” can't be accessed"),
            ));
        }
    };

    // The handle must be the very process from the snapshot (pid reuse guard).
    let (mut c, mut x, mut k, mut u) = (
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
        FILETIME::default(),
    );
    // SAFETY: valid handle with PROCESS_QUERY_LIMITED_INFORMATION; four valid out pointers.
    if unsafe { GetProcessTimes(handle.0, &mut c, &mut x, &mut k, &mut u) }.is_ok()
        && target.create_time_100ns != 0
        && filetime_100ns(c) != target.create_time_100ns
    {
        return Err(gone());
    }

    let mut critical = BOOL(0);
    // SAFETY: valid handle; valid out pointer.
    if unsafe { IsProcessCritical(handle.0, &mut critical) }.is_ok() && critical.as_bool() {
        return Err(lang.pick(
            format!("«{shown}» è un processo critico di Windows e non può essere chiuso"),
            format!("“{shown}” is a critical Windows process and can't be closed"),
        ));
    }
    let mut prot = PROCESS_PROTECTION_LEVEL_INFORMATION::default();
    // SAFETY: valid handle; the out buffer is the struct of the declared size.
    let prot_ok = unsafe {
        GetProcessInformation(
            handle.0,
            ProcessProtectionLevelInfo,
            (&raw mut prot).cast(),
            u32::try_from(size_of::<PROCESS_PROTECTION_LEVEL_INFORMATION>()).unwrap_or(0),
        )
    }
    .is_ok();
    if prot_ok && prot.ProtectionLevel != PROTECTION_LEVEL_NONE {
        return Err(lang.pick(
            format!("«{shown}» è un processo protetto di Windows e non può essere chiuso"),
            format!("“{shown}” is a protected Windows process and can't be closed"),
        ));
    }

    let windows = top_level_windows(pid);
    if windows.is_empty() {
        return Err(lang.pick(
            format!("«{shown}» non ha finestre da chiudere"),
            format!("“{shown}” has no windows to close"),
        ));
    }
    Ok(CloseTarget {
        process: handle,
        pid,
        name: shown,
        windows,
    })
}

/// Posts `WM_CLOSE` to the target's windows (like clicking their X) and waits up to
/// `timeout` for the process to exit. `Ok(true)` = exited. Never terminates anything.
pub fn request_close(target: &CloseTarget, timeout: Duration) -> Result<bool, AppError> {
    let mut posted = 0usize;
    let mut last_error = None;
    for hwnd in &target.windows {
        // SAFETY: plain message post; a stale handle just fails.
        match unsafe { PostMessageW(Some(*hwnd), WM_CLOSE, WPARAM(0), LPARAM(0)) } {
            Ok(()) => posted += 1,
            Err(e) => last_error = Some(e.code()),
        }
    }
    let wait = |ms: u32| {
        // SAFETY: valid handle opened with SYNCHRONIZE.
        unsafe { WaitForSingleObject(target.process.0, ms) == WAIT_OBJECT_0 }
    };
    if posted == 0 {
        if wait(0) {
            return Ok(true);
        }
        return Err(AppError::new(
            ErrorCode::PermissionDenied,
            format!("could not ask {} to close", target.name),
        )
        .with_detail(format!(
            "PostMessageW(WM_CLOSE) failed for pid {}: HRESULT 0x{:08X}",
            target.pid,
            last_error.map_or(0, |h| h.0)
        )));
    }
    let ms = u32::try_from(timeout.as_millis()).unwrap_or(u32::MAX - 1);
    Ok(wait(ms))
}
