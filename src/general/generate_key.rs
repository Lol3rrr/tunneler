use rand::rngs::OsRng;
use rand::RngCore;

use std::collections::hash_map::DefaultHasher;
use std::hash::Hasher;

/// Generates a random Key with size bytes length
///
/// Params:
/// * size: The size in bytes of the key to generate
pub fn generate_key(size: u64) -> Vec<u8> {
    let mut result: Vec<u8> = Vec::new();
    let mut rng = OsRng;

    let mut rand_numb: u64 = rng.next_u64();
    let mut hasher = DefaultHasher::new();
    for _ in 0..size {
        hasher.write_u64(rand_numb);
        hasher.write_u64(rng.next_u64());
        let hash = hasher.finish();

        let hash_bytes = hash.to_le_bytes();
        result.push(hash_bytes[1]);
        result.push(hash_bytes[5]);

        rand_numb = hash;
    }

    result
}
