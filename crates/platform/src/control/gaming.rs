//! OS access for the gaming center. Owned by the gaming feature.
//!
//! - Library: Steam (`steam`), Epic (`epic`), macOS game apps (`macapps`), GOG on Windows
//!   (registry read), and user-picked apps / executables ([`manual_game`]).
//! - Art from the launchers' local caches and app icons ([`read_art`]); nothing is fetched.
//! - Launch of library entries only ([`launch`]): launcher URLs with a fixed scheme, an app
//!   bundle through `/usr/bin/open -a`, an executable without a shell.
//! - Running executables for game detection ([`SystemProcessLister`]).

use std::path::{Path, PathBuf};

use models::control::gaming::{GameArtKind, GameSource};
use models::{AppError, ErrorCode};

pub mod epic;
pub mod icns;
pub mod macapps;
pub mod steam;
pub mod vdf;
#[cfg(windows)]
pub mod windows;

/// Largest art file returned (bytes).
pub const ART_MAX_BYTES: usize = 4 * 1024 * 1024;

/// How the app starts an entry. Built only by the scanners, never from frontend input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LaunchTarget {
    /// `steam://…` or `com.epicgames.launcher://…`, opened by the registered launcher.
    Url(String),
    /// A macOS `.app` bundle.
    AppBundle(PathBuf),
    /// A Windows executable, started in its folder.
    Executable(PathBuf),
}

/// How a running process is recognised as this game.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MatchRule {
    /// The executable lives under this folder (Steam/Epic install dir, an app bundle).
    UnderDir(PathBuf),
    /// The executable is exactly this file.
    Exact(PathBuf),
}

/// Art files of an entry on disk.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ArtPaths {
    pub cover: Option<PathBuf>,
    pub hero: Option<PathBuf>,
    pub logo: Option<PathBuf>,
    /// An `.icns` file.
    pub icon: Option<PathBuf>,
}

impl ArtPaths {
    pub fn get(&self, kind: GameArtKind) -> Option<&Path> {
        match kind {
            GameArtKind::Cover => self.cover.as_deref(),
            GameArtKind::Hero => self.hero.as_deref(),
            GameArtKind::Logo => self.logo.as_deref(),
            GameArtKind::Icon => self.icon.as_deref(),
        }
    }
}

/// One library entry as found on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScannedGame {
    pub id: String,
    pub title: String,
    pub source: GameSource,
    pub source_id: String,
    pub install_path: Option<PathBuf>,
    pub size_bytes: Option<u64>,
    /// Unix ms.
    pub last_played_ms: Option<u64>,
    pub art: ArtPaths,
    /// `None`: the entry cannot be started (e.g. its file is gone).
    pub launch: Option<LaunchTarget>,
    pub detect: Option<MatchRule>,
}

/// What the scan found for one source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceScan {
    pub source: GameSource,
    /// The app can read this source on this OS.
    pub supported: bool,
    pub found: bool,
    pub location: Option<PathBuf>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LibraryScan {
    pub games: Vec<ScannedGame>,
    pub sources: Vec<SourceScan>,
}

/// Scans every launcher source of this OS (not the manual entries).
pub fn scan_library() -> LibraryScan {
    let mut scan = LibraryScan::default();

    let steam_root = steam::steam_root();
    scan.sources.push(SourceScan {
        source: GameSource::Steam,
        supported: cfg!(any(windows, target_os = "macos")),
        found: steam_root.is_some(),
        location: steam_root.clone(),
    });
    if let Some(root) = &steam_root {
        for g in steam::scan_games(root) {
            let appid = g.manifest.appid;
            scan.games.push(ScannedGame {
                id: format!("steam:{appid}"),
                title: g.manifest.name.clone(),
                source: GameSource::Steam,
                source_id: appid.to_string(),
                install_path: Some(g.install_dir.clone()),
                size_bytes: g.manifest.size_on_disk,
                last_played_ms: g.manifest.last_played.map(|s| s.saturating_mul(1000)),
                art: ArtPaths {
                    cover: steam::art_path(root, appid, GameArtKind::Cover),
                    hero: steam::art_path(root, appid, GameArtKind::Hero),
                    logo: steam::art_path(root, appid, GameArtKind::Logo),
                    icon: None,
                },
                launch: Some(LaunchTarget::Url(format!("steam://rungameid/{appid}"))),
                detect: Some(MatchRule::UnderDir(g.install_dir)),
            });
        }
    }

    let epic_dir = epic::manifests_dir();
    let epic_found = epic_dir.as_ref().is_some_and(|d| d.is_dir());
    scan.sources.push(SourceScan {
        source: GameSource::Epic,
        supported: cfg!(any(windows, target_os = "macos")),
        found: epic_found,
        location: epic_dir.clone(),
    });
    if let Some(dir) = epic_dir.filter(|_| epic_found) {
        for m in epic::scan_games(&dir) {
            scan.games.push(ScannedGame {
                id: format!("epic:{}", m.app_name),
                title: m.display_name.clone(),
                source: GameSource::Epic,
                source_id: m.app_name.clone(),
                install_path: Some(m.install_location.clone()),
                size_bytes: m.install_size,
                last_played_ms: None,
                art: ArtPaths::default(),
                launch: Some(LaunchTarget::Url(epic::launch_url(&m))),
                detect: Some(MatchRule::UnderDir(m.install_location.clone())),
            });
        }
    }

    #[cfg(target_os = "macos")]
    {
        let dirs = macapps::app_dirs();
        let apps = macapps::scan(&dirs);
        scan.sources.push(SourceScan {
            source: GameSource::MacApp,
            supported: true,
            found: !apps.is_empty(),
            location: dirs.first().cloned(),
        });
        for app in apps {
            let source_id = app
                .info
                .bundle_id
                .clone()
                .unwrap_or_else(|| app.bundle.to_string_lossy().into_owned());
            scan.games.push(ScannedGame {
                id: format!("macApp:{source_id}"),
                title: app.title(),
                source: GameSource::MacApp,
                source_id,
                install_path: Some(app.bundle.clone()),
                size_bytes: None,
                last_played_ms: None,
                art: ArtPaths {
                    icon: app.icon_path(),
                    ..ArtPaths::default()
                },
                launch: Some(LaunchTarget::AppBundle(app.bundle.clone())),
                detect: Some(MatchRule::UnderDir(app.bundle)),
            });
        }
    }

    #[cfg(windows)]
    {
        let games = windows::gog_games();
        scan.sources.push(SourceScan {
            source: GameSource::Gog,
            supported: true,
            found: windows::gog_present(),
            location: Some(PathBuf::from("HKLM\\SOFTWARE\\WOW6432Node\\GOG.com\\Games")),
        });
        for g in games {
            scan.games.push(ScannedGame {
                id: format!("gog:{}", g.id),
                title: g.name.clone(),
                source: GameSource::Gog,
                source_id: g.id.clone(),
                install_path: Some(g.path.clone()),
                size_bytes: None,
                last_played_ms: None,
                art: ArtPaths::default(),
                launch: Some(LaunchTarget::Executable(g.exe.clone())),
                detect: Some(MatchRule::UnderDir(g.path)),
            });
        }
    }

    scan
}

/// FNV-1a 64: a short stable id for a path (manual entries).
pub fn stable_hash(s: &str) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{h:016x}")
}

fn strip_verbatim(p: PathBuf) -> PathBuf {
    let s = p.to_string_lossy();
    match s.strip_prefix("\\\\?\\") {
        Some(rest) if !rest.starts_with("UNC\\") => PathBuf::from(rest),
        _ => p,
    }
}

/// The id a manual entry for `path` gets (path as stored, already validated).
pub fn manual_id(path: &Path) -> String {
    let key = path.to_string_lossy();
    let key = if cfg!(windows) {
        key.to_lowercase()
    } else {
        key.into_owned()
    };
    format!("manual:{}", stable_hash(&key))
}

/// Validates a user-picked app and builds its entry: an existing `.app` bundle with an
/// `Info.plist` on macOS, an existing `.exe` on Windows. Returns the canonical path too.
pub fn manual_game(path: &Path) -> Result<ScannedGame, AppError> {
    if !path.is_absolute() {
        return Err(AppError::invalid_input("path must be absolute"));
    }
    let path = std::fs::canonicalize(path)
        .map(strip_verbatim)
        .map_err(|e| {
            AppError::new(ErrorCode::NotFound, "the chosen file does not exist")
                .with_detail(e.to_string())
        })?;
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    if cfg!(target_os = "macos") && ext == "app" && path.is_dir() {
        let info = macapps::read_bundle(&path)
            .ok_or_else(|| AppError::invalid_input("not an application bundle"))?;
        let title = info
            .name
            .clone()
            .unwrap_or_else(|| macapps::bundle_stem(&path));
        return Ok(ScannedGame {
            id: manual_id(&path),
            title,
            source: GameSource::Manual,
            source_id: info
                .bundle_id
                .clone()
                .unwrap_or_else(|| path.to_string_lossy().into_owned()),
            install_path: Some(path.clone()),
            size_bytes: None,
            last_played_ms: None,
            art: ArtPaths {
                icon: macapps::icon_path(&path, &info),
                ..ArtPaths::default()
            },
            launch: Some(LaunchTarget::AppBundle(path.clone())),
            detect: Some(MatchRule::UnderDir(path)),
        });
    }
    if cfg!(windows) && ext == "exe" && path.is_file() {
        let title = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        return Ok(ScannedGame {
            id: manual_id(&path),
            title,
            source: GameSource::Manual,
            source_id: path.to_string_lossy().into_owned(),
            install_path: Some(path.clone()),
            size_bytes: std::fs::metadata(&path).ok().map(|m| m.len()),
            last_played_ms: None,
            art: ArtPaths::default(),
            launch: Some(LaunchTarget::Executable(path.clone())),
            detect: Some(MatchRule::Exact(path)),
        });
    }
    Err(AppError::invalid_input(if cfg!(windows) {
        "choose a .exe file"
    } else {
        "choose an application (.app)"
    }))
}

/// Entry of a manual game whose file is gone: listed, not launchable.
pub fn missing_manual_game(path: &Path, title: &str) -> ScannedGame {
    ScannedGame {
        id: manual_id(path),
        title: title.to_owned(),
        source: GameSource::Manual,
        source_id: path.to_string_lossy().into_owned(),
        install_path: Some(path.to_path_buf()),
        size_bytes: None,
        last_played_ms: None,
        art: ArtPaths::default(),
        launch: None,
        detect: None,
    }
}

/// Image bytes and MIME type of one art file (`.icns`: its largest PNG).
pub fn read_art(path: &Path, kind: GameArtKind) -> Option<(Vec<u8>, &'static str)> {
    let meta = std::fs::metadata(path).ok()?;
    let is_icns = path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("icns"));
    let cap = if is_icns {
        4 * ART_MAX_BYTES
    } else {
        ART_MAX_BYTES
    };
    if usize::try_from(meta.len()).ok()? > cap {
        return None;
    }
    let bytes = std::fs::read(path).ok()?;
    let bytes = if kind == GameArtKind::Icon && is_icns {
        icns::largest_png(&bytes, ART_MAX_BYTES)?.to_vec()
    } else {
        bytes
    };
    let mime = image_mime(&bytes)?;
    Some((bytes, mime))
}

/// MIME type from the magic bytes (JPEG, PNG, WebP); other data is refused.
pub fn image_mime(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg")
    } else if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if bytes.len() > 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else {
        None
    }
}

/// Only launcher URLs with these schemes are opened.
const URL_SCHEMES: &[&str] = &["steam://rungameid/", "com.epicgames.launcher://apps/"];

/// Starts a library entry. No shell is involved; URLs must use a launcher scheme.
pub fn launch(target: &LaunchTarget) -> Result<(), AppError> {
    match target {
        LaunchTarget::Url(url) => {
            if !URL_SCHEMES.iter().any(|s| url.starts_with(s)) || url.contains(['\n', '\r', ' ']) {
                return Err(AppError::invalid_input(
                    "launch URL with an unexpected scheme",
                ));
            }
            open_url(url)
        }
        LaunchTarget::AppBundle(bundle) => open_bundle(bundle),
        LaunchTarget::Executable(exe) => {
            if !exe.is_file() {
                return Err(AppError::new(ErrorCode::NotFound, "executable not found"));
            }
            let mut cmd = std::process::Command::new(exe);
            if let Some(dir) = exe.parent() {
                cmd.current_dir(dir);
            }
            cmd.spawn().map(drop).map_err(|e| {
                AppError::new(ErrorCode::Platform, "could not start the game")
                    .with_detail(e.to_string())
            })
        }
    }
}

#[cfg(target_os = "macos")]
fn run_open(args: &[&std::ffi::OsStr]) -> Result<(), AppError> {
    let status = std::process::Command::new("/usr/bin/open")
        .args(args)
        .status()
        .map_err(|e| {
            AppError::new(ErrorCode::Platform, "could not run open").with_detail(e.to_string())
        })?;
    if status.success() {
        Ok(())
    } else {
        Err(AppError::new(ErrorCode::Platform, "open failed").with_detail(status.to_string()))
    }
}

fn open_url(url: &str) -> Result<(), AppError> {
    #[cfg(target_os = "macos")]
    {
        run_open(&[std::ffi::OsStr::new(url)])
    }
    #[cfg(windows)]
    {
        crate::windows::remediation::shell_open(url.to_owned(), None)
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let _ = url;
        Err(AppError::new(
            ErrorCode::NotSupported,
            "launch not supported here",
        ))
    }
}

fn open_bundle(bundle: &Path) -> Result<(), AppError> {
    #[cfg(target_os = "macos")]
    {
        if !bundle.is_dir() {
            return Err(AppError::new(ErrorCode::NotFound, "app not found"));
        }
        run_open(&[std::ffi::OsStr::new("-a"), bundle.as_os_str()])
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = bundle;
        Err(AppError::new(
            ErrorCode::NotSupported,
            "app bundles exist only on macOS",
        ))
    }
}

fn normalize(path: &Path, fold_case: bool) -> String {
    let mut s = path.to_string_lossy().into_owned();
    if let Some(rest) = s.strip_prefix("\\\\?\\") {
        s = rest.to_owned();
    }
    if fold_case {
        s = s.replace('/', "\\").to_lowercase();
    }
    s
}

/// Whether `exe` matches `rule`. `fold_case`: Windows paths (case-insensitive, `/` = `\`).
pub fn matches_with(rule: &MatchRule, exe: &Path, fold_case: bool) -> bool {
    let exe = normalize(exe, fold_case);
    match rule {
        MatchRule::Exact(path) => exe == normalize(path, fold_case),
        MatchRule::UnderDir(dir) => {
            let dir = normalize(dir, fold_case);
            let dir = dir.trim_end_matches(['/', '\\']);
            !dir.is_empty()
                && exe.len() > dir.len()
                && exe.starts_with(dir)
                && matches!(exe.as_bytes()[dir.len()], b'/' | b'\\')
        }
    }
}

/// [`matches_with`] with this OS's path rules.
pub fn matches(rule: &MatchRule, exe: &Path) -> bool {
    matches_with(rule, exe, cfg!(windows))
}

/// Lists the executables of the running processes (game detection).
pub trait ProcessLister: Send {
    fn running_executables(&mut self) -> Vec<PathBuf>;
}

/// [`ProcessLister`] on `sysinfo`: refreshes only the executable path of each process.
pub struct SystemProcessLister {
    /// `None` when `sysinfo` could not initialise (detection then sees nothing running).
    sys: Option<sysinfo::System>,
}

impl SystemProcessLister {
    pub fn new() -> Self {
        Self {
            sys: sysinfo::System::new()
                .map_err(|e| tracing::warn!(error = %e, "process listing unavailable"))
                .ok(),
        }
    }
}

impl Default for SystemProcessLister {
    fn default() -> Self {
        Self::new()
    }
}

impl ProcessLister for SystemProcessLister {
    fn running_executables(&mut self) -> Vec<PathBuf> {
        use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, UpdateKind};
        let Some(sys) = self.sys.as_mut() else {
            return Vec::new();
        };
        sys.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing().with_exe(UpdateKind::OnlyIfNotSet),
        );
        sys.processes()
            .values()
            .filter_map(|p| p.exe().map(Path::to_path_buf))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn match_rules() {
        let dir = MatchRule::UnderDir(PathBuf::from("/Games/steamapps/common/Geometry Dash"));
        assert!(matches_with(
            &dir,
            Path::new(
                "/Games/steamapps/common/Geometry Dash/Geometry Dash.app/Contents/MacOS/Geometry Dash"
            ),
            false
        ));
        assert!(!matches_with(
            &dir,
            Path::new("/Games/steamapps/common/Geometry Dash 2/x"),
            false
        ));
        assert!(!matches_with(
            &dir,
            Path::new("/Games/steamapps/common/Geometry Dash"),
            false
        ));

        let win = MatchRule::UnderDir(PathBuf::from(
            "C:\\Program Files\\Epic Games\\rocketleague\\",
        ));
        assert!(matches_with(
            &win,
            Path::new(
                "\\\\?\\c:/program files/epic games/RocketLeague/Binaries/Win64/RocketLeague.exe"
            ),
            true
        ));
        let exact = MatchRule::Exact(PathBuf::from("C:\\Games\\a.exe"));
        assert!(matches_with(&exact, Path::new("c:\\games\\A.EXE"), true));
        assert!(!matches_with(&exact, Path::new("c:\\games\\b.exe"), true));
        assert!(!matches_with(
            &MatchRule::UnderDir(PathBuf::from("/")),
            Path::new("/x"),
            false
        ));
    }

    #[test]
    fn ids_and_mime() {
        assert_eq!(stable_hash("a"), stable_hash("a"));
        assert_ne!(stable_hash("a"), stable_hash("b"));
        assert!(manual_id(Path::new("/Applications/X.app")).starts_with("manual:"));
        assert_eq!(image_mime(&[0xFF, 0xD8, 0xFF, 0xE0]), Some("image/jpeg"));
        assert_eq!(image_mime(b"\x89PNG\r\n\x1a\nxx"), Some("image/png"));
        assert_eq!(image_mime(b"<svg"), None);
    }

    #[test]
    fn launch_refuses_foreign_urls_and_manual_refuses_bad_paths() {
        assert!(launch(&LaunchTarget::Url("file:///etc/passwd".into())).is_err());
        assert!(launch(&LaunchTarget::Url("steam://rungameid/1 --x".into())).is_err());
        assert!(manual_game(Path::new("relative/path")).is_err());
        assert!(manual_game(Path::new("/definitely/not/here.app")).is_err());
        let file = std::env::temp_dir().join(format!("pcd-manual-{}.txt", std::process::id()));
        std::fs::write(&file, b"x").expect("write");
        assert_eq!(
            manual_game(&file).map(|_| ()).map_err(|e| e.code),
            Err(ErrorCode::InvalidInput)
        );
        let _ = std::fs::remove_file(&file);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn manual_app_bundle_is_accepted_on_macos() {
        let root = std::env::temp_dir().join(format!("pcd-manual-app-{}", std::process::id()));
        let app = root.join("Thing.app/Contents");
        std::fs::create_dir_all(&app).expect("mkdir");
        std::fs::write(
            app.join("Info.plist"),
            r#"<?xml version="1.0" encoding="UTF-8"?><plist version="1.0"><dict><key>CFBundleName</key><string>Thing</string></dict></plist>"#,
        )
        .expect("write");
        let game = manual_game(&root.join("Thing.app")).expect("valid bundle");
        assert_eq!(game.title, "Thing");
        assert_eq!(game.source, GameSource::Manual);
        assert!(matches!(game.launch, Some(LaunchTarget::AppBundle(_))));
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Prints what the real scanners find on this computer.
    /// `cargo test -p platform -- --ignored --nocapture print_real_library`
    #[test]
    #[ignore = "reads the real launchers of this computer"]
    fn print_real_library() {
        let scan = scan_library();
        for s in &scan.sources {
            println!(
                "source {:?}: found={} at {:?}",
                s.source, s.found, s.location
            );
        }
        for g in &scan.games {
            println!(
                "{} | {} | size={:?} | last_played={:?} | cover={} hero={} logo={} icon={} | {:?}",
                g.id,
                g.title,
                g.size_bytes,
                g.last_played_ms,
                g.art.cover.is_some(),
                g.art.hero.is_some(),
                g.art.logo.is_some(),
                g.art
                    .icon
                    .as_deref()
                    .and_then(|p| read_art(p, GameArtKind::Icon))
                    .map(|(b, _)| b.len())
                    .is_some(),
                g.launch
            );
        }
        let mut lister = SystemProcessLister::new();
        println!(
            "running executables: {}",
            lister.running_executables().len()
        );
    }
}
