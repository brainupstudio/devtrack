//! Credentials live in the operating system's keychain (macOS Keychain, Windows Credential
//! Manager, Linux kernel keyring), never in DevTrack's data files or its sync folder.

//!
//! The keychain is read once per credential while DevTrack runs; after that the value comes from
//! memory. On macOS every read of an item by an app the person only "Allowed" (not "Always
//! allowed") asks for their password again, and the Connections page and the work item finder
//! ask whether a token exists on every frame they draw.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

const SERVICE: &str = "DevTrack";

/// What the keychain answered, by credential: `None` when it holds nothing for it.
fn cache() -> &'static Mutex<HashMap<String, Option<String>>> {
    static CACHE: OnceLock<Mutex<HashMap<String, Option<String>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn set(integration_id: &str, secret: &str) -> Result<(), String> {
    let entry = keyring::Entry::new(SERVICE, integration_id).map_err(|e| e.to_string())?;
    entry.set_password(secret).map_err(|e| e.to_string())?;
    remember(integration_id, Some(secret.to_string()));
    Ok(())
}

pub fn get(integration_id: &str) -> Option<String> {
    if let Some(known) = cache().lock().ok().and_then(|c| c.get(integration_id).cloned()) {
        return known;
    }
    let read = keyring::Entry::new(SERVICE, integration_id).ok().and_then(|e| e.get_password().ok());
    remember(integration_id, read.clone());
    read
}

pub fn has(integration_id: &str) -> bool {
    get(integration_id).is_some()
}

pub fn remove(integration_id: &str) {
    if let Ok(e) = keyring::Entry::new(SERVICE, integration_id) {
        let _ = e.delete_credential();
    }
    remember(integration_id, None);
}

fn remember(integration_id: &str, value: Option<String>) {
    if let Ok(mut c) = cache().lock() {
        c.insert(integration_id.to_string(), value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// After the first answer the keychain is not asked again: a value set, a removal, or what a
    /// read found is served from memory.
    #[test]
    fn answers_come_from_memory_after_the_first() {
        remember("test-cached", Some("tok".into()));
        assert_eq!(get("test-cached").as_deref(), Some("tok"));
        assert!(has("test-cached"));
        remember("test-absent", None);
        assert_eq!(get("test-absent"), None);
        assert!(!has("test-absent"));
    }
}
