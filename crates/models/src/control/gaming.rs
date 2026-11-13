//! Gaming center contracts. Owned by the gaming feature.
//!
//! The library is what the local launchers and the system report (Steam and Epic manifests,
//! macOS game apps, GOG on Windows) plus entries the user added. Nothing is invented: a value
//! the source does not record (size, last played, art) is `None` / `false`.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{ControlDomainId, ProfileRef, Support};

/// Tauri event emitted when a game session starts or ends (payload: [`GamingSessionEvent`]).
pub const GAMING_SESSION_EVENT: &str = "gaming:session";

/// Where a library entry comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum GameSource {
    Steam,
    Epic,
    /// GOG Galaxy installs (Windows, read from the registry).
    Gog,
    /// A macOS app whose bundle declares a games category, or a known game bundle id.
    MacApp,
    /// Added by the user (an app bundle or an executable).
    Manual,
}

impl GameSource {
    /// Prefix of the stable entry id (`steam:322170`).
    pub fn id_prefix(self) -> &'static str {
        match self {
            Self::Steam => "steam",
            Self::Epic => "epic",
            Self::Gog => "gog",
            Self::MacApp => "macApp",
            Self::Manual => "manual",
        }
    }
}

/// Artwork of a game, from the launcher's local cache or the app bundle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum GameArtKind {
    /// Portrait cover (2:3, Steam `library_600x900`).
    Cover,
    /// Wide background (Steam `library_hero`).
    Hero,
    /// Transparent title logo (Steam `logo.png`).
    Logo,
    /// App icon (macOS bundle `.icns`, largest PNG inside).
    Icon,
}

/// One game of the library.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct GameEntry {
    /// Stable id: `<source>:<source id>` (`steam:322170`, `macApp:com.roblox.RobloxPlayer`).
    pub id: String,
    pub title: String,
    pub source: GameSource,
    /// Install folder, bundle or executable (shown in Advanced only).
    pub install_path: Option<String>,
    /// Size on disk as the launcher records it.
    #[ts(type = "number | null")]
    pub size_bytes: Option<u64>,
    /// Last time the launcher saw it played, Unix ms.
    #[ts(type = "number | null")]
    pub last_played: Option<u64>,
    pub has_cover: bool,
    pub has_hero: bool,
    pub has_logo: bool,
    pub has_icon: bool,
    pub hidden: bool,
    /// Whether the app can start it, and why not.
    pub launch: Support,
    /// The id the source uses (Steam app id, Epic app name, bundle id), Advanced only.
    pub source_id: String,
}

/// Detection of one source on this computer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct GameSourceStatus {
    pub source: GameSource,
    /// Whether the app can read this source here (`Unsupported` e.g. GOG on macOS).
    pub support: Support,
    /// The launcher or folder exists.
    pub found: bool,
    /// Visible (not hidden) entries from this source.
    pub count: u32,
    /// Where the app looked (Advanced only).
    pub location: Option<String>,
}

/// Profiles of the other centers linked to one game.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct GameProfile {
    pub game_id: String,
    /// Off: the links are kept but nothing is applied when the game runs.
    pub enabled: bool,
    pub performance: Option<String>,
    pub cooling: Option<String>,
    pub lighting: Option<String>,
}

impl GameProfile {
    pub fn profile_for(&self, domain: ControlDomainId) -> Option<&str> {
        match domain {
            ControlDomainId::Performance => self.performance.as_deref(),
            ControlDomainId::Cooling => self.cooling.as_deref(),
            ControlDomainId::Lighting => self.lighting.as_deref(),
        }
    }

    pub fn has_links(&self) -> bool {
        self.performance.is_some() || self.cooling.is_some() || self.lighting.is_some()
    }
}

/// How a session started.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum GameSessionTrigger {
    /// The user pressed "Gioca con profilo".
    Launch,
    /// Game mode found the game running.
    Detected,
}

/// Outcome of applying one linked profile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct GameDomainResult {
    pub domain: ControlDomainId,
    pub profile_id: String,
    /// Name of the profile when the center still lists it.
    pub profile_name: Option<String>,
    pub applied: bool,
    /// `Supported` when applied; otherwise why not, in the user's language.
    pub support: Support,
}

/// The game being played.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct GameSession {
    pub game_id: String,
    pub title: String,
    /// Unix ms.
    #[ts(type = "number")]
    pub since: u64,
    pub trigger: GameSessionTrigger,
    /// Whether the game's process has been seen running (a launch may take a while).
    pub detected: bool,
    /// One per linked profile (empty without an enabled game profile).
    pub domains: Vec<GameDomainResult>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct GameModeState {
    /// Detect running games and apply their profiles automatically.
    pub enabled: bool,
    pub session: Option<GameSession>,
}

/// Everything the Gaming page shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct GamingState {
    pub library: Vec<GameEntry>,
    pub sources: Vec<GameSourceStatus>,
    pub profiles: Vec<GameProfile>,
    /// Profiles of Performance, Cooling and Lighting a game can link.
    pub linkable: Vec<ProfileRef>,
    /// The centers connected to Gaming (a missing one cannot apply anything).
    pub domains: Vec<ControlDomainId>,
    pub game_mode: GameModeState,
    /// Time of the library scan, Unix ms.
    #[ts(type = "number")]
    pub scanned_at: u64,
}

/// Why a session ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum GameSessionEndReason {
    /// The game's process is gone.
    Exited,
    /// The user pressed Stop / Ripristina.
    Stopped,
    /// A launched game was never seen running.
    NotDetected,
    /// The app is closing.
    Shutdown,
}

/// Payload of [`GAMING_SESSION_EVENT`]. The page re-reads the full state on it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct GamingSessionEvent {
    pub game_id: String,
    pub active: bool,
    pub reason: Option<GameSessionEndReason>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_lookup_by_domain() {
        let p = GameProfile {
            game_id: "steam:1".into(),
            enabled: true,
            performance: Some("perf".into()),
            cooling: None,
            lighting: Some("rgb".into()),
        };
        assert_eq!(p.profile_for(ControlDomainId::Performance), Some("perf"));
        assert_eq!(p.profile_for(ControlDomainId::Cooling), None);
        assert!(p.has_links());
        assert!(!GameProfile::default().has_links());
    }

    #[test]
    fn sources_serialize_camel_case() {
        let json = serde_json::to_value(GameSource::MacApp).expect("serialize");
        assert_eq!(json, "macApp");
        assert_eq!(GameSource::MacApp.id_prefix(), "macApp");
    }
}
