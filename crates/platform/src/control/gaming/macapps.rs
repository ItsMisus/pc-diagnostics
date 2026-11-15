//! macOS game apps: bundles in `/Applications` and `~/Applications` whose `Info.plist`
//! declares a games category (`LSApplicationCategoryType` = `public.app-category.games` or
//! `public.app-category.<genre>-games`), plus [`KNOWN_GAME_BUNDLE_IDS`].
//!
//! The parsers are portable (unit-tested on every OS); only the folder list is macOS.

use std::path::{Path, PathBuf};

/// Game apps (and launchers that are the way to play one game) that ship without a games
/// category in their `Info.plist`. Extend when a popular game is missed; launchers of many
/// games (Steam, Epic, GOG Galaxy) are left out on purpose: their games are listed directly.
pub const KNOWN_GAME_BUNDLE_IDS: &[&str] = &[
    "com.mojang.minecraftlauncher",               // Minecraft Launcher
    "com.roblox.RobloxPlayer",                    // Roblox
    "com.riotgames.RiotGames.RiotClient",         // Riot Client (League of Legends, Valorant)
    "com.riotgames.LeagueofLegends.LeagueClient", // League of Legends client
    "net.battle.app",                             // Battle.net
    "net.battle.bootstrapper",                    // Battle.net (installer-era bundle id)
    "com.blizzard.worldofwarcraft",               // World of Warcraft
    "org.prismlauncher.PrismLauncher",            // Prism Launcher (Minecraft)
    "com.hypixel.HytaleLauncher",                 // Hytale launcher
];

/// Bundles never listed even if they declare a games category (launchers of other entries).
const EXCLUDED_BUNDLE_IDS: &[&str] = &[
    "com.valvesoftware.steam",
    "com.epicgames.EpicGamesLauncher",
    "com.gog.galaxy",
];

/// What the app needs from a bundle's `Info.plist`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BundleInfo {
    pub bundle_id: Option<String>,
    pub name: Option<String>,
    pub category: Option<String>,
    /// `CFBundleIconFile` (with or without `.icns`).
    pub icon_file: Option<String>,
}

/// Parses an `Info.plist` (XML or binary).
pub fn parse_info_plist(bytes: &[u8]) -> Option<BundleInfo> {
    let value = plist::Value::from_reader(std::io::Cursor::new(bytes)).ok()?;
    let dict = value.as_dictionary()?;
    let text = |key: &str| {
        dict.get(key)
            .and_then(plist::Value::as_string)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
    };
    Some(BundleInfo {
        bundle_id: text("CFBundleIdentifier"),
        name: text("CFBundleDisplayName").or_else(|| text("CFBundleName")),
        category: text("LSApplicationCategoryType"),
        icon_file: text("CFBundleIconFile"),
    })
}

/// `public.app-category.games` or any `public.app-category.*-games`.
pub fn is_games_category(category: &str) -> bool {
    let c = category.to_ascii_lowercase();
    c == "public.app-category.games"
        || (c.starts_with("public.app-category.") && c.ends_with("-games"))
}

pub fn is_game(info: &BundleInfo) -> bool {
    if let Some(id) = &info.bundle_id {
        if EXCLUDED_BUNDLE_IDS
            .iter()
            .any(|x| x.eq_ignore_ascii_case(id))
        {
            return false;
        }
        if KNOWN_GAME_BUNDLE_IDS
            .iter()
            .any(|x| x.eq_ignore_ascii_case(id))
        {
            return true;
        }
    }
    info.category.as_deref().is_some_and(is_games_category)
}

/// A game app found on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacGameApp {
    pub bundle: PathBuf,
    pub info: BundleInfo,
}

impl MacGameApp {
    pub fn title(&self) -> String {
        self.info
            .name
            .clone()
            .unwrap_or_else(|| bundle_stem(&self.bundle))
    }

    /// The `.icns` file of the bundle, if it names one that exists.
    pub fn icon_path(&self) -> Option<PathBuf> {
        icon_path(&self.bundle, &self.info)
    }
}

pub fn bundle_stem(bundle: &Path) -> String {
    bundle
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default()
}

pub fn icon_path(bundle: &Path, info: &BundleInfo) -> Option<PathBuf> {
    let file = info.icon_file.as_deref()?;
    let file = if Path::new(file).extension().is_some() {
        file.to_owned()
    } else {
        format!("{file}.icns")
    };
    let path = bundle.join("Contents/Resources").join(file);
    path.is_file().then_some(path)
}

/// Reads one bundle's `Info.plist`.
pub fn read_bundle(bundle: &Path) -> Option<BundleInfo> {
    let bytes = std::fs::read(bundle.join("Contents/Info.plist")).ok()?;
    parse_info_plist(&bytes)
}

fn is_app_bundle(path: &Path) -> bool {
    path.extension()
        .is_some_and(|x| x.eq_ignore_ascii_case("app"))
        && path.is_dir()
}

/// Game apps in `dirs`, looking one level into plain subfolders (e.g. `/Applications/Games`).
pub fn scan(dirs: &[PathBuf]) -> Vec<MacGameApp> {
    let mut out: Vec<MacGameApp> = Vec::new();
    let consider = |bundle: PathBuf, out: &mut Vec<MacGameApp>| {
        let Some(info) = read_bundle(&bundle) else {
            return;
        };
        if !is_game(&info) {
            return;
        }
        let duplicate = out.iter().any(|g| {
            g.bundle == bundle || (g.info.bundle_id.is_some() && g.info.bundle_id == info.bundle_id)
        });
        if !duplicate {
            out.push(MacGameApp { bundle, info });
        }
    };
    for dir in dirs {
        let Ok(read) = std::fs::read_dir(dir) else {
            continue;
        };
        for entry in read.flatten() {
            let path = entry.path();
            if is_app_bundle(&path) {
                consider(path, &mut out);
            } else if path.is_dir() && !entry.file_name().to_string_lossy().starts_with('.') {
                let Ok(inner) = std::fs::read_dir(&path) else {
                    continue;
                };
                for sub in inner.flatten() {
                    let sub = sub.path();
                    if is_app_bundle(&sub) {
                        consider(sub, &mut out);
                    }
                }
            }
        }
    }
    out.sort_by_key(|g| g.title().to_lowercase());
    out
}

/// The folders scanned on this computer.
pub fn app_dirs() -> Vec<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        let mut dirs = vec![PathBuf::from("/Applications")];
        if let Some(home) = std::env::var_os("HOME") {
            dirs.push(PathBuf::from(home).join("Applications"));
        }
        dirs
    }
    #[cfg(not(target_os = "macos"))]
    {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plist(id: &str, name: &str, category: Option<&str>) -> String {
        let cat = category
            .map(|c| format!("<key>LSApplicationCategoryType</key><string>{c}</string>"))
            .unwrap_or_default();
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleIdentifier</key><string>{id}</string>
<key>CFBundleName</key><string>{name}</string>
<key>CFBundleIconFile</key><string>AppIcon</string>
{cat}
</dict></plist>"#
        )
    }

    #[test]
    fn categories() {
        assert!(is_games_category("public.app-category.games"));
        assert!(is_games_category("public.app-category.arcade-games"));
        assert!(is_games_category("public.app-category.Role-Playing-Games"));
        assert!(!is_games_category("public.app-category.developer-tools"));
        assert!(!is_games_category("games"));
    }

    #[test]
    fn parses_info_plist_and_decides() {
        let info = parse_info_plist(
            plist(
                "com.robtop.gd",
                "Geometry Dash",
                Some("public.app-category.arcade-games"),
            )
            .as_bytes(),
        )
        .expect("plist");
        assert_eq!(info.bundle_id.as_deref(), Some("com.robtop.gd"));
        assert_eq!(info.name.as_deref(), Some("Geometry Dash"));
        assert!(is_game(&info));

        let roblox = parse_info_plist(plist("com.roblox.RobloxPlayer", "Roblox", None).as_bytes())
            .expect("plist");
        assert!(is_game(&roblox), "allowlisted without a category");

        let editor = parse_info_plist(
            plist(
                "com.microsoft.VSCode",
                "Code",
                Some("public.app-category.developer-tools"),
            )
            .as_bytes(),
        )
        .expect("plist");
        assert!(!is_game(&editor));

        let steam = parse_info_plist(
            plist(
                "com.valvesoftware.steam",
                "Steam",
                Some("public.app-category.games"),
            )
            .as_bytes(),
        )
        .expect("plist");
        assert!(!is_game(&steam), "launchers are excluded");
        assert!(parse_info_plist(b"garbage").is_none());
    }

    #[test]
    fn scans_fixture_folders_one_level_deep() {
        let root = std::env::temp_dir().join(format!("pcd-macapps-{}", std::process::id()));
        let make = |rel: &str, body: String| {
            let contents = root.join(rel).join("Contents");
            std::fs::create_dir_all(contents.join("Resources")).expect("mkdir");
            std::fs::write(contents.join("Info.plist"), body).expect("write");
        };
        make(
            "Game.app",
            plist("a.game", "A Game", Some("public.app-category.games")),
        );
        make(
            "Games/Inner.app",
            plist("b.inner", "Inner", Some("public.app-category.puzzle-games")),
        );
        make(
            "Tool.app",
            plist("c.tool", "Tool", Some("public.app-category.utilities")),
        );
        std::fs::write(root.join("Game.app/Contents/Resources/AppIcon.icns"), b"x").expect("w");

        let found = scan(std::slice::from_ref(&root));
        let titles: Vec<_> = found.iter().map(MacGameApp::title).collect();
        assert_eq!(titles, vec!["A Game", "Inner"]);
        assert!(found[0].icon_path().is_some());
        assert!(found[1].icon_path().is_none());
        let _ = std::fs::remove_dir_all(&root);
    }
}
