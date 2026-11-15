//! `control::DocumentStore` over the app database. The control centers get their own
//! connection to the same SQLite file (WAL mode), so their writes never wait on the
//! `AppState` database lock held by history queries.

use std::path::Path;
use std::sync::Mutex;

use control::DocumentStore;
use models::AppError;
use persistence::Database;

pub struct DbDocumentStore(Mutex<Database>);

impl DbDocumentStore {
    pub fn open(path: &Path) -> Result<Self, AppError> {
        Ok(Self(Mutex::new(Database::open(path)?)))
    }

    #[cfg(test)]
    pub fn in_memory() -> Result<Self, AppError> {
        Ok(Self(Mutex::new(Database::open_in_memory()?)))
    }

    fn db(&self) -> Result<std::sync::MutexGuard<'_, Database>, AppError> {
        self.0
            .lock()
            .map_err(|_| AppError::internal("control store lock poisoned by an earlier panic"))
    }
}

impl DocumentStore for DbDocumentStore {
    fn load(&self, key: &str) -> Result<Option<String>, AppError> {
        Ok(self.db()?.load_control_document(key)?)
    }

    fn save(&self, key: &str, json: &str) -> Result<(), AppError> {
        Ok(self.db()?.save_control_document(key, json)?)
    }

    fn delete(&self, key: &str) -> Result<(), AppError> {
        Ok(self.db()?.delete_control_document(key)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn documents_persist_through_the_store() {
        let store = DbDocumentStore::in_memory().expect("open");
        store.save("gaming.library", "{}").expect("save");
        assert_eq!(
            store.load("gaming.library").expect("load").as_deref(),
            Some("{}")
        );
        store.delete("gaming.library").expect("delete");
        assert_eq!(store.load("gaming.library").expect("load"), None);
    }
}
