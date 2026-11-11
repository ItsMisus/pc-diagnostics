//! Process categories (rules: docs/impact-score.md, "Categories").
//!
//! Categories describe *what kind* of process it is, for grouping and wording in the UI.
//! They never imply that a process can or should be stopped.

use models::process::ProcessCategory;

/// Kernel pseudo-processes: no Win32 image, children of the System process (pid 4).
#[cfg(not(target_os = "macos"))]
const KERNEL_PSEUDO: &[&str] = &["Registry", "Memory Compression", "Secure System"];

/// Core OS executables. Matched by name *and* location (`%SystemRoot%\System32`) when
/// the path is readable, so a look-alike name elsewhere is not classified as System.
#[cfg(not(target_os = "macos"))]
const CORE_OS_IMAGES: &[&str] = &[
    "smss.exe",
    "csrss.exe",
    "wininit.exe",
    "winlogon.exe",
    "services.exe",
    "lsass.exe",
    "lsaiso.exe",
    "fontdrvhost.exe",
    "dwm.exe",
];

/// Windows: the System process. macOS: `launchd` (pid 0 is `kernel_task`).
#[cfg(not(target_os = "macos"))]
pub const SYSTEM_PID: u32 = 4;
#[cfg(target_os = "macos")]
pub const SYSTEM_PID: u32 = 1;

/// macOS core executables, matched by name and by a path under the sealed system
/// volume (`/System/Library/` or `/sbin/`) when the path is readable.
#[cfg(target_os = "macos")]
const CORE_OS_IMAGES: &[&str] = &["WindowServer", "loginwindow", "kernelmanagerd", "logd"];

/// Facts the classifier needs about one process.
#[derive(Debug, Clone, Copy)]
pub struct CategoryInput<'a> {
    pub pid: u32,
    pub parent_pid: u32,
    pub name: &'a str,
    /// Windows session. On macOS: 0 for system accounts, 1 for user accounts.
    pub session_id: u32,
    /// `None` when the path could not be read (access denied or no Win32 image).
    pub exe_path: Option<&'a str>,
    /// `None` when window ownership could not be determined this tick.
    pub has_visible_window: Option<bool>,
}

/// `system_dir` is `%SystemRoot%\System32`, lower-cased, without trailing separator
/// (unused on macOS).
pub fn categorize(p: &CategoryInput<'_>, system_dir: Option<&str>) -> ProcessCategory {
    if is_os_core(p, system_dir) {
        return ProcessCategory::System;
    }
    if p.name.is_empty() || p.session_id == u32::MAX {
        return ProcessCategory::Unknown;
    }
    if p.session_id == 0 {
        return ProcessCategory::Service;
    }
    match p.has_visible_window {
        Some(true) => ProcessCategory::Application,
        Some(false) => ProcessCategory::Background,
        None => ProcessCategory::Unknown,
    }
}

#[cfg(not(target_os = "macos"))]
fn is_os_core(p: &CategoryInput<'_>, system_dir: Option<&str>) -> bool {
    if p.pid == SYSTEM_PID {
        return true;
    }
    if p.parent_pid == SYSTEM_PID
        && p.exe_path.is_none_or(|path| !path.contains('\\'))
        && KERNEL_PSEUDO.iter().any(|n| n.eq_ignore_ascii_case(p.name))
    {
        return true;
    }
    if CORE_OS_IMAGES
        .iter()
        .any(|n| n.eq_ignore_ascii_case(p.name))
    {
        return match (p.exe_path, system_dir) {
            (Some(path), Some(dir)) => path
                .to_ascii_lowercase()
                .strip_prefix(dir)
                .and_then(|rest| rest.strip_prefix('\\'))
                .is_some_and(|file| !file.contains('\\')),
            // Path unreadable (protected process) or system dir unknown: trust the name.
            _ => true,
        };
    }
    false
}

#[cfg(target_os = "macos")]
fn is_os_core(p: &CategoryInput<'_>, _system_dir: Option<&str>) -> bool {
    if p.pid == 0 || p.pid == SYSTEM_PID {
        return true;
    }
    if CORE_OS_IMAGES.contains(&p.name) {
        // Path unreadable (other user's process): trust the name.
        return p
            .exe_path
            .is_none_or(|path| path.starts_with("/System/Library/") || path.starts_with("/sbin/"));
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    const SYS: &str = r"c:\windows\system32";

    fn input(name: &str) -> CategoryInput<'_> {
        CategoryInput {
            pid: 1000,
            parent_pid: 900,
            name,
            session_id: 1,
            exe_path: None,
            has_visible_window: Some(false),
        }
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn kernel_and_core_os_are_system() {
        let mut p = input("System");
        p.pid = 4;
        p.session_id = 0;
        assert_eq!(categorize(&p, Some(SYS)), ProcessCategory::System);
        for n in ["Registry", "Memory Compression", "Secure System"] {
            let mut p = input(n);
            p.parent_pid = 4;
            p.session_id = 0;
            assert_eq!(categorize(&p, Some(SYS)), ProcessCategory::System, "{n}");
        }
        let mut p = input("csrss.exe");
        p.exe_path = Some(r"C:\Windows\System32\csrss.exe");
        assert_eq!(categorize(&p, Some(SYS)), ProcessCategory::System);
        // Protected process: path unreadable, the name is trusted.
        let mut p = input("lsass.exe");
        p.session_id = 0;
        assert_eq!(categorize(&p, Some(SYS)), ProcessCategory::System);
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn look_alikes_are_not_system() {
        let mut p = input("csrss.exe");
        p.exe_path = Some(r"C:\Users\x\AppData\Local\Temp\csrss.exe");
        assert_eq!(categorize(&p, Some(SYS)), ProcessCategory::Background);
        let mut p = input("svchost.exe");
        p.exe_path = Some(r"C:\Windows\System32\drivers\svchost.exe");
        p.session_id = 0;
        assert_eq!(categorize(&p, Some(SYS)), ProcessCategory::Service);
        let mut p = input("Registry");
        p.parent_pid = 1234;
        assert_eq!(categorize(&p, Some(SYS)), ProcessCategory::Background);
        let mut p = input("dwm.exe");
        p.exe_path = Some(r"C:\Windows\System32\sub\dwm.exe");
        assert_eq!(categorize(&p, Some(SYS)), ProcessCategory::Background);
    }

    #[test]
    fn session_and_window_rules() {
        let mut p = input("svchost.exe");
        p.session_id = 0;
        assert_eq!(categorize(&p, Some(SYS)), ProcessCategory::Service);
        let mut p = input("notepad.exe");
        p.has_visible_window = Some(true);
        assert_eq!(categorize(&p, Some(SYS)), ProcessCategory::Application);
        let p = input("helper.exe");
        assert_eq!(categorize(&p, Some(SYS)), ProcessCategory::Background);
        let mut p = input("helper.exe");
        p.has_visible_window = None;
        assert_eq!(categorize(&p, Some(SYS)), ProcessCategory::Unknown);
        let p = input("");
        assert_eq!(categorize(&p, Some(SYS)), ProcessCategory::Unknown);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_core_processes_are_system() {
        let mut p = input("kernel_task");
        p.pid = 0;
        assert_eq!(categorize(&p, None), ProcessCategory::System);
        let mut p = input("launchd");
        p.pid = 1;
        p.session_id = 0;
        assert_eq!(categorize(&p, None), ProcessCategory::System);
        let mut p = input("WindowServer");
        p.exe_path =
            Some("/System/Library/PrivateFrameworks/SkyLight.framework/Resources/WindowServer");
        assert_eq!(categorize(&p, None), ProcessCategory::System);
        let mut p = input("WindowServer");
        p.exe_path = Some("/Users/x/Downloads/WindowServer");
        assert_eq!(categorize(&p, None), ProcessCategory::Background);
    }
}
