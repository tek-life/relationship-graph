use keyring::Entry;

const SERVICE_NAME: &str = "relationship-graph";
const ACCOUNT_NAME: &str = "database-key";

pub fn store_key(key_hex: &str) -> Result<(), String> {
    log::debug!(target: "keychain", "store_key_start");
    let entry = Entry::new(SERVICE_NAME, ACCOUNT_NAME).map_err(|e| e.to_string())?;
    entry.set_password(key_hex).map_err(|e| e.to_string())?;
    log::info!(target: "keychain", "store_key_success");
    Ok(())
}

pub fn get_key() -> Result<Option<String>, String> {
    log::debug!(target: "keychain", "get_key_start");
    let entry = Entry::new(SERVICE_NAME, ACCOUNT_NAME).map_err(|e| e.to_string())?;
    match entry.get_password() {
        Ok(password) => {
            log::info!(target: "keychain", "get_key_success found=true");
            Ok(Some(password))
        }
        Err(keyring::Error::NoEntry) => {
            log::info!(target: "keychain", "get_key_success found=false");
            Ok(None)
        }
        Err(error) => {
            log::warn!(target: "keychain", "get_key_failed error={}", error);
            Err(error.to_string())
        }
    }
}

pub fn delete_key() -> Result<(), String> {
    log::info!(target: "keychain", "delete_key_start");
    let entry = Entry::new(SERVICE_NAME, ACCOUNT_NAME).map_err(|e| e.to_string())?;
    match entry.delete_credential() {
        Ok(()) => {
            log::info!(target: "keychain", "delete_key_success existed=true");
            Ok(())
        }
        Err(keyring::Error::NoEntry) => {
            log::info!(target: "keychain", "delete_key_success existed=false");
            Ok(())
        }
        Err(error) => {
            log::warn!(target: "keychain", "delete_key_failed error={}", error);
            Err(error.to_string())
        }
    }
}
