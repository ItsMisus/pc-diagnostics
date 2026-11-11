//! Native implementations of the "Risolvi" assistant's whitelisted actions
//! ([`RemediationAction`]).
//!
//! The model's plan is data. Each action is implemented here with fixed parameters (fixed
//! `ms-settings:` URIs, fixed executables from `System32`, a fixed `sfc` command line);
//! nothing the model writes is ever passed to a shell. Consent is collected by the caller;
//! this module only checks that an action is safe *now* ([`validate`]) and performs it
//! ([`execute`], which validates again).
//!
//! - [`validate`]: `Err(reason)`, in the requested language, when the action must not run
//!   in the current state (drive missing, PID gone or protected, out-of-range age, …).
//! - [`execute`]: performs the action; returns a plain-language result message, or an
//!   [`ActionFailure`] carrying the message for the user and the technical error.
//!
//! `execute` blocks: `EmptyRecycleBin` waits for the user to answer Windows' confirmation
//! dialog, `CloseProgram` waits up to [`CLOSE_TIMEOUT`], `CleanTempFiles` walks the temp
//! folder. Call it from a blocking worker thread, never from a UI or async executor thread.
//!
//! Never done here: force-killing a process, touching the registry, drivers or BIOS,
//! reading file contents, deleting anything outside the user's temp folder.
//!
//! macOS: only two actions exist there, both low-risk.
//! - `OpenSettingsPage` opens the matching System Settings pane through a fixed
//!   `x-apple.systempreferences:` URL from a closed list (see `macos::remediation`);
//!   pages without a macOS pane are refused.
//! - `CleanTempFiles` cleans the per-user folder `confstr(_CS_DARWIN_USER_TEMP_DIR)`
//!   (`/private/var/folders/…/T`, owned by the user, not shared) with the same walk and
//!   limits as on Windows. Because macOS lets a program delete a file another program
//!   still has open, a file is deleted only if both its modification and its access time
//!   are older than the requested age (the same "not accessed" rule macOS applies when it
//!   cleans that folder itself).
//!
//! Device Manager, Disk Cleanup, emptying the Recycle Bin, closing a program and System
//! File Checker are Windows tools or Windows-only mechanisms: on macOS they are refused
//! with a plain-language reason (`validate`) and a `NotSupported` error (`execute`).

#[cfg(any(windows, test))]
use std::collections::HashMap;
#[cfg(any(windows, target_os = "macos", test))]
use std::fs;
#[cfg(any(windows, target_os = "macos", test))]
use std::io;
#[cfg(any(windows, target_os = "macos", test))]
use std::path::Path;
use std::time::Duration;
#[cfg(any(windows, target_os = "macos", test))]
use std::time::SystemTime;

use models::assistant::{RemediationAction, SettingsPage};
use models::settings::Language;
use models::{AppError, ErrorCode};

#[cfg(any(windows, test))]
use crate::process::RawProcess;

/// Smallest accepted `CleanTempFiles::older_than_days`.
pub const MIN_TEMP_AGE_DAYS: u32 = 1;
/// Largest accepted `CleanTempFiles::older_than_days`.
pub const MAX_TEMP_AGE_DAYS: u32 = 365;
/// How long `CloseProgram` waits for the program to exit after asking it to close.
pub const CLOSE_TIMEOUT: Duration = Duration::from_secs(10);
/// The only command line `RunSystemFileCheck` ever starts (as `System32\cmd.exe` params).
pub const SFC_PARAMETERS: &str = "/k sfc /scannow";

/// Directory depth limit for the temp-folder walk.
#[cfg(any(windows, target_os = "macos", test))]
const MAX_TEMP_DEPTH: u32 = 32;
/// Entry budget for one temp-folder walk (bounds the time a pathological tree can take).
#[cfg(any(windows, target_os = "macos", test))]
const MAX_TEMP_ENTRIES: u64 = 500_000;
#[cfg(windows)]
const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;

/// Processes `CloseProgram` never targets (lower-case image names without `.exe`):
/// kernel pseudo-processes, session/logon infrastructure, the shell and its hosts,
/// security software, audio and print services.
const PROTECTED_PROCESSES: &[&str] = &[
    "",
    "system",
    "idle",
    "system idle process",
    "secure system",
    "registry",
    "memcompression",
    "memory compression",
    "vmmem",
    "smss",
    "csrss",
    "wininit",
    "winlogon",
    "logonui",
    "lockapp",
    "userinit",
    "services",
    "lsass",
    "lsaiso",
    "svchost",
    "dwm",
    "explorer",
    "fontdrvhost",
    "sihost",
    "ctfmon",
    "conhost",
    "taskhostw",
    "dllhost",
    "runtimebroker",
    "applicationframehost",
    "searchhost",
    "searchindexer",
    "searchapp",
    "startmenuexperiencehost",
    "shellexperiencehost",
    "shellhost",
    "textinputhost",
    "widgets",
    "msmpeng",
    "mpdefendercoreservice",
    "nissrv",
    "securityhealthservice",
    "securityhealthsystray",
    "smartscreen",
    "audiodg",
    "spoolsv",
    "wudfhost",
    "dashost",
    "msedgewebview2",
];

/// The fixed `ms-settings:` URI of each page.
pub fn settings_uri(page: SettingsPage) -> &'static str {
    match page {
        SettingsPage::WindowsUpdate => "ms-settings:windowsupdate",
        SettingsPage::WindowsUpdateOptional => "ms-settings:windowsupdate-optionalupdates",
        SettingsPage::StorageSense => "ms-settings:storagesense",
        SettingsPage::Display => "ms-settings:display",
        SettingsPage::Sound => "ms-settings:sound",
        SettingsPage::Bluetooth => "ms-settings:bluetooth",
        SettingsPage::Network => "ms-settings:network-status",
        SettingsPage::Power => "ms-settings:powersleep",
        SettingsPage::Troubleshoot => "ms-settings:troubleshoot",
        SettingsPage::StartupApps => "ms-settings:startupapps",
        SettingsPage::InstalledApps => "ms-settings:appsfeatures",
        SettingsPage::Privacy => "ms-settings:privacy",
    }
}

/// Name of the page as Windows 11 titles it in `lang`.
pub fn settings_page_label(page: SettingsPage, lang: Language) -> &'static str {
    match page {
        SettingsPage::WindowsUpdate => "Windows Update",
        SettingsPage::WindowsUpdateOptional => {
            lang.pick("Aggiornamenti facoltativi", "Optional updates")
        }
        SettingsPage::StorageSense => lang.pick("Sensore memoria", "Storage Sense"),
        SettingsPage::Display => lang.pick("Schermo", "Display"),
        SettingsPage::Sound => lang.pick("Audio", "Sound"),
        SettingsPage::Bluetooth => lang.pick("Bluetooth e dispositivi", "Bluetooth & devices"),
        SettingsPage::Network => lang.pick("Rete e Internet", "Network & internet"),
        SettingsPage::Power => lang.pick("Alimentazione", "Power"),
        SettingsPage::Troubleshoot => lang.pick("Risoluzione dei problemi", "Troubleshoot"),
        SettingsPage::StartupApps => lang.pick("App di avvio", "Startup apps"),
        SettingsPage::InstalledApps => lang.pick("App installate", "Installed apps"),
        SettingsPage::Privacy => lang.pick("Privacy e sicurezza", "Privacy & security"),
    }
}

/// `"c"`, `"C:"`, `"c:\"`, `" C:/ "` → `"C:"`. Anything else (paths, UNC, `"CD:"`) → `None`.
/// A bare letter is not accepted: the model must name a drive (`"C:"`).
pub fn normalize_drive(input: &str) -> Option<String> {
    let mut chars = input.trim().chars();
    let letter = chars.next().filter(char::is_ascii_alphabetic)?;
    if chars.next() != Some(':') {
        return None;
    }
    matches!(chars.as_str(), "" | "\\" | "/").then(|| format!("{}:", letter.to_ascii_uppercase()))
}

/// Lower-case file name without directory and without a trailing `.exe`:
/// `"C:\\Windows\\Explorer.EXE"` → `"explorer"`.
pub fn process_base_name(name: &str) -> String {
    let file = name.rsplit(['\\', '/']).next().unwrap_or(name).trim();
    let lower = file.to_lowercase();
    match lower.strip_suffix(".exe") {
        Some(base) => base.to_owned(),
        None => lower,
    }
}

/// Whether `name` (image name or path) is on the `CloseProgram` deny-list.
pub fn is_protected_process_name(name: &str) -> bool {
    PROTECTED_PROCESSES.contains(&process_base_name(name).as_str())
}

/// Case-insensitive file-name comparison; `"notepad"` matches `"Notepad.exe"`.
#[cfg(any(windows, test))]
fn names_match(image: &str, requested: &str) -> bool {
    !requested.trim().is_empty() && process_base_name(image) == process_base_name(requested)
}

/// Size in the units File Explorer shows (powers of 1024), with the decimal separator of
/// `lang`: "1,2 GB" in Italian, "1.2 GB" in English.
pub fn format_bytes(bytes: u64, lang: Language) -> String {
    const UNITS: [&str; 4] = ["KB", "MB", "GB", "TB"];
    if bytes < 1024 {
        return match lang {
            Language::Italian => format!("{bytes} byte"),
            Language::English => count_en(bytes, "byte", "bytes"),
        };
    }
    #[allow(clippy::cast_precision_loss)] // display only
    let mut v = bytes as f64 / 1024.0;
    let mut unit = 0;
    while v >= 1024.0 && unit < UNITS.len() - 1 {
        v /= 1024.0;
        unit += 1;
    }
    let s = if v >= 100.0 {
        format!("{v:.0}")
    } else {
        format!("{v:.1}")
    };
    let s = s.strip_suffix(".0").unwrap_or(&s);
    let s = match lang {
        Language::Italian => s.replace('.', ","),
        Language::English => s.to_owned(),
    };
    format!("{s} {}", UNITS[unit])
}

/// "1 file" / "3 files". English only: the Italian texts use invariant nouns.
fn count_en(n: u64, one: &str, many: &str) -> String {
    if n == 1 {
        format!("1 {one}")
    } else {
        format!("{n} {many}")
    }
}

#[cfg(any(windows, target_os = "macos", test))]
fn check_days(days: u32, lang: Language) -> Result<(), String> {
    if (MIN_TEMP_AGE_DAYS..=MAX_TEMP_AGE_DAYS).contains(&days) {
        Ok(())
    } else {
        Err(lang.pick(
            format!(
                "Il numero di giorni deve essere compreso tra {MIN_TEMP_AGE_DAYS} e {MAX_TEMP_AGE_DAYS} (ricevuto {days})"
            ),
            format!(
                "The number of days must be between {MIN_TEMP_AGE_DAYS} and {MAX_TEMP_AGE_DAYS} (received {days})"
            ),
        ))
    }
}

#[cfg(any(windows, target_os = "macos", test))]
fn days_text(days: u32, lang: Language) -> String {
    match lang {
        Language::Italian if days == 1 => "1 giorno".into(),
        Language::Italian => format!("{days} giorni"),
        Language::English => count_en(u64::from(days), "day", "days"),
    }
}

/// The program named by `pid` has exited (or the pid was reused).
#[cfg(any(windows, test))]
pub(crate) fn gone_text(pid: u32, lang: Language) -> String {
    lang.pick(
        format!("Il programma indicato (PID {pid}) non è più in esecuzione"),
        format!("The program (PID {pid}) is no longer running"),
    )
}

/// True when `pid` descends from `ancestor` in the snapshot. A parent link is trusted only
/// if the parent was created before the child (otherwise the parent pid was reused).
#[cfg(any(windows, test))]
fn is_descendant_of(procs: &[RawProcess], pid: u32, ancestor: u32) -> bool {
    let by_pid: HashMap<u32, &RawProcess> = procs.iter().map(|p| (p.pid, p)).collect();
    let Some(anc) = by_pid.get(&ancestor) else {
        return false;
    };
    let Some(mut cur) = by_pid.get(&pid).copied() else {
        return false;
    };
    for _ in 0..64 {
        if cur.parent_pid == ancestor {
            return anc.create_time_100ns <= cur.create_time_100ns;
        }
        let Some(parent) = by_pid.get(&cur.parent_pid).copied() else {
            return false;
        };
        if parent.pid == cur.pid || parent.create_time_100ns > cur.create_time_100ns {
            return false;
        }
        cur = parent;
    }
    false
}

/// The snapshot-only part of the `CloseProgram` rules (everything except window ownership
/// and the handle-based critical/protected checks). Returns the target process.
#[cfg(any(windows, test))]
pub(crate) fn check_close_target<'a>(
    procs: &'a [RawProcess],
    self_pid: u32,
    pid: u32,
    name: &str,
    lang: Language,
) -> Result<&'a RawProcess, String> {
    if pid == 0 || pid == 4 {
        return Err(lang
            .pick(
                "È un processo di sistema protetto e non può essere chiuso",
                "This is a protected system process and can't be closed",
            )
            .into());
    }
    let Some(target) = procs.iter().find(|p| p.pid == pid) else {
        return Err(gone_text(pid, lang));
    };
    let shown = if target.name.is_empty() {
        format!("PID {pid}")
    } else {
        target.name.clone()
    };
    if !names_match(&target.name, name) {
        let asked = name.trim();
        return Err(lang.pick(
            format!(
                "Il PID {pid} appartiene a «{shown}», non a «{asked}»: il programma potrebbe essere già stato chiuso"
            ),
            format!(
                "PID {pid} belongs to “{shown}”, not “{asked}”: the program may already have been closed"
            ),
        ));
    }
    if pid == self_pid {
        return Err(lang
            .pick(
                "Non è possibile chiudere PC Diagnostics da sé stesso",
                "PC Diagnostics can't close itself",
            )
            .into());
    }
    if is_protected_process_name(&target.name) {
        return Err(lang.pick(
            format!("«{shown}» è un processo di Windows protetto e non può essere chiuso"),
            format!("“{shown}” is a protected Windows process and can't be closed"),
        ));
    }
    let me = procs.iter().find(|p| p.pid == self_pid);
    if is_descendant_of(procs, pid, self_pid)
        || me.is_some_and(|m| process_base_name(&m.name) == process_base_name(&target.name))
    {
        return Err(lang.pick(
            format!("«{shown}» fa parte di PC Diagnostics e non può essere chiuso da qui"),
            format!("“{shown}” is part of PC Diagnostics and can't be closed from here"),
        ));
    }
    let Some(me) = me else {
        return Err(lang
            .pick(
                "Impossibile verificare la sessione utente di PC Diagnostics",
                "Couldn't verify the user session of PC Diagnostics",
            )
            .into());
    };
    if target.session_id == 0 {
        return Err(lang.pick(
            format!("«{shown}» è un servizio di sistema e non può essere chiuso"),
            format!("“{shown}” is a system service and can't be closed"),
        ));
    }
    if target.session_id != me.session_id {
        return Err(lang.pick(
            format!("«{shown}» appartiene a un'altra sessione utente e non può essere chiuso"),
            format!("“{shown}” belongs to another user session and can't be closed"),
        ));
    }
    Ok(target)
}

/// Rejects a temp folder that is a drive root or not strictly inside the user's profile.
/// Both paths must already be canonical (`fs::canonicalize`). Windows rule; macOS has its
/// own (`macos::remediation::check_temp_root`) because its temp folder is not inside the
/// home folder.
#[cfg(any(windows, test))]
pub(crate) fn check_temp_root(temp: &Path, profile: &Path, lang: Language) -> Result<(), String> {
    if temp.parent().is_none() || profile.parent().is_none() {
        return Err(lang
            .pick(
                "La cartella dei file temporanei coincide con la radice di un'unità: pulizia rifiutata",
                "The temporary files folder is a drive root: cleanup refused",
            )
            .into());
    }
    if temp == profile || !temp.starts_with(profile) {
        return Err(lang
            .pick(
                "La cartella dei file temporanei non si trova dentro il profilo utente: pulizia rifiutata",
                "The temporary files folder is not inside the user profile: cleanup refused",
            )
            .into());
    }
    Ok(())
}

/// Counts of one temp-folder cleaning.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TempCleanReport {
    pub deleted_files: u64,
    pub freed_bytes: u64,
    /// Old enough but could not be deleted (in use, access denied).
    pub skipped_files: u64,
    /// Empty, old sub-folders removed after their files were deleted.
    pub removed_dirs: u64,
    /// Symlinks, junctions and other reparse points (cloud placeholders): never followed,
    /// never deleted.
    pub skipped_reparse: u64,
    /// The entry budget or depth limit stopped the walk early.
    pub truncated: bool,
}

#[cfg(any(windows, target_os = "macos", test))]
fn is_reparse(md: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        md.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 || md.file_type().is_symlink()
    }
    #[cfg(not(windows))]
    {
        md.file_type().is_symlink()
    }
}

#[cfg(any(windows, target_os = "macos", test))]
struct TempWalk<'a> {
    root: &'a Path,
    cutoff: SystemTime,
    seen: u64,
    report: TempCleanReport,
}

#[cfg(any(windows, target_os = "macos", test))]
impl TempWalk<'_> {
    /// Windows: last modified before the cutoff (a file in use cannot be deleted there, so
    /// the deletion itself skips it). Elsewhere a file still open by a program can be
    /// deleted, so it must also not have been read since the cutoff.
    fn is_old(&self, md: &fs::Metadata) -> bool {
        let modified_old = md.modified().is_ok_and(|t| t < self.cutoff);
        #[cfg(windows)]
        {
            modified_old
        }
        #[cfg(not(windows))]
        {
            modified_old && md.accessed().is_ok_and(|t| t < self.cutoff)
        }
    }

    fn walk(&mut self, dir: &Path, depth: u32) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            self.seen += 1;
            if self.seen > MAX_TEMP_ENTRIES {
                self.report.truncated = true;
                return;
            }
            let path = entry.path();
            // Never follows links: describes the entry itself.
            let Ok(md) = fs::symlink_metadata(&path) else {
                continue;
            };
            if is_reparse(&md) {
                self.report.skipped_reparse += 1;
                continue;
            }
            if md.is_dir() {
                if depth >= MAX_TEMP_DEPTH {
                    self.report.truncated = true;
                    continue;
                }
                // Re-checked right before descending: a folder swapped for a junction after
                // the metadata read must still resolve inside the temp folder.
                match fs::canonicalize(&path) {
                    Ok(c) if c.starts_with(self.root) && c != self.root => {}
                    _ => {
                        self.report.skipped_reparse += 1;
                        continue;
                    }
                }
                let was_old = self.is_old(&md);
                self.walk(&path, depth + 1);
                // `remove_dir` only removes an empty folder (and only the link, should the
                // entry have become one).
                if was_old && fs::remove_dir(&path).is_ok() {
                    self.report.removed_dirs += 1;
                }
            } else if md.is_file() && self.is_old(&md) {
                match fs::remove_file(&path) {
                    Ok(()) => {
                        self.report.deleted_files += 1;
                        self.report.freed_bytes = self.report.freed_bytes.saturating_add(md.len());
                    }
                    Err(_) => self.report.skipped_files += 1,
                }
            }
        }
    }
}

/// Deletes regular files last modified more than `older_than_days` days before `now`
/// under `root`, then removes sub-folders that were already old and are now empty. Never
/// follows or deletes reparse points; never removes `root` itself. `root` must be
/// canonical and must not itself be a reparse point.
#[cfg(any(windows, target_os = "macos", test))]
pub(crate) fn clean_temp_dir(
    root: &Path,
    older_than_days: u32,
    now: SystemTime,
) -> io::Result<TempCleanReport> {
    let md = fs::symlink_metadata(root)?;
    if !md.is_dir() || is_reparse(&md) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "temp root is not a plain directory",
        ));
    }
    let age = Duration::from_secs(u64::from(older_than_days) * 86_400);
    let cutoff = now.checked_sub(age).unwrap_or(SystemTime::UNIX_EPOCH);
    let mut w = TempWalk {
        root,
        cutoff,
        seen: 0,
        report: TempCleanReport::default(),
    };
    w.walk(root, 0);
    Ok(w.report)
}

/// "Eliminati N file, liberati X" / "Deleted N files, freed X" plus skipped/removed details.
#[cfg(any(windows, target_os = "macos", test))]
pub(crate) fn temp_clean_message(r: &TempCleanReport, days: u32, lang: Language) -> String {
    let age = days_text(days, lang);
    let freed = format_bytes(r.freed_bytes, lang);
    let mut msg = if r.deleted_files == 0 && r.skipped_files == 0 {
        lang.pick(
            format!("Nessun file temporaneo più vecchio di {age}: non c'era nulla da eliminare"),
            format!("No temporary files older than {age}: there was nothing to delete"),
        )
    } else {
        lang.pick(
            format!("Eliminati {} file, liberati {freed}", r.deleted_files),
            format!(
                "Deleted {}, freed {freed}",
                count_en(r.deleted_files, "file", "files")
            ),
        )
    };
    if r.removed_dirs > 0 {
        msg.push_str(&lang.pick(
            format!("; rimosse {} cartelle vuote", r.removed_dirs),
            format!(
                "; removed {}",
                count_en(r.removed_dirs, "empty folder", "empty folders")
            ),
        ));
    }
    if r.skipped_files > 0 {
        msg.push_str(&lang.pick(
            format!(
                "; {} file saltati perché in uso o protetti",
                r.skipped_files
            ),
            format!(
                "; skipped {} in use or protected",
                count_en(r.skipped_files, "file", "files")
            ),
        ));
    }
    if r.truncated {
        msg.push_str(lang.pick(
            "; la cartella è molto grande: puoi ripetere l'operazione per continuare",
            "; the folder is very large: run the action again to continue",
        ));
    }
    msg
}

/// Recycle Bin contents across all drives as `(items, bytes)`. Read-only (lets the UI show
/// what `EmptyRecycleBin` would remove).
pub fn recycle_bin_contents() -> Result<(u64, u64), AppError> {
    #[cfg(windows)]
    {
        crate::windows::remediation::query_recycle_bin().map(|i| (i.items, i.bytes))
    }
    #[cfg(not(windows))]
    {
        Err(AppError::new(
            ErrorCode::NotSupported,
            "the Recycle Bin is only available on Windows",
        ))
    }
}

/// Why an action did not complete.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionFailure {
    /// Plain-language outcome for the user, in the requested language.
    pub message: String,
    /// Technical cause (English), for logs.
    pub error: AppError,
}

impl ActionFailure {
    fn new(error: AppError, message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            error,
        }
    }

    /// Refused by the safety checks; `reason` is already in the user's language.
    #[cfg(any(windows, target_os = "macos"))]
    fn refused(reason: String) -> Self {
        Self::new(
            AppError::invalid_input("the action was refused by the safety checks"),
            reason,
        )
    }
}

/// Checks that `action` may run now. `Err` carries the reason in `lang`, for the UI.
pub fn validate(action: &RemediationAction, lang: Language) -> Result<(), String> {
    #[cfg(windows)]
    {
        sys::validate(action, lang)
    }
    #[cfg(target_os = "macos")]
    {
        mac::validate(action, lang)
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let _ = action;
        Err(lang
            .pick(
                "Le azioni di correzione sono disponibili solo su Windows",
                "Fix actions are only available on Windows",
            )
            .into())
    }
}

/// Validates `action` again (an [`ActionFailure`] with the reason if it must not run),
/// then performs it. `Ok` carries a plain-language result for the user, in `lang`.
/// Blocking; see the module docs.
pub fn execute(action: &RemediationAction, lang: Language) -> Result<String, ActionFailure> {
    #[cfg(windows)]
    {
        sys::execute(action, lang)
    }
    #[cfg(target_os = "macos")]
    {
        mac::execute(action, lang)
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let _ = action;
        Err(ActionFailure::new(
            AppError::new(
                ErrorCode::NotSupported,
                "fix actions are only available on Windows",
            ),
            lang.pick(
                "Le azioni di correzione sono disponibili solo su Windows",
                "Fix actions are only available on Windows",
            ),
        ))
    }
}

#[cfg(windows)]
mod sys {
    use super::*;
    use crate::windows::remediation as win;

    fn fixed_drive(drive: &str, lang: Language) -> Result<String, String> {
        let d = normalize_drive(drive).ok_or_else(|| {
            let shown = drive.trim();
            lang.pick(
                format!("Unità non valida: «{shown}»"),
                format!("Invalid drive: “{shown}”"),
            )
        })?;
        if win::is_fixed_drive(&d) {
            Ok(d)
        } else {
            Err(lang.pick(
                format!("L'unità {d} non esiste o non è un disco fisso locale"),
                format!("Drive {d} doesn't exist or isn't a local fixed disk"),
            ))
        }
    }

    /// The canonical user temp folder, checked to lie inside the user's profile.
    fn temp_root(lang: Language) -> Result<std::path::PathBuf, String> {
        let refuse = |_| {
            lang.pick(
                "Impossibile individuare la cartella dei file temporanei",
                "Couldn't find the temporary files folder",
            )
            .to_string()
        };
        let temp = fs::canonicalize(win::user_temp_dir().map_err(|e| refuse(e.message))?)
            .map_err(|e| refuse(e.to_string()))?;
        let profile = fs::canonicalize(win::user_profile_dir().map_err(|e| refuse(e.message))?)
            .map_err(|e| refuse(e.to_string()))?;
        check_temp_root(&temp, &profile, lang)?;
        Ok(temp)
    }

    pub fn validate(action: &RemediationAction, lang: Language) -> Result<(), String> {
        match action {
            RemediationAction::OpenDiskCleanup { drive } => fixed_drive(drive, lang).map(|_| ()),
            RemediationAction::CleanTempFiles { older_than_days } => {
                check_days(*older_than_days, lang)?;
                temp_root(lang).map(|_| ())
            }
            RemediationAction::CloseProgram { pid, name } => {
                win::prepare_close(*pid, name, lang).map(|_| ())
            }
            RemediationAction::OpenSettingsPage { .. }
            | RemediationAction::OpenDeviceManager
            | RemediationAction::EmptyRecycleBin
            | RemediationAction::RunSystemFileCheck => Ok(()),
        }
    }

    /// `System32\<file>`, or the failure the user sees.
    fn system_file(file: &str, lang: Language) -> Result<String, ActionFailure> {
        win::system_file(file).map_err(|e| {
            ActionFailure::new(
                e,
                lang.pick(
                    "Impossibile individuare la cartella di sistema di Windows",
                    "Couldn't find the Windows system folder",
                ),
            )
        })
    }

    fn open(file: String, params: Option<String>, lang: Language) -> Result<(), ActionFailure> {
        let shown = file.clone();
        win::shell_open(file, params).map_err(|e| {
            ActionFailure::new(
                e,
                lang.pick(
                    format!("Impossibile aprire «{shown}»"),
                    format!("Couldn't open “{shown}”"),
                ),
            )
        })
    }

    pub fn execute(action: &RemediationAction, lang: Language) -> Result<String, ActionFailure> {
        validate(action, lang).map_err(ActionFailure::refused)?;
        match action {
            RemediationAction::OpenSettingsPage { page } => {
                open(settings_uri(*page).to_owned(), None, lang)?;
                let label = settings_page_label(*page, lang);
                Ok(lang.pick(
                    format!("Aperta la pagina Impostazioni: {label}"),
                    format!("Opened the Settings page: {label}"),
                ))
            }
            RemediationAction::OpenDeviceManager => {
                open(system_file("devmgmt.msc", lang)?, None, lang)?;
                Ok(lang
                    .pick("Aperta Gestione dispositivi", "Opened Device Manager")
                    .into())
            }
            RemediationAction::OpenDiskCleanup { drive } => {
                let d = fixed_drive(drive, lang).map_err(ActionFailure::refused)?;
                open(
                    system_file("cleanmgr.exe", lang)?,
                    Some(format!("/d {d}")),
                    lang,
                )?;
                Ok(lang.pick(
                    format!(
                        "Aperta Pulizia disco per l'unità {d} scegli nella sua finestra cosa eliminare"
                    ),
                    format!(
                        "Opened Disk Cleanup: choose in its window what to delete from drive {d}"
                    ),
                ))
            }
            RemediationAction::EmptyRecycleBin => empty_recycle_bin(lang),
            RemediationAction::CleanTempFiles { older_than_days } => {
                let root = temp_root(lang).map_err(ActionFailure::refused)?;
                let report =
                    clean_temp_dir(&root, *older_than_days, SystemTime::now()).map_err(|e| {
                        ActionFailure::new(
                            AppError::new(
                                ErrorCode::Platform,
                                "could not clean the temporary files folder",
                            )
                            .with_detail(e.to_string()),
                            lang.pick(
                                "Impossibile pulire la cartella dei file temporanei",
                                "Couldn't clean the temporary files folder",
                            ),
                        )
                    })?;
                Ok(temp_clean_message(&report, *older_than_days, lang))
            }
            RemediationAction::CloseProgram { pid, name } => {
                let target =
                    win::prepare_close(*pid, name, lang).map_err(ActionFailure::refused)?;
                let shown = target.name.clone();
                let closed = win::request_close(&target, CLOSE_TIMEOUT).map_err(|e| {
                    ActionFailure::new(
                        e,
                        lang.pick(
                            format!(
                                "Impossibile chiedere a «{shown}» di chiudersi (forse è in esecuzione come amministratore)"
                            ),
                            format!(
                                "Couldn't ask “{shown}” to close (it may be running as administrator)"
                            ),
                        ),
                    )
                })?;
                Ok(if closed {
                    lang.pick(
                        format!("«{shown}» si è chiuso"),
                        format!("“{shown}” closed"),
                    )
                } else {
                    lang.pick(
                        format!(
                            "Chiusura richiesta, ma «{shown}» è ancora in esecuzione: il programma potrebbe chiederti di salvare, controlla la sua finestra"
                        ),
                        format!(
                            "Close requested, but “{shown}” is still running: it may be asking you to save your work, check its window"
                        ),
                    )
                })
            }
            RemediationAction::RunSystemFileCheck => {
                win::start_system_file_check().map_err(|e| {
                    let message = if e.code == ErrorCode::PermissionDenied {
                        lang.pick(
                            "Richiesta di amministratore annullata",
                            "Administrator request cancelled",
                        )
                    } else {
                        lang.pick(
                            "Impossibile avviare il Controllo file di sistema",
                            "Couldn't start System File Checker",
                        )
                    };
                    ActionFailure::new(e, message)
                })?;
                Ok(lang
                    .pick(
                        "Controllo file di sistema avviato in una finestra separata (può richiedere 10–20 minuti)",
                        "System File Checker started in a separate window (it can take 10–20 minutes)",
                    )
                    .into())
            }
        }
    }

    fn empty_recycle_bin(lang: Language) -> Result<String, ActionFailure> {
        let r = win::empty_recycle_bin().map_err(|e| {
            ActionFailure::new(
                e,
                lang.pick(
                    "Impossibile leggere il contenuto del Cestino",
                    "Couldn't read the Recycle Bin contents",
                ),
            )
        })?;
        let Some(after) = r.after else {
            return Ok(lang
                .pick(
                    "Il Cestino è già vuoto: non c'era nulla da eliminare",
                    "The Recycle Bin is already empty: there was nothing to delete",
                )
                .into());
        };
        let removed = r.before.items.saturating_sub(after.items);
        let freed = format_bytes(r.before.bytes.saturating_sub(after.bytes), lang);
        if removed == 0 {
            return Err(match r.error {
                Some(hr) if !win::is_cancel(hr) => ActionFailure::new(
                    AppError::new(ErrorCode::Platform, "could not empty the Recycle Bin")
                        .with_detail(format!("HRESULT 0x{:08X}", hr.0)),
                    lang.pick(
                        "Impossibile svuotare il Cestino",
                        "Couldn't empty the Recycle Bin",
                    ),
                ),
                _ => ActionFailure::new(
                    AppError::new(
                        ErrorCode::Cancelled,
                        "emptying the Recycle Bin was cancelled",
                    ),
                    lang.pick(
                        "Il Cestino non è stato svuotato: operazione annullata",
                        "The Recycle Bin was not emptied: the operation was cancelled",
                    ),
                ),
            });
        }
        let mut msg = lang.pick(
            format!("Cestino svuotato: eliminati {removed} elementi, liberati {freed}"),
            format!(
                "Recycle Bin emptied: deleted {}, freed {freed}",
                count_en(removed, "item", "items")
            ),
        );
        if after.items > 0 {
            msg.push_str(&lang.pick(
                format!(
                    "; restano {} elementi che Windows non ha potuto eliminare",
                    after.items
                ),
                format!(
                    "; {} could not be deleted by Windows",
                    count_en(after.items, "item", "items")
                ),
            ));
        }
        Ok(msg)
    }
}

#[cfg(target_os = "macos")]
mod mac {
    use super::*;
    use crate::macos::remediation as macos;

    /// Why `action` cannot run on macOS, in `lang`. `None` for the actions macOS supports.
    pub(super) fn unavailable_reason(action: &RemediationAction, lang: Language) -> Option<String> {
        let text = match action {
            RemediationAction::OpenSettingsPage { page } => {
                if macos::settings_url(*page).is_some() {
                    return None;
                }
                let label = settings_page_label(*page, lang);
                return Some(lang.pick(
                    format!(
                        "La pagina «{label}» delle Impostazioni di Windows non ha un equivalente in Impostazioni di Sistema di macOS"
                    ),
                    format!(
                        "The Windows Settings page “{label}” has no equivalent in macOS System Settings"
                    ),
                ));
            }
            RemediationAction::CleanTempFiles { .. } => return None,
            RemediationAction::OpenDeviceManager => lang.pick(
                "Gestione dispositivi è uno strumento di Windows e non esiste su macOS",
                "Device Manager is a Windows tool and doesn't exist on macOS",
            ),
            RemediationAction::OpenDiskCleanup { .. } => lang.pick(
                "Pulizia disco è uno strumento di Windows: su macOS lo spazio si gestisce in Impostazioni di Sistema > Generali > Spazio",
                "Disk Cleanup is a Windows tool: on macOS, storage is managed in System Settings > General > Storage",
            ),
            RemediationAction::EmptyRecycleBin => lang.pick(
                "Su macOS l'app non svuota il Cestino: puoi svuotarlo dal Finder (menu Finder > Svuota il Cestino)",
                "On macOS the app doesn't empty the Trash: you can empty it from the Finder (Finder menu > Empty Trash)",
            ),
            RemediationAction::CloseProgram { .. } => lang.pick(
                "Su macOS l'app non chiude i programmi: chiudilo dal suo menu con «Esci»",
                "On macOS the app doesn't close programs: quit it from its own menu with “Quit”",
            ),
            RemediationAction::RunSystemFileCheck => lang.pick(
                "Il Controllo file di sistema (sfc) esiste solo su Windows; macOS protegge e verifica da sé i propri file di sistema",
                "System File Checker (sfc) exists only on Windows; macOS protects and verifies its own system files",
            ),
        };
        Some(text.to_owned())
    }

    /// The canonical per-user temp folder, checked by `macos::check_temp_root`.
    fn temp_root(lang: Language) -> Result<std::path::PathBuf, String> {
        let refuse = |_| {
            lang.pick(
                "Impossibile individuare la cartella dei file temporanei",
                "Couldn't find the temporary files folder",
            )
            .to_string()
        };
        let raw = macos::user_temp_dir().map_err(|e| refuse(e.message))?;
        let temp = fs::canonicalize(raw).map_err(|e| refuse(e.to_string()))?;
        let facts = macos::TempRootFacts::read(&temp).map_err(|e| refuse(e.to_string()))?;
        macos::check_temp_root(&facts, macos::current_uid(), lang)?;
        Ok(temp)
    }

    pub fn validate(action: &RemediationAction, lang: Language) -> Result<(), String> {
        if let Some(reason) = unavailable_reason(action, lang) {
            return Err(reason);
        }
        match action {
            RemediationAction::CleanTempFiles { older_than_days } => {
                check_days(*older_than_days, lang)?;
                temp_root(lang).map(|_| ())
            }
            _ => Ok(()),
        }
    }

    pub fn execute(action: &RemediationAction, lang: Language) -> Result<String, ActionFailure> {
        if let Some(reason) = unavailable_reason(action, lang) {
            return Err(ActionFailure::new(
                AppError::new(
                    ErrorCode::NotSupported,
                    "this fix action is not available on macOS",
                ),
                reason,
            ));
        }
        validate(action, lang).map_err(ActionFailure::refused)?;
        match action {
            RemediationAction::OpenSettingsPage { page } => {
                let (Some(url), Some(label)) = (
                    macos::settings_url(*page),
                    macos::settings_label(*page, lang),
                ) else {
                    return Err(ActionFailure::new(
                        AppError::new(ErrorCode::NotSupported, "no macOS settings pane"),
                        lang.pick(
                            "Pagina non disponibile su macOS",
                            "Page not available on macOS",
                        ),
                    ));
                };
                macos::open_settings_url(url).map_err(|e| {
                    ActionFailure::new(
                        e,
                        lang.pick(
                            "Impossibile aprire Impostazioni di Sistema",
                            "Couldn't open System Settings",
                        ),
                    )
                })?;
                Ok(lang.pick(
                    format!("Aperta la sezione «{label}» di Impostazioni di Sistema"),
                    format!("Opened “{label}” in System Settings"),
                ))
            }
            RemediationAction::CleanTempFiles { older_than_days } => {
                let root = temp_root(lang).map_err(ActionFailure::refused)?;
                let report =
                    clean_temp_dir(&root, *older_than_days, SystemTime::now()).map_err(|e| {
                        ActionFailure::new(
                            AppError::new(
                                ErrorCode::Platform,
                                "could not clean the temporary files folder",
                            )
                            .with_detail(e.to_string()),
                            lang.pick(
                                "Impossibile pulire la cartella dei file temporanei",
                                "Couldn't clean the temporary files folder",
                            ),
                        )
                    })?;
                Ok(temp_clean_message(&report, *older_than_days, lang))
            }
            // Refused above by `unavailable_reason`.
            _ => Err(ActionFailure::new(
                AppError::new(
                    ErrorCode::NotSupported,
                    "this fix action is not available on macOS",
                ),
                lang.pick(
                    "Azione non disponibile su macOS",
                    "Action not available on macOS",
                ),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    const IT: Language = Language::Italian;
    const EN: Language = Language::English;

    fn proc(pid: u32, parent: u32, name: &str, session: u32, created: i64) -> RawProcess {
        RawProcess {
            pid,
            parent_pid: parent,
            name: name.into(),
            session_id: session,
            create_time_100ns: created,
            cpu_time_100ns: 0,
            working_set_bytes: 0,
            private_bytes: 0,
            io_read_bytes: 0,
            io_write_bytes: 0,
        }
    }

    #[test]
    fn drive_normalization() {
        for ok in ["C:", "c:", "C:\\", "c:/", "  d:  ", "Z:\\"] {
            let n = normalize_drive(ok).expect(ok);
            assert_eq!(n.len(), 2);
            assert!(n.ends_with(':') && n.chars().next().is_some_and(|c| c.is_ascii_uppercase()));
        }
        assert_eq!(normalize_drive("c:\\").as_deref(), Some("C:"));
        for bad in [
            "",
            "C",
            "CD:",
            "C:\\Windows",
            "\\\\server\\share",
            "1:",
            "C:\\\\",
            ":",
            "é:",
            "C;",
            "C: & calc",
        ] {
            assert_eq!(normalize_drive(bad), None, "{bad}");
        }
    }

    #[test]
    fn deny_list_and_names() {
        for p in [
            "System",
            "csrss.exe",
            "C:\\Windows\\explorer.exe",
            "LSASS.EXE",
            "svchost",
            "Registry",
            "MsMpEng.exe",
            "dwm.exe",
            "RuntimeBroker.exe",
            "msedgewebview2.exe",
            "",
        ] {
            assert!(is_protected_process_name(p), "{p}");
        }
        for p in ["notepad.exe", "chrome.exe", "Code.exe", "explorer2.exe"] {
            assert!(!is_protected_process_name(p), "{p}");
        }
        assert!(names_match("Notepad.exe", "notepad.exe"));
        assert!(names_match("Notepad.exe", "notepad"));
        assert!(names_match("notepad.exe", "C:\\Windows\\notepad.exe"));
        assert!(!names_match("notepad.exe", "notepad.com"));
        assert!(!names_match("notepad.exe", " "));
        assert!(!names_match("notepad.exe", "chrome.exe"));
    }

    #[test]
    fn settings_uris_are_fixed_and_distinct() {
        use SettingsPage::*;
        let all = [
            WindowsUpdate,
            WindowsUpdateOptional,
            StorageSense,
            Display,
            Sound,
            Bluetooth,
            Network,
            Power,
            Troubleshoot,
            StartupApps,
            InstalledApps,
            Privacy,
        ];
        let uris: std::collections::HashSet<_> = all.iter().map(|p| settings_uri(*p)).collect();
        assert_eq!(uris.len(), all.len());
        for p in all {
            let u = settings_uri(p);
            assert!(u.starts_with("ms-settings:"), "{u}");
            assert!(
                u["ms-settings:".len()..]
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c == '-'),
                "{u}"
            );
            assert!(!settings_page_label(p, IT).is_empty());
        }
        assert_eq!(settings_uri(Network), "ms-settings:network-status");
        assert_eq!(settings_uri(Power), "ms-settings:powersleep");
        assert_eq!(settings_uri(InstalledApps), "ms-settings:appsfeatures");
        assert_eq!(
            settings_uri(WindowsUpdateOptional),
            "ms-settings:windowsupdate-optionalupdates"
        );
    }

    #[test]
    fn bytes_formatting() {
        assert_eq!(format_bytes(0, IT), "0 byte");
        assert_eq!(format_bytes(1023, IT), "1023 byte");
        assert_eq!(format_bytes(1024, IT), "1 KB");
        assert_eq!(format_bytes(1536, IT), "1,5 KB");
        assert_eq!(format_bytes(1_288_490_189, IT), "1,2 GB");
        assert_eq!(format_bytes(250 * 1024 * 1024, IT), "250 MB");
    }

    #[test]
    fn day_bounds() {
        assert!(check_days(0, IT).is_err());
        assert!(check_days(1, IT).is_ok());
        assert!(check_days(365, IT).is_ok());
        assert!(check_days(366, IT).is_err());
        assert!(check_days(u32::MAX, IT).is_err());
    }

    #[test]
    fn close_target_rules() {
        let me = 1000;
        let procs = vec![
            proc(4, 0, "System", 0, 1),
            proc(600, 4, "csrss.exe", 1, 2),
            proc(700, 600, "explorer.exe", 1, 3),
            proc(me, 700, "pc-diagnostics.exe", 1, 10),
            proc(1100, me, "msedgewebview2.exe", 1, 11),
            proc(1200, 1100, "renderer.exe", 1, 12),
            proc(2000, 700, "notepad.exe", 1, 20),
            proc(2100, 700, "other.exe", 2, 21),
            proc(2200, 500, "svc.exe", 0, 22),
            // Parent pid reused by a newer `me`-unrelated chain: pid 1300's parent is `me`,
            // but it was created before `me`, so the link is stale.
            proc(1300, me, "old-child.exe", 1, 5),
            proc(2300, 700, "pc-diagnostics.exe", 1, 23),
        ];
        let ok = check_close_target(&procs, me, 2000, "Notepad", IT).expect("notepad ok");
        assert_eq!(ok.pid, 2000);
        assert!(check_close_target(&procs, me, 1300, "old-child.exe", IT).is_ok());

        let err = |pid, name| check_close_target(&procs, me, pid, name, IT).expect_err("denied");
        assert!(err(4, "System").contains("protetto"));
        assert!(err(0, "Idle").contains("protetto"));
        assert!(err(600, "csrss.exe").contains("protetto"));
        assert!(err(700, "explorer.exe").contains("protetto"));
        assert!(err(9999, "notepad.exe").contains("non è più in esecuzione"));
        assert!(err(2000, "chrome.exe").contains("appartiene a «notepad.exe»"));
        assert!(err(me, "pc-diagnostics.exe").contains("PC Diagnostics"));
        assert!(err(1100, "msedgewebview2.exe").contains("protetto"));
        assert!(err(1200, "renderer.exe").contains("PC Diagnostics"));
        assert!(err(2300, "pc-diagnostics.exe").contains("PC Diagnostics"));
        assert!(err(2100, "other.exe").contains("un'altra sessione"));
        assert!(err(2200, "svc.exe").contains("servizio"));
    }

    #[cfg(windows)]
    #[test]
    fn temp_root_must_be_inside_profile() {
        let profile = PathBuf::from(r"\\?\C:\Users\anna");
        let temp = PathBuf::from(r"\\?\C:\Users\anna\AppData\Local\Temp");
        assert!(check_temp_root(&temp, &profile, IT).is_ok());
        assert!(check_temp_root(&profile, &profile, IT).is_err());
        assert!(check_temp_root(&PathBuf::from(r"\\?\C:\"), &profile, IT).is_err());
        assert!(check_temp_root(&PathBuf::from(r"\\?\C:\Windows\Temp"), &profile, IT).is_err());
        assert!(check_temp_root(&PathBuf::from(r"\\?\C:\Users\anna2\Temp"), &profile, IT).is_err());
        assert!(check_temp_root(&temp, &PathBuf::from(r"\\?\C:\"), IT).is_err());
        assert!(check_temp_root(&PathBuf::from("/"), &PathBuf::from("/"), IT).is_err());
    }

    #[test]
    fn temp_messages() {
        let r = TempCleanReport {
            deleted_files: 12,
            freed_bytes: 3 * 1024 * 1024,
            skipped_files: 2,
            ..Default::default()
        };
        assert_eq!(
            temp_clean_message(&r, 7, IT),
            "Eliminati 12 file, liberati 3 MB; 2 file saltati perché in uso o protetti"
        );
        assert!(temp_clean_message(&TempCleanReport::default(), 1, IT).contains("1 giorno"));
    }

    #[test]
    fn english_texts() {
        assert_eq!(
            settings_page_label(SettingsPage::StorageSense, EN),
            "Storage Sense"
        );
        assert_eq!(settings_page_label(SettingsPage::Sound, IT), "Audio");
        assert_eq!(format_bytes(1, EN), "1 byte");
        assert_eq!(format_bytes(1023, EN), "1023 bytes");
        assert_eq!(format_bytes(1536, EN), "1.5 KB");
        assert_eq!(format_bytes(1_288_490_189, EN), "1.2 GB");
        assert_eq!(
            check_days(0, EN).expect_err("out of range"),
            "The number of days must be between 1 and 365 (received 0)"
        );
        let r = TempCleanReport {
            deleted_files: 12,
            freed_bytes: 3 * 1024 * 1024,
            skipped_files: 1,
            removed_dirs: 1,
            truncated: true,
            ..Default::default()
        };
        assert_eq!(
            temp_clean_message(&r, 7, EN),
            "Deleted 12 files, freed 3 MB; removed 1 empty folder; skipped 1 file in use or \
             protected; the folder is very large: run the action again to continue"
        );
        assert_eq!(
            temp_clean_message(&TempCleanReport::default(), 1, EN),
            "No temporary files older than 1 day: there was nothing to delete"
        );

        let me = 1000;
        let procs = vec![
            proc(700, 600, "explorer.exe", 1, 3),
            proc(me, 700, "pc-diagnostics.exe", 1, 10),
            proc(2000, 700, "notepad.exe", 1, 20),
            proc(2100, 700, "other.exe", 2, 21),
        ];
        let err = |pid, name| check_close_target(&procs, me, pid, name, EN).expect_err("denied");
        assert_eq!(
            err(4, "System"),
            "This is a protected system process and can't be closed"
        );
        assert_eq!(
            err(9999, "notepad.exe"),
            "The program (PID 9999) is no longer running"
        );
        assert!(err(2000, "chrome.exe").contains("belongs to “notepad.exe”, not “chrome.exe”"));
        assert!(err(700, "explorer.exe").contains("protected Windows process"));
        assert!(err(2100, "other.exe").contains("another user session"));
        assert!(
            check_temp_root(&PathBuf::from("/"), &PathBuf::from("/"), EN)
                .expect_err("root")
                .contains("drive root")
        );
    }

    /// A scratch directory of our own (never the real temp contents), removed on drop.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(tag: &str) -> Self {
            let p = std::env::temp_dir().join(format!(
                "pcdiag-remediation-test-{}-{tag}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .map(|d| d.as_nanos())
                    .unwrap_or(0)
            ));
            fs::create_dir_all(&p).expect("create scratch");
            Self(fs::canonicalize(&p).expect("canonical scratch"))
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    const DAY: Duration = Duration::from_secs(86_400);

    /// Writes `bytes` bytes and dates both the modification and the access time
    /// `age_days` back (macOS also requires an old access time).
    fn write_file(path: &Path, bytes: usize, age_days: u64) {
        fs::write(path, vec![b'x'; bytes]).expect("write");
        let f = fs::OpenOptions::new()
            .write(true)
            .open(path)
            .expect("open for times");
        let t = SystemTime::now() - DAY * u32::try_from(age_days).unwrap_or(0);
        f.set_times(fs::FileTimes::new().set_accessed(t).set_modified(t))
            .expect("set times");
    }

    #[cfg(unix)]
    fn set_dir_mtime(path: &Path, age_days: u32) {
        let t = SystemTime::now() - DAY * age_days;
        fs::File::open(path)
            .expect("open dir")
            .set_times(fs::FileTimes::new().set_accessed(t).set_modified(t))
            .expect("set dir times");
    }

    #[cfg(windows)]
    fn set_dir_mtime(path: &Path, age_days: u32) {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_WRITE_ATTRIBUTES: u32 = 0x100;
        const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
        let f = fs::OpenOptions::new()
            .access_mode(FILE_WRITE_ATTRIBUTES)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
            .open(path)
            .expect("open dir");
        f.set_modified(SystemTime::now() - DAY * age_days)
            .expect("set dir mtime");
    }

    #[test]
    fn temp_cleaning_selects_only_old_files() {
        let s = Scratch::new("select");
        let root = &s.0;
        write_file(&root.join("old.tmp"), 1000, 10);
        write_file(&root.join("new.tmp"), 500, 0);
        write_file(&root.join("edge.tmp"), 10, 6);
        let sub = root.join("sub");
        fs::create_dir(&sub).expect("sub");
        write_file(&sub.join("old2.tmp"), 24, 30);
        let keep = root.join("recent-dir");
        fs::create_dir(&keep).expect("recent dir");
        write_file(&keep.join("old3.tmp"), 1, 30);
        #[cfg(any(windows, unix))]
        set_dir_mtime(&sub, 30);

        let r = clean_temp_dir(root, 7, SystemTime::now()).expect("clean");
        assert_eq!(r.deleted_files, 3, "{r:?}");
        assert_eq!(r.freed_bytes, 1025, "{r:?}");
        assert_eq!(r.skipped_files, 0);
        assert!(root.join("new.tmp").exists());
        assert!(root.join("edge.tmp").exists());
        assert!(!root.join("old.tmp").exists());
        // An old folder emptied by the cleaning goes; a recent (possibly in-use) one stays.
        #[cfg(any(windows, unix))]
        {
            assert!(!sub.exists());
            assert_eq!(r.removed_dirs, 1);
        }
        assert!(keep.exists() && !keep.join("old3.tmp").exists());
        assert!(root.exists(), "the root is never removed");
    }

    #[cfg(windows)]
    #[test]
    fn temp_cleaning_skips_files_in_use() {
        use std::os::windows::fs::OpenOptionsExt;
        let s = Scratch::new("inuse");
        let path = s.0.join("locked.tmp");
        write_file(&path, 100, 10);
        let lock = fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(&path)
            .expect("exclusive open");
        let r = clean_temp_dir(&s.0, 1, SystemTime::now()).expect("clean");
        assert_eq!(r.deleted_files, 0);
        assert_eq!(r.skipped_files, 1);
        assert!(path.exists());
        drop(lock);
        assert!(temp_clean_message(&r, 1, IT).contains("1 file saltati"));
    }

    #[cfg(windows)]
    #[test]
    fn temp_cleaning_never_follows_links() {
        let s = Scratch::new("links");
        let root = s.0.join("root");
        let outside = s.0.join("outside");
        fs::create_dir(&root).expect("root");
        fs::create_dir(&outside).expect("outside");
        let precious = outside.join("precious.txt");
        write_file(&precious, 64, 100);
        let root = fs::canonicalize(&root).expect("canonical root");

        // Junctions need no privilege.
        let junction = root.join("junction");
        let made_junction = std::process::Command::new("cmd")
            .arg("/c")
            .arg("mklink")
            .arg("/J")
            .arg(junction.as_os_str())
            .arg(outside.as_os_str())
            .output()
            .is_ok_and(|o| o.status.success());
        // File symlinks need Developer Mode or admin; tested when available.
        let made_symlink =
            std::os::windows::fs::symlink_file(&precious, root.join("link.txt")).is_ok();

        let r = clean_temp_dir(&root, 1, SystemTime::now()).expect("clean");
        assert!(precious.exists(), "target outside the root survives");
        assert_eq!(r.deleted_files, 0, "{r:?}");
        let expected = u64::from(made_junction) + u64::from(made_symlink);
        assert_eq!(r.skipped_reparse, expected, "{r:?}");
        if made_junction {
            assert!(junction.exists(), "the junction itself is left alone");
        }
        eprintln!("junction tested: {made_junction}, symlink tested: {made_symlink}");
        // Remove the junction before the scratch cleanup so remove_dir_all never sees it.
        let _ = fs::remove_dir(&junction);
    }

    #[test]
    fn temp_root_that_is_not_a_dir_is_rejected() {
        let s = Scratch::new("notdir");
        let f = s.0.join("file");
        write_file(&f, 1, 0);
        assert!(clean_temp_dir(&f, 1, SystemTime::now()).is_err());
    }

    /// Symlinks (to a file and to a folder outside the root) are never followed or deleted.
    #[cfg(unix)]
    #[test]
    fn temp_cleaning_never_follows_symlinks() {
        let s = Scratch::new("symlinks");
        let root = s.0.join("root");
        let outside = s.0.join("outside");
        fs::create_dir(&root).expect("root");
        fs::create_dir(&outside).expect("outside");
        let precious = outside.join("precious.txt");
        write_file(&precious, 64, 100);
        let root = fs::canonicalize(&root).expect("canonical root");
        std::os::unix::fs::symlink(&precious, root.join("link.txt")).expect("file link");
        std::os::unix::fs::symlink(&outside, root.join("dir-link")).expect("dir link");

        let r = clean_temp_dir(&root, 1, SystemTime::now()).expect("clean");
        assert!(precious.exists(), "target outside the root survives");
        assert_eq!(r.deleted_files, 0, "{r:?}");
        assert_eq!(r.skipped_reparse, 2, "{r:?}");
        assert!(root.join("link.txt").symlink_metadata().is_ok());
        assert!(root.join("dir-link").symlink_metadata().is_ok());
    }

    /// A file modified long ago but read recently is kept on macOS (it may still be open).
    #[cfg(target_os = "macos")]
    #[test]
    fn temp_cleaning_keeps_recently_read_files() {
        let s = Scratch::new("atime");
        let path = s.0.join("old-but-read.tmp");
        write_file(&path, 10, 30);
        let f = fs::OpenOptions::new()
            .write(true)
            .open(&path)
            .expect("open");
        f.set_times(fs::FileTimes::new().set_accessed(SystemTime::now()))
            .expect("atime");
        drop(f);
        let r = clean_temp_dir(&s.0, 7, SystemTime::now()).expect("clean");
        assert_eq!(r.deleted_files, 0, "{r:?}");
        assert!(path.exists());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_refuses_windows_only_actions() {
        let windows_only = [
            RemediationAction::OpenDeviceManager,
            RemediationAction::OpenDiskCleanup { drive: "C:".into() },
            RemediationAction::EmptyRecycleBin,
            RemediationAction::CloseProgram {
                pid: std::process::id(),
                name: "x".into(),
            },
            RemediationAction::RunSystemFileCheck,
            RemediationAction::OpenSettingsPage {
                page: SettingsPage::Troubleshoot,
            },
            RemediationAction::OpenSettingsPage {
                page: SettingsPage::WindowsUpdateOptional,
            },
            RemediationAction::OpenSettingsPage {
                page: SettingsPage::InstalledApps,
            },
            RemediationAction::OpenSettingsPage {
                page: SettingsPage::Power,
            },
        ];
        for a in &windows_only {
            let it = validate(a, IT).expect_err("refused");
            let en = validate(a, EN).expect_err("refused");
            assert!(!it.is_empty() && !en.is_empty());
            assert!(en.contains("macOS") || en.contains("Windows"), "{en}");
            // Nothing runs: refused before any side effect, as NotSupported.
            let f = execute(a, EN).expect_err("not run");
            assert_eq!(f.error.code, ErrorCode::NotSupported, "{a:?}");
            assert_eq!(f.message, en);
        }
        assert_eq!(
            validate(&RemediationAction::EmptyRecycleBin, EN).expect_err("trash"),
            "On macOS the app doesn't empty the Trash: you can empty it from the Finder \
             (Finder menu > Empty Trash)"
        );
        assert!(recycle_bin_contents().is_err_and(|e| e.code == ErrorCode::NotSupported));
    }

    /// Supported macOS actions validate against the real system (nothing is opened or
    /// deleted).
    #[cfg(target_os = "macos")]
    #[test]
    fn host_validation_macos() {
        for page in crate::macos::remediation::supported_pages() {
            validate(&RemediationAction::OpenSettingsPage { page }, IT).expect("pane");
        }
        validate(
            &RemediationAction::CleanTempFiles { older_than_days: 7 },
            IT,
        )
        .expect("temp folder accepted");
        assert!(
            validate(
                &RemediationAction::CleanTempFiles { older_than_days: 0 },
                IT
            )
            .is_err()
        );
        let errs = execute(
            &RemediationAction::CleanTempFiles { older_than_days: 0 },
            IT,
        )
        .expect_err("invalid days refused by execute");
        assert_eq!(errs.error.code, ErrorCode::InvalidInput);
    }

    /// Opens the Storage pane of System Settings. Run manually with --ignored.
    #[cfg(target_os = "macos")]
    #[test]
    #[ignore = "opens System Settings"]
    fn host_open_settings_macos() {
        let m = execute(
            &RemediationAction::OpenSettingsPage {
                page: SettingsPage::StorageSense,
            },
            EN,
        )
        .expect("settings");
        assert_eq!(m, "Opened “Storage” in System Settings");
    }

    #[cfg(windows)]
    #[test]
    fn host_validation() {
        // Pure input checks and the real temp-folder resolution (nothing is deleted).
        assert!(
            validate(
                &RemediationAction::CleanTempFiles { older_than_days: 7 },
                IT
            )
            .is_ok()
        );
        assert!(
            validate(
                &RemediationAction::CleanTempFiles { older_than_days: 0 },
                IT
            )
            .is_err()
        );
        assert!(
            validate(
                &RemediationAction::CleanTempFiles {
                    older_than_days: 400
                },
                IT
            )
            .is_err()
        );
        assert!(
            validate(
                &RemediationAction::OpenDiskCleanup { drive: "C:".into() },
                IT
            )
            .is_ok()
        );
        assert!(
            validate(
                &RemediationAction::OpenDiskCleanup {
                    drive: "C:\\Windows".into()
                },
                EN
            )
            .is_err_and(|r| r == "Invalid drive: “C:\\Windows”")
        );
        assert!(validate(&RemediationAction::OpenDeviceManager, IT).is_ok());
        assert!(validate(&RemediationAction::RunSystemFileCheck, IT).is_ok());

        let close = |pid: u32, name: &str| {
            validate(
                &RemediationAction::CloseProgram {
                    pid,
                    name: name.into(),
                },
                IT,
            )
        };
        let me = std::process::id();
        let my_name = std::env::current_exe()
            .ok()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
            .unwrap_or_default();
        assert!(close(me, &my_name).is_err(), "never self");
        assert!(close(4, "System").is_err());
        assert!(close(u32::MAX - 3, "notepad.exe").is_err());

        let mut e = crate::process::ProcessEnumerator::new();
        let scan = e.scan().expect("scan");
        for name in [
            "csrss.exe",
            "winlogon.exe",
            "lsass.exe",
            "explorer.exe",
            "dwm.exe",
        ] {
            if let Some(p) = scan
                .processes
                .iter()
                .find(|p| p.name.eq_ignore_ascii_case(name))
            {
                let reason = close(p.pid, &p.name).expect_err(name);
                eprintln!("{name}: {reason}");
            }
        }
        let errs = execute(
            &RemediationAction::CleanTempFiles { older_than_days: 0 },
            IT,
        )
        .expect_err("invalid days refused by execute");
        assert_eq!(errs.error.code, ErrorCode::InvalidInput);
    }

    #[cfg(windows)]
    #[test]
    fn host_recycle_bin_query_is_read_only() {
        let (items, bytes) = recycle_bin_contents().expect("query");
        eprintln!("recycle bin: {items} items, {bytes} bytes");
    }

    /// Starts Notepad (detached, so it is not our descendant), validates and closes it.
    /// Launches a visible window: run with `cargo test -p platform -- --ignored`.
    #[cfg(windows)]
    #[test]
    #[ignore = "launches Notepad"]
    fn host_close_notepad() {
        let mut e = crate::process::ProcessEnumerator::new();
        let before: std::collections::HashSet<u32> = e
            .scan()
            .expect("scan")
            .processes
            .iter()
            .filter(|p| process_base_name(&p.name) == "notepad")
            .map(|p| p.pid)
            .collect();
        // `start` through a short-lived cmd: Notepad's parent exits, so it does not count
        // as part of this process tree.
        let st = std::process::Command::new("cmd")
            .args(["/c", "start", "", "notepad.exe"])
            .status()
            .expect("start notepad");
        assert!(st.success());

        let deadline = std::time::Instant::now() + Duration::from_secs(15);
        let target = loop {
            let windows = crate::process::visible_window_pids().expect("windows");
            let found = e.scan().expect("scan").processes.into_iter().find(|p| {
                process_base_name(&p.name) == "notepad"
                    && !before.contains(&p.pid)
                    && windows.contains(&p.pid)
            });
            if let Some(p) = found {
                break Some(p);
            }
            if std::time::Instant::now() > deadline {
                break None;
            }
            std::thread::sleep(Duration::from_millis(250));
        };
        let Some(target) = target else {
            eprintln!(
                "SKIPPED: no new Notepad process with a window (an existing Notepad probably opened a tab)"
            );
            return;
        };
        eprintln!("notepad pid {} ({})", target.pid, target.name);
        let action = RemediationAction::CloseProgram {
            pid: target.pid,
            name: "notepad.exe".into(),
        };
        validate(&action, IT).expect("notepad is closable");
        let msg = execute(&action, IT).expect("close");
        eprintln!("{msg}");
        assert!(msg.contains("si è chiuso"), "{msg}");
        assert!(
            validate(&action, IT)
                .expect_err("gone")
                .contains("non è più in esecuzione")
        );
    }

    /// Opens a Settings page, Device Manager and Disk Cleanup. Run manually with --ignored.
    #[cfg(windows)]
    #[test]
    #[ignore = "opens Windows UI"]
    fn host_open_ui() {
        let m = execute(
            &RemediationAction::OpenSettingsPage {
                page: SettingsPage::StorageSense,
            },
            IT,
        )
        .expect("settings");
        eprintln!("{m}");
        eprintln!(
            "{}",
            execute(&RemediationAction::OpenDeviceManager, EN).expect("devmgmt")
        );
        eprintln!(
            "{}",
            execute(
                &RemediationAction::OpenDiskCleanup {
                    drive: "c:\\".into()
                },
                IT,
            )
            .expect("cleanmgr")
        );
    }
}
