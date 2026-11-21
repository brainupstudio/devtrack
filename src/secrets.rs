//! Credentials live in the operating system's keychain (macOS Keychain, Windows Credential
//! Manager, Linux kernel keyring), never in DevTrack's data files or its sync folder.

const SERVICE: &str = "DevTrack";

pub fn set(integration_id: &str, secret: &str) -> Result<(), String> {
    let entry = keyring::Entry::new(SERVICE, integration_id).map_err(|e| e.to_string())?;
    entry.set_password(secret).map_err(|e| e.to_string())
}

pub fn get(integration_id: &str) -> Option<String> {
    keyring::Entry::new(SERVICE, integration_id).ok().and_then(|e| e.get_password().ok())
}

pub fn has(integration_id: &str) -> bool {
    get(integration_id).is_some()
}

pub fn remove(integration_id: &str) {
    if let Ok(e) = keyring::Entry::new(SERVICE, integration_id) {
        let _ = e.delete_credential();
    }
}
