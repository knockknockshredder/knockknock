// src-tauri/src/shredder/platform/common.rs

use rand::Rng;

/// Generate a random filename (16 chars, alphanumeric)
pub fn generate_random_name() -> String {
    let mut rng = rand::thread_rng();
    (0..16)
        .map(|_| {
            let idx = rng.gen_range(0..62);
            match idx {
                0..=9 => (b'0' + idx as u8) as char,
                10..=35 => (b'a' + (idx - 10) as u8) as char,
                36..=61 => (b'A' + (idx - 36) as u8) as char,
                _ => unreachable!(),
            }
        })
        .collect()
}
