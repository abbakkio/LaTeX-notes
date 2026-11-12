use crate::error::{CoreError, Result};

const SERVICE_NAME: &str = "dev.latexnotes.app";

pub fn set_secret(account: &str, value: &str) -> Result<()> {
    let entry = keyring::Entry::new(SERVICE_NAME, account)
        .map_err(|e| CoreError::Keyring(e.to_string()))?;
    entry
        .set_password(value)
        .map_err(|e| CoreError::Keyring(e.to_string()))
}

pub fn get_secret(account: &str) -> Result<Option<String>> {
    let entry = keyring::Entry::new(SERVICE_NAME, account)
        .map_err(|e| CoreError::Keyring(e.to_string()))?;
    match entry.get_password() {
        Ok(val) => Ok(Some(val)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(CoreError::Keyring(e.to_string())),
    }
}

pub fn delete_secret(account: &str) -> Result<()> {
    let entry = keyring::Entry::new(SERVICE_NAME, account)
        .map_err(|e| CoreError::Keyring(e.to_string()))?;
    match entry.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(CoreError::Keyring(e.to_string())),
    }
}
