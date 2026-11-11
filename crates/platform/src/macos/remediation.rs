//! macOS side of the assistant's whitelisted actions: System Settings panes opened by a
//! fixed `x-apple.systempreferences:` URL, and the per-user temporary folder.
//!
//! - [`settings_url`]: the closed list of pane URLs. Only these strings are ever opened.
//! - [`open_settings_url`]: `/usr/bin/open <url>` with an argument from that list (no
//!   shell, no user- or model-provided text).
//! - [`user_temp_dir`]: `confstr(_CS_DARWIN_USER_TEMP_DIR)`, the per-user folder under
//!   `/private/var/folders/…/T` that `$TMPDIR` normally points to, checked with
//!   [`check_temp_root`] before anything is deleted from it.

use std::ffi::CStr;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use models::assistant::SettingsPage;
use models::settings::Language;
use models::{AppError, ErrorCode};

/// URL scheme of System Settings panes.
pub const SETTINGS_SCHEME: &str = "x-apple.systempreferences:";

/// The System Settings pane matching `page`, by its extension bundle id (macOS 13 and
/// later). `None` for pages with no macOS equivalent: optional Windows updates,
/// troubleshooters, the installed-apps list, and Power (the pane is "Battery" on laptops
/// and "Energy" on desktops, so one fixed URL cannot name it reliably).
pub fn settings_url(page: SettingsPage) -> Option<&'static str> {
    match page {
        SettingsPage::WindowsUpdate => {
            Some("x-apple.systempreferences:com.apple.Software-Update-Settings.extension")
        }
        SettingsPage::StorageSense => Some("x-apple.systempreferences:com.apple.settings.Storage"),
        SettingsPage::Display => {
            Some("x-apple.systempreferences:com.apple.Displays-Settings.extension")
        }
        SettingsPage::Sound => Some("x-apple.systempreferences:com.apple.Sound-Settings.extension"),
        SettingsPage::Bluetooth => Some("x-apple.systempreferences:com.apple.BluetoothSettings"),
        SettingsPage::Network => {
            Some("x-apple.systempreferences:com.apple.Network-Settings.extension")
        }
        SettingsPage::StartupApps => {
            Some("x-apple.systempreferences:com.apple.LoginItems-Settings.extension")
        }
        SettingsPage::Privacy => {
            Some("x-apple.systempreferences:com.apple.settings.PrivacySecurity.extension")
        }
        SettingsPage::WindowsUpdateOptional
        | SettingsPage::Troubleshoot
        | SettingsPage::InstalledApps
        | SettingsPage::Power => None,
    }
}

/// Name of the pane as System Settings titles it in `lang` (from the panes' own
/// localizations).
pub fn settings_label(page: SettingsPage, lang: Language) -> Option<&'static str> {
    settings_url(page)?;
    Some(match page {
        SettingsPage::WindowsUpdate => lang.pick("Aggiornamento software", "Software Update"),
        SettingsPage::StorageSense => lang.pick("Spazio", "Storage"),
        SettingsPage::Display => lang.pick("Schermi", "Displays"),
        SettingsPage::Sound => lang.pick("Suono", "Sound"),
        SettingsPage::Bluetooth => "Bluetooth",
        SettingsPage::Network => lang.pick("Rete", "Network"),
        SettingsPage::StartupApps => lang.pick("Elementi login", "Login Items"),
        SettingsPage::Privacy => lang.pick("Privacy e sicurezza", "Privacy & Security"),
        SettingsPage::WindowsUpdateOptional
        | SettingsPage::Troubleshoot
        | SettingsPage::InstalledApps
        | SettingsPage::Power => return None,
    })
}

/// Every page with a macOS pane.
pub fn supported_pages() -> impl Iterator<Item = SettingsPage> {
    ALL_PAGES.into_iter().filter(|p| settings_url(*p).is_some())
}

const ALL_PAGES: [SettingsPage; 12] = [
    SettingsPage::WindowsUpdate,
    SettingsPage::WindowsUpdateOptional,
    SettingsPage::StorageSense,
    SettingsPage::Display,
    SettingsPage::Sound,
    SettingsPage::Bluetooth,
    SettingsPage::Network,
    SettingsPage::Power,
    SettingsPage::Troubleshoot,
    SettingsPage::StartupApps,
    SettingsPage::InstalledApps,
    SettingsPage::Privacy,
];

/// Opens `url` in System Settings. Refuses anything that is not exactly one of the
/// [`settings_url`] values. Returns when `open` has handed the URL to Launch Services.
pub fn open_settings_url(url: &str) -> Result<(), AppError> {
    if !url.starts_with(SETTINGS_SCHEME) || !supported_pages().any(|p| settings_url(p) == Some(url))
    {
        return Err(AppError::invalid_input(
            "not a whitelisted System Settings URL",
        ));
    }
    let status = Command::new("/usr/bin/open")
        .arg(url)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|e| {
            AppError::new(ErrorCode::Platform, "could not start /usr/bin/open")
                .with_detail(e.to_string())
        })?;
    if status.success() {
        Ok(())
    } else {
        Err(
            AppError::new(ErrorCode::Platform, "System Settings did not open the pane")
                .with_detail(format!("/usr/bin/open exited with {status}")),
        )
    }
}

/// The per-user temporary folder (`confstr(_CS_DARWIN_USER_TEMP_DIR)`), not yet
/// canonicalized. The environment (`$TMPDIR`) is not consulted: it can be changed by
/// whoever starts the app.
pub fn user_temp_dir() -> Result<PathBuf, AppError> {
    let mut buf = vec![0u8; 1024];
    // SAFETY: `buf` is a live, writable buffer of `buf.len()` bytes; confstr writes at most
    // that many bytes, NUL-terminated, and returns the size it needed (0 on error).
    let needed = unsafe {
        libc::confstr(
            libc::_CS_DARWIN_USER_TEMP_DIR,
            buf.as_mut_ptr().cast(),
            buf.len(),
        )
    };
    if needed == 0 || needed > buf.len() {
        return Err(
            AppError::new(ErrorCode::Platform, "the user temporary folder is unknown")
                .with_detail(format!("confstr returned {needed}")),
        );
    }
    let s = CStr::from_bytes_until_nul(&buf)
        .map_err(|e| AppError::internal("confstr result").with_detail(e.to_string()))?
        .to_str()
        .map_err(|e| AppError::internal("confstr result").with_detail(e.to_string()))?;
    Ok(PathBuf::from(s))
}

/// The real uid of this process.
pub fn current_uid() -> u32 {
    // SAFETY: getuid has no preconditions and cannot fail.
    unsafe { libc::getuid() }
}

/// Facts about the candidate temp folder that [`check_temp_root`] judges. `path` must be
/// canonical; the metadata must come from `symlink_metadata` of that path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TempRootFacts<'a> {
    pub path: &'a Path,
    pub is_plain_dir: bool,
    pub owner_uid: u32,
    pub mode: u32,
}

impl<'a> TempRootFacts<'a> {
    pub fn read(path: &'a Path) -> std::io::Result<Self> {
        let md = std::fs::symlink_metadata(path)?;
        Ok(Self {
            path,
            is_plain_dir: md.is_dir() && !md.file_type().is_symlink(),
            owner_uid: md.uid(),
            mode: md.mode(),
        })
    }
}

/// Accepts only the per-user folder of `/private/var/folders/<xx>/<id>/T`, owned by `uid`,
/// a plain directory, not writable by group or others. Anything else (the system-wide
/// `/tmp`, a home folder, a shared or foreign folder) is refused with a reason in `lang`.
pub fn check_temp_root(facts: &TempRootFacts, uid: u32, lang: Language) -> Result<(), String> {
    let refuse = |it: &str, en: &str| Err(lang.pick(it, en).to_owned());
    let parts: Vec<_> = facts
        .path
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    let shape_ok = parts.len() == 7
        && parts[..4] == ["/", "private", "var", "folders"]
        && parts[6] == "T"
        && !parts[4].is_empty()
        && !parts[5].is_empty();
    if !shape_ok {
        return refuse(
            "La cartella dei file temporanei non è quella personale di macOS: pulizia rifiutata",
            "The temporary files folder is not the macOS per-user one: cleanup refused",
        );
    }
    if !facts.is_plain_dir {
        return refuse(
            "La cartella dei file temporanei non è una cartella normale: pulizia rifiutata",
            "The temporary files folder is not a plain folder: cleanup refused",
        );
    }
    if facts.owner_uid != uid || uid == 0 {
        return refuse(
            "La cartella dei file temporanei appartiene a un altro utente: pulizia rifiutata",
            "The temporary files folder belongs to another user: cleanup refused",
        );
    }
    if facts.mode & 0o022 != 0 {
        return refuse(
            "La cartella dei file temporanei è condivisa con altri utenti: pulizia rifiutata",
            "The temporary files folder is shared with other users: cleanup refused",
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const IT: Language = Language::Italian;
    const EN: Language = Language::English;

    fn facts(path: &Path) -> TempRootFacts<'_> {
        TempRootFacts {
            path,
            is_plain_dir: true,
            owner_uid: 501,
            mode: 0o40700,
        }
    }

    #[test]
    fn settings_urls_are_a_closed_fixed_list() {
        let urls: Vec<_> = supported_pages().filter_map(settings_url).collect();
        assert_eq!(urls.len(), 8);
        let unique: std::collections::HashSet<_> = urls.iter().collect();
        assert_eq!(unique.len(), urls.len());
        for u in &urls {
            let id = u.strip_prefix(SETTINGS_SCHEME).expect("scheme");
            assert!(id.starts_with("com.apple."), "{u}");
            assert!(
                id.chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-'),
                "{u}"
            );
        }
        for p in supported_pages() {
            assert!(settings_label(p, IT).is_some_and(|l| !l.is_empty()));
            assert!(settings_label(p, EN).is_some_and(|l| !l.is_empty()));
        }
        for p in [
            SettingsPage::WindowsUpdateOptional,
            SettingsPage::Troubleshoot,
            SettingsPage::InstalledApps,
            SettingsPage::Power,
        ] {
            assert_eq!(settings_url(p), None);
            assert_eq!(settings_label(p, EN), None);
        }
        assert_eq!(
            settings_label(SettingsPage::StorageSense, IT),
            Some("Spazio")
        );
    }

    #[test]
    fn only_whitelisted_urls_are_opened() {
        for bad in [
            "",
            "x-apple.systempreferences:",
            "x-apple.systempreferences:com.apple.settings.Storage ",
            "x-apple.systempreferences:com.apple.settings.Storage;open -a Calculator",
            "file:///Applications/Calculator.app",
            "https://example.com",
            "/System/Applications/Utilities/Terminal.app",
        ] {
            let e = open_settings_url(bad).expect_err(bad);
            assert_eq!(e.code, ErrorCode::InvalidInput, "{bad}");
        }
    }

    /// Every URL names a System Settings extension that exists on this Mac (the bundle id
    /// appears in one of the system's `.appex` Info.plist files). Read-only.
    #[test]
    fn host_settings_panes_exist() {
        let dir = Path::new("/System/Library/ExtensionKit/Extensions");
        let plists: Vec<Vec<u8>> = std::fs::read_dir(dir)
            .expect("extensions dir")
            .flatten()
            .filter_map(|e| std::fs::read(e.path().join("Contents/Info.plist")).ok())
            .collect();
        assert!(!plists.is_empty());
        for p in supported_pages() {
            let url = settings_url(p).expect("url");
            let id = url
                .strip_prefix(SETTINGS_SCHEME)
                .expect("scheme")
                .as_bytes();
            let found = plists.iter().any(|b| b.windows(id.len()).any(|w| w == id));
            assert!(found, "no extension with bundle id {url}");
        }
    }

    #[test]
    fn temp_root_rules() {
        let ok = PathBuf::from("/private/var/folders/3j/v65_7wb54216l2sfkdzcqmt00000gn/T");
        assert!(check_temp_root(&facts(&ok), 501, IT).is_ok());

        for bad in [
            "/",
            "/tmp",
            "/private/tmp",
            "/Users/anna",
            "/private/var/folders",
            "/private/var/folders/3j/v65/C",
            "/private/var/folders/3j/v65/T/sub",
            "/var/folders/3j/v65/T",
        ] {
            let p = PathBuf::from(bad);
            assert!(
                check_temp_root(&facts(&p), 501, EN)
                    .expect_err(bad)
                    .contains("per-user"),
                "{bad}"
            );
        }
        let mut f = facts(&ok);
        f.is_plain_dir = false;
        assert!(check_temp_root(&f, 501, EN).is_err());
        let f = facts(&ok);
        assert!(
            check_temp_root(&f, 502, IT)
                .expect_err("other user")
                .contains("altro utente")
        );
        assert!(
            check_temp_root(
                &TempRootFacts {
                    owner_uid: 0,
                    ..facts(&ok)
                },
                0,
                EN
            )
            .is_err()
        );
        let shared = TempRootFacts {
            mode: 0o41777,
            ..facts(&ok)
        };
        assert!(
            check_temp_root(&shared, 501, EN)
                .expect_err("shared")
                .contains("shared")
        );
    }

    /// The real per-user temp folder passes the checks (nothing is deleted).
    #[test]
    fn host_user_temp_dir_is_accepted() {
        let raw = user_temp_dir().expect("confstr");
        let canon = std::fs::canonicalize(&raw).expect("canonical");
        eprintln!("user temp dir: {} -> {}", raw.display(), canon.display());
        let facts = TempRootFacts::read(&canon).expect("metadata");
        check_temp_root(&facts, current_uid(), EN).expect("accepted");
    }
}
