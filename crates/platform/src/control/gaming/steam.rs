//! Steam library: `steamapps/libraryfolders.vdf` lists the library folders, each
//! `<library>/steamapps/appmanifest_<appid>.acf` describes one installed app. Covers come
//! from the client's local cache `appcache/librarycache`.

use std::path::{Path, PathBuf};

use super::vdf;

/// Installed Steam app as its manifest records it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SteamManifest {
    pub appid: u32,
    pub name: String,
    pub installdir: String,
    pub size_on_disk: Option<u64>,
    /// Unix seconds; `None` when 0 or missing.
    pub last_played: Option<u64>,
}

/// Steamworks redistributables, Proton and the Steam Linux runtimes: installed as apps but
/// not games.
const TOOL_APPIDS: &[u32] = &[
    228_980,   // Steamworks Common Redistributables
    1_070_560, // Steam Linux Runtime 1.0 (scout)
    1_391_110, // Steam Linux Runtime 2.0 (soldier)
    1_628_350, // Steam Linux Runtime 3.0 (sniper)
    1_493_710, // Proton Experimental
    2_180_100, // Proton Hotfix
    961_940,   // Proton 3.16
    1_054_830, // Proton 4.2
    1_113_280, // Proton 4.11
    1_245_040, // Proton 5.0
    1_420_170, // Proton 5.13
    1_580_130, // Proton 6.3
    1_887_720, // Proton 7.0
    2_348_590, // Proton 8.0
    2_805_730, // Proton 9.0
    3_658_110, // Proton 10.0
    250_820,   // SteamVR
];

/// Whether an installed app is a tool rather than a game.
pub fn is_tool(appid: u32, name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    TOOL_APPIDS.contains(&appid)
        || lower.starts_with("steamworks")
        || lower.starts_with("proton ")
        || lower.starts_with("steam linux runtime")
        || lower.contains("dedicated server")
}

/// Library folder paths from `libraryfolders.vdf` (new format: blocks with `path`; old
/// format: numbered keys with the path as value).
pub fn parse_library_folders(text: &str) -> Vec<PathBuf> {
    let doc = vdf::parse(text);
    let Some(root) = doc.get("libraryfolders") else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (key, value) in root.pairs() {
        if !key.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        let path = match value {
            vdf::VdfValue::Str(path) => Some(path.as_str()),
            vdf::VdfValue::Block(_) => value.str("path"),
        };
        if let Some(path) = path.filter(|p| !p.is_empty()) {
            let path = PathBuf::from(path);
            if !out.contains(&path) {
                out.push(path);
            }
        }
    }
    out
}

/// One `appmanifest_*.acf`. `None` when it lacks an id, a name or an install dir, or the app
/// is not fully installed (`StateFlags` without bit 4).
pub fn parse_app_manifest(text: &str) -> Option<SteamManifest> {
    let doc = vdf::parse(text);
    let app = doc.get("AppState")?;
    let appid = app.str("appid")?.trim().parse().ok()?;
    let name = app.str("name")?.trim().to_owned();
    let installdir = app.str("installdir")?.trim().to_owned();
    if name.is_empty() || installdir.is_empty() {
        return None;
    }
    if let Some(flags) = app
        .str("StateFlags")
        .and_then(|f| f.trim().parse::<u32>().ok())
        && flags & 4 == 0
    {
        return None;
    }
    let number = |key: &str| {
        app.str(key)
            .and_then(|v| v.trim().parse::<u64>().ok())
            .filter(|&v| v > 0)
    };
    Some(SteamManifest {
        appid,
        name,
        installdir,
        size_on_disk: number("SizeOnDisk"),
        last_played: number("LastPlayed"),
    })
}

/// The Steam client folder, if installed.
pub fn steam_root() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        let home = std::env::var_os("HOME")?;
        let root = PathBuf::from(home).join("Library/Application Support/Steam");
        root.is_dir().then_some(root)
    }
    #[cfg(windows)]
    {
        let from_registry = super::windows::reg_string(
            super::windows::Hive::CurrentUser,
            "Software\\Valve\\Steam",
            "SteamPath",
        )
        .map(|p| PathBuf::from(p.replace('/', "\\")));
        from_registry.filter(|p| p.is_dir()).or_else(|| {
            let fallback = PathBuf::from("C:\\Program Files (x86)\\Steam");
            fallback.is_dir().then_some(fallback)
        })
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        None
    }
}

/// An installed game with the folder it lives in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SteamGame {
    pub manifest: SteamManifest,
    /// `<library>/steamapps/common/<installdir>`.
    pub install_dir: PathBuf,
}

/// Every installed game of every library folder under `root`.
pub fn scan_games(root: &Path) -> Vec<SteamGame> {
    let mut libraries = std::fs::read_to_string(root.join("steamapps/libraryfolders.vdf"))
        .map(|t| parse_library_folders(&t))
        .unwrap_or_default();
    if !libraries.iter().any(|l| l == root) {
        libraries.insert(0, root.to_path_buf());
    }
    let mut out: Vec<SteamGame> = Vec::new();
    for library in libraries {
        let apps = library.join("steamapps");
        let Ok(dir) = std::fs::read_dir(&apps) else {
            continue;
        };
        for entry in dir.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if !(name.starts_with("appmanifest_") && name.ends_with(".acf")) {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(entry.path()) else {
                continue;
            };
            let Some(manifest) = parse_app_manifest(&text) else {
                continue;
            };
            if is_tool(manifest.appid, &manifest.name)
                || out.iter().any(|g| g.manifest.appid == manifest.appid)
            {
                continue;
            }
            let install_dir = apps.join("common").join(&manifest.installdir);
            out.push(SteamGame {
                manifest,
                install_dir,
            });
        }
    }
    out.sort_by_key(|g| g.manifest.appid);
    out
}

/// File names of each kind of art in the client cache, newest layout first.
fn art_names(kind: models::control::gaming::GameArtKind) -> &'static [&'static str] {
    use models::control::gaming::GameArtKind;
    match kind {
        GameArtKind::Cover => &["library_600x900.jpg", "library_600x900_2x.jpg"],
        GameArtKind::Hero => &["library_hero.jpg"],
        GameArtKind::Logo => &["logo.png"],
        GameArtKind::Icon => &[],
    }
}

/// Cached art of `appid`: `appcache/librarycache/<appid>/<name>`, or the legacy flat
/// `appcache/librarycache/<appid>_<name>`.
pub fn art_path(
    root: &Path,
    appid: u32,
    kind: models::control::gaming::GameArtKind,
) -> Option<PathBuf> {
    let cache = root.join("appcache/librarycache");
    art_names(kind).iter().find_map(|name| {
        [
            cache.join(appid.to_string()).join(name),
            cache.join(format!("{appid}_{name}")),
        ]
        .into_iter()
        .find(|p| p.is_file())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use models::control::gaming::GameArtKind;

    const LIBRARY: &str = r#""libraryfolders"
{
	"0"
	{
		"path"		"/Users/me/Library/Application Support/Steam"
		"label"		""
		"apps"
		{
			"322170"		"344474566"
		}
	}
	"1"
	{
		"path"		"D:\\SteamLibrary"
	}
}"#;

    const MANIFEST: &str = r#""AppState"
{
	"appid"		"322170"
	"Universe"		"1"
	"name"		"Geometry Dash"
	"StateFlags"		"4"
	"installdir"		"Geometry Dash"
	"LastUpdated"		"1794427653"
	"LastPlayed"		"1794496368"
	"SizeOnDisk"		"344474566"
	"InstalledDepots"
	{
		"322172" { "manifest" "2113602821724303233" "size" "344474566" }
	}
}"#;

    #[test]
    fn library_folders_new_and_old_format() {
        assert_eq!(
            parse_library_folders(LIBRARY),
            vec![
                PathBuf::from("/Users/me/Library/Application Support/Steam"),
                PathBuf::from("D:\\SteamLibrary"),
            ]
        );
        let old = "\"LibraryFolders\" { \"TimeNextStatsReport\" \"1\" \"1\" \"E:\\\\Steam\" }";
        assert_eq!(parse_library_folders(old), vec![PathBuf::from("E:\\Steam")]);
    }

    #[test]
    fn manifest_fields() {
        let m = parse_app_manifest(MANIFEST).expect("manifest");
        assert_eq!(m.appid, 322_170);
        assert_eq!(m.name, "Geometry Dash");
        assert_eq!(m.installdir, "Geometry Dash");
        assert_eq!(m.size_on_disk, Some(344_474_566));
        assert_eq!(m.last_played, Some(1_794_496_368));
    }

    #[test]
    fn manifest_not_installed_or_incomplete_is_skipped() {
        let downloading = MANIFEST.replace("\"StateFlags\"\t\t\"4\"", "\"StateFlags\"\t\t\"1026\"");
        assert!(parse_app_manifest(&downloading).is_none());
        let updating = MANIFEST.replace("\"StateFlags\"\t\t\"4\"", "\"StateFlags\"\t\t\"6\"");
        assert!(parse_app_manifest(&updating).is_some());
        assert!(parse_app_manifest("\"AppState\" { \"appid\" \"1\" }").is_none());
        let never_played = MANIFEST.replace("1794496368", "0");
        assert_eq!(
            parse_app_manifest(&never_played).and_then(|m| m.last_played),
            None
        );
    }

    #[test]
    fn tools_are_not_games() {
        assert!(is_tool(228_980, "Steamworks Common Redistributables"));
        assert!(is_tool(1, "Proton 9.0"));
        assert!(is_tool(2, "Counter-Strike 2 Dedicated Server"));
        assert!(!is_tool(322_170, "Geometry Dash"));
    }

    #[test]
    fn scans_a_fixture_library_and_finds_art() {
        let dir = std::env::temp_dir().join(format!("pcd-steam-{}", std::process::id()));
        let apps = dir.join("steamapps");
        std::fs::create_dir_all(apps.join("common/Geometry Dash")).expect("mkdir");
        std::fs::write(apps.join("libraryfolders.vdf"), "\"libraryfolders\" {}").expect("w");
        std::fs::write(apps.join("appmanifest_322170.acf"), MANIFEST).expect("w");
        std::fs::write(
            apps.join("appmanifest_228980.acf"),
            MANIFEST
                .replace("322170", "228980")
                .replace("Geometry Dash", "Steamworks Common Redistributables"),
        )
        .expect("w");
        let cache = dir.join("appcache/librarycache");
        std::fs::create_dir_all(cache.join("322170")).expect("mkdir");
        std::fs::write(cache.join("322170/library_600x900.jpg"), b"jpg").expect("w");
        std::fs::write(cache.join("322170_library_hero.jpg"), b"jpg").expect("w");

        let games = scan_games(&dir);
        assert_eq!(games.len(), 1);
        assert_eq!(games[0].install_dir, apps.join("common/Geometry Dash"));
        assert!(art_path(&dir, 322_170, GameArtKind::Cover).is_some());
        assert!(art_path(&dir, 322_170, GameArtKind::Hero).is_some());
        assert!(art_path(&dir, 322_170, GameArtKind::Logo).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
