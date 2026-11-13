//! Control centers: Performance, Cooling, Lighting and Gaming.
//!
//! Each center is a controller (one module, owned by its feature) that keeps the user's
//! profiles, knows what the computer supports (through `platform::control::<center>`) and
//! applies changes only when the user asks. OS access stays in the `platform` crate.
//!
//! Shared rules (docs/control-centers.md):
//! - A control the system does not let the app change is reported `ReadOnly` or
//!   `Unsupported` with a reason, never simulated.
//! - Before the first change the controller records the original state, so "Ripristina"
//!   can bring it back, even after a restart.
//! - No admin rights, no driver, firmware, BIOS or registry writes.
//!
//! Two layers of state per center: the user's own selection, and an optional override set
//! by Gaming while a game with a linked profile runs (`ProfileDomain`). Removing the
//! override brings back the user's selection, not the original system state.

#![forbid(unsafe_code)]

use std::sync::Arc;

use models::AppError;
use models::control::{ControlDomainId, ProfileRef};
use models::settings::Language;

pub mod cooling;
pub mod gaming;
pub mod lighting;
pub mod performance;

/// Persistent JSON documents (one per key), backed by the app database. Each center owns
/// the keys that start with its name, e.g. `performance.profiles`.
pub trait DocumentStore: Send + Sync {
    fn load(&self, key: &str) -> Result<Option<String>, AppError>;
    fn save(&self, key: &str, json: &str) -> Result<(), AppError>;
    fn delete(&self, key: &str) -> Result<(), AppError>;
}

/// Loads a JSON document, falling back to `T::default()` when it is missing or no longer
/// deserializes (logged, never fatal).
pub fn load_json<T>(store: &dyn DocumentStore, key: &str) -> T
where
    T: serde::de::DeserializeOwned + Default,
{
    match store.load(key) {
        Ok(Some(raw)) => serde_json::from_str(&raw).unwrap_or_else(|e| {
            tracing::warn!(key, error = %e, "stored control document invalid, using defaults");
            T::default()
        }),
        Ok(None) => T::default(),
        Err(e) => {
            tracing::warn!(key, error = %e.message, "control document unreadable, using defaults");
            T::default()
        }
    }
}

pub fn save_json<T: serde::Serialize>(
    store: &dyn DocumentStore,
    key: &str,
    value: &T,
) -> Result<(), AppError> {
    let json = serde_json::to_string(value).map_err(|e| {
        AppError::internal("could not encode control document").with_detail(e.to_string())
    })?;
    store.save(key, &json)
}

/// A center whose profiles a game profile can link (Performance, Cooling, Lighting).
pub trait ProfileDomain: Send + Sync {
    fn domain(&self) -> ControlDomainId;

    /// Every profile, built-in and user-made, named in `lang`.
    fn profile_refs(&self, lang: Language) -> Vec<ProfileRef>;

    /// Applies `profile_id` as the game override. The user's own selection is kept and
    /// comes back with `clear_game_override`. Applying while an override is active replaces
    /// it. Fails with `NotFound` for an unknown id and `Unsupported` when nothing in the
    /// profile can be applied here.
    fn apply_game_override(&self, profile_id: &str) -> Result<(), AppError>;

    /// Removes the game override and re-applies the user's selection. No-op without one.
    fn clear_game_override(&self) -> Result<(), AppError>;
}

pub type SharedDomain = Arc<dyn ProfileDomain>;

/// In-memory `DocumentStore` for tests.
#[derive(Default)]
pub struct MemoryStore(std::sync::Mutex<std::collections::HashMap<String, String>>);

impl DocumentStore for MemoryStore {
    fn load(&self, key: &str) -> Result<Option<String>, AppError> {
        Ok(self
            .0
            .lock()
            .map_err(|_| AppError::internal("store lock poisoned"))?
            .get(key)
            .cloned())
    }

    fn save(&self, key: &str, json: &str) -> Result<(), AppError> {
        self.0
            .lock()
            .map_err(|_| AppError::internal("store lock poisoned"))?
            .insert(key.to_owned(), json.to_owned());
        Ok(())
    }

    fn delete(&self, key: &str) -> Result<(), AppError> {
        self.0
            .lock()
            .map_err(|_| AppError::internal("store lock poisoned"))?
            .remove(key);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_documents_round_trip_and_fall_back_to_default() {
        let store = MemoryStore::default();
        let empty: Vec<u32> = load_json(&store, "x.list");
        assert!(empty.is_empty());
        save_json(&store, "x.list", &vec![1u32, 2]).expect("save");
        assert_eq!(load_json::<Vec<u32>>(&store, "x.list"), vec![1, 2]);
        store.save("x.list", "not json").expect("save");
        assert!(load_json::<Vec<u32>>(&store, "x.list").is_empty());
    }
}
