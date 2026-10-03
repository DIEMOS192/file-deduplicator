use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

/// How many bytes from the start of a file the partial hash covers.
pub const PARTIAL_HASH_BYTES: u64 = 16 * 1024;

pub type Hash = [u8; 32];

/// Hash only the first [`PARTIAL_HASH_BYTES`] of a file.
pub fn partial_hash(path: &Path) -> io::Result<Hash> {
    let file = File::open(path)?;
    let mut hasher = blake3::Hasher::new();
    io::copy(&mut file.take(PARTIAL_HASH_BYTES), &mut hasher)?;
    Ok(*hasher.finalize().as_bytes())
}

/// Hash the entire contents of a file.
pub fn full_hash(path: &Path) -> io::Result<Hash> {
    let mut hasher = blake3::Hasher::new();
    hasher.update_reader(File::open(path)?)?;
    Ok(*hasher.finalize().as_bytes())
}

/// Lowercase hex representation of a hash, for display.
pub fn to_hex(hash: &Hash) -> String {
    blake3::Hash::from_bytes(*hash).to_hex().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partial_hash_ignores_bytes_past_the_prefix() {
        let dir = tempfile::tempdir().unwrap();
        let prefix = vec![7u8; PARTIAL_HASH_BYTES as usize];
        let a = dir.path().join("a");
        let b = dir.path().join("b");
        std::fs::write(&a, [prefix.as_slice(), b"tail-one"].concat()).unwrap();
        std::fs::write(&b, [prefix.as_slice(), b"tail-two"].concat()).unwrap();

        assert_eq!(partial_hash(&a).unwrap(), partial_hash(&b).unwrap());
        assert_ne!(full_hash(&a).unwrap(), full_hash(&b).unwrap());
    }
}
