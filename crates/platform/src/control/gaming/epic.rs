//! Epic Games Launcher: one JSON `.item` manifest per installed app in
//! `EpicGamesLauncher/Data/Manifests`.

use std::path::{Path, PathBuf};

use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EpicManifest {
    pub app_name: String,
    pub display_name: String,
    pub install_location: PathBuf,
    pub install_size: Option<u64>,
    pub catalog_namespace: Option<String>,
    pub catalog_item_id: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct RawItem {
    app_name: Option<String>,
    display_name: Option<String>,
    install_location: Option<String>,
    install_size: Option<u64>,
    catalog_namespace: Option<String>,
    catalog_item_id: Option<String>,
    #[serde(default)]
    app_categories: Vec<String>,
    #[serde(rename = "bIsIncompleteInstall", default)]
    incomplete: bool,
}

/// Parses one `.item`. Skips incomplete installs and apps whose categories do not include
/// `games` (Unreal Engine, plugins).
pub fn parse_item(json: &str) -> Option<EpicManifest> {
    let raw: RawItem = serde_json::from_str(json).ok()?;
    if raw.incomplete {
        return None;
    }
    if !raw.app_categories.is_empty()
        && !raw
            .app_categories
            .iter()
            .any(|c| c.eq_ignore_ascii_case("games"))
    {
        return None;
    }
    let app_name = raw.app_name.filter(|s| !s.trim().is_empty())?;
    let display_name = raw
        .display_name
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| app_name.clone());
    let install_location = PathBuf::from(raw.install_location.filter(|s| !s.is_empty())?);
    Some(EpicManifest {
        app_name,
        display_name,
        install_location,
        install_size: raw.install_size.filter(|&s| s > 0),
        catalog_namespace: raw.catalog_namespace.filter(|s| !s.is_empty()),
        catalog_item_id: raw.catalog_item_id.filter(|s| !s.is_empty()),
    })
}

/// Launch URL. With the catalog ids it is the form Epic's own shortcuts use
/// (`<namespace>:<item>:<app>`, `:` encoded), otherwise the bare app name.
pub fn launch_url(m: &EpicManifest) -> String {
    let target = match (&m.catalog_namespace, &m.catalog_item_id) {
        (Some(ns), Some(item)) => format!(
            "{}%3A{}%3A{}",
            encode(ns),
            encode(item),
            encode(&m.app_name)
        ),
        _ => encode(&m.app_name),
    };
    format!("com.epicgames.launcher://apps/{target}?action=launch&silent=true")
}

/// Percent-encodes everything but unreserved URL characters.
fn encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// The manifests folder of this OS.
pub fn manifests_dir() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        let home = std::env::var_os("HOME")?;
        Some(
            PathBuf::from(home)
                .join("Library/Application Support/Epic/EpicGamesLauncher/Data/Manifests"),
        )
    }
    #[cfg(windows)]
    {
        let program_data =
            std::env::var_os("ProgramData").unwrap_or_else(|| "C:\\ProgramData".into());
        Some(PathBuf::from(program_data).join("Epic\\EpicGamesLauncher\\Data\\Manifests"))
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        None
    }
}

/// Every installed game whose manifest is in `dir`.
pub fn scan_games(dir: &Path) -> Vec<EpicManifest> {
    let Ok(read) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<EpicManifest> = read
        .flatten()
        .filter(|e| {
            e.path()
                .extension()
                .is_some_and(|x| x.eq_ignore_ascii_case("item"))
        })
        .filter_map(|e| std::fs::read_to_string(e.path()).ok())
        .filter_map(|t| parse_item(&t))
        .collect();
    out.sort_by(|a, b| a.app_name.cmp(&b.app_name));
    out.dedup_by(|a, b| a.app_name == b.app_name);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const ITEM: &str = r#"{
        "FormatVersion": 0,
        "bIsIncompleteInstall": false,
        "DisplayName": "Rocket League",
        "InstallLocation": "C:\\Program Files\\Epic Games\\rocketleague",
        "LaunchExecutable": "Binaries\\Win64\\RocketLeague.exe",
        "AppName": "Sugar",
        "CatalogNamespace": "9773aa1aa54f4f7b80e44bef04986cea",
        "CatalogItemId": "530145df28a24424923f5828cc9031a1",
        "AppCategories": ["public", "games", "applications"],
        "InstallSize": 21000000000
    }"#;

    #[test]
    fn parses_a_game_manifest_and_builds_the_launch_url() {
        let m = parse_item(ITEM).expect("item");
        assert_eq!(m.display_name, "Rocket League");
        assert_eq!(m.app_name, "Sugar");
        assert_eq!(m.install_size, Some(21_000_000_000));
        assert_eq!(
            launch_url(&m),
            "com.epicgames.launcher://apps/9773aa1aa54f4f7b80e44bef04986cea%3A530145df28a24424923f5828cc9031a1%3ASugar?action=launch&silent=true"
        );
        let bare = EpicManifest {
            catalog_namespace: None,
            ..m
        };
        assert_eq!(
            launch_url(&bare),
            "com.epicgames.launcher://apps/Sugar?action=launch&silent=true"
        );
    }

    #[test]
    fn skips_incomplete_non_games_and_garbage() {
        assert!(
            parse_item(&ITEM.replace(
                "\"bIsIncompleteInstall\": false",
                "\"bIsIncompleteInstall\": true"
            ))
            .is_none()
        );
        assert!(parse_item(&ITEM.replace("\"games\", ", "")).is_none());
        assert!(parse_item("not json").is_none());
        assert!(parse_item("{\"AppName\": \"x\"}").is_none());
    }
}
