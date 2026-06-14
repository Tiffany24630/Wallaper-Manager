use sha2::{Digest, Sha256};
use std::fs;

pub fn file_hash(
    path: &str,
) -> Result<String,String> {
    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();

    hasher.update(bytes);

    Ok(format!("{:x}", hasher.finalize()))
}