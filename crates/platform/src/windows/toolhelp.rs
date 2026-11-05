//! Process tree snapshot via Toolhelp32. Opens no per-process handles.

use ::windows::Win32::Foundation::{CloseHandle, HANDLE};
use ::windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
};
use models::{AppError, ErrorCode};

struct Snapshot(HANDLE);

impl Drop for Snapshot {
    fn drop(&mut self) {
        // SAFETY: handle came from CreateToolhelp32Snapshot and is closed exactly once.
        let _ = unsafe { CloseHandle(self.0) };
    }
}

/// `(pid, parent pid)` for every running process.
pub fn process_parents() -> Result<Vec<(u32, u32)>, AppError> {
    // SAFETY: plain snapshot call; the handle is owned by `Snapshot`.
    let snap = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }.map_err(|e| {
        AppError::new(ErrorCode::Platform, "CreateToolhelp32Snapshot failed")
            .with_detail(e.to_string())
    })?;
    let snap = Snapshot(snap);
    let mut entry = PROCESSENTRY32W {
        dwSize: u32::try_from(std::mem::size_of::<PROCESSENTRY32W>()).unwrap_or(0),
        ..Default::default()
    };
    let mut out = Vec::with_capacity(512);
    // SAFETY: valid snapshot handle; `entry.dwSize` is initialized as required.
    let mut ok = unsafe { Process32FirstW(snap.0, &mut entry) }.is_ok();
    while ok {
        out.push((entry.th32ProcessID, entry.th32ParentProcessID));
        // SAFETY: as above.
        ok = unsafe { Process32NextW(snap.0, &mut entry) }.is_ok();
    }
    Ok(out)
}
