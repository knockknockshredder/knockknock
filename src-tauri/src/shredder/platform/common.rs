// src-tauri/src/shredder/platform/common.rs

use rand::Rng;

/// Windows reserved device names that must never be used as filenames.
/// Generating one of these can cause silent filesystem failures on Windows.
const RESERVED_NAMES: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Generate a random filename (16 chars, alphanumeric).
/// Retries until the result is not a Windows reserved name.
pub fn generate_random_name() -> String {
    let mut rng = rand::thread_rng();
    loop {
        let name: String = (0..16)
            .map(|_| {
                let idx = rng.gen_range(0..62);
                match idx {
                    0..=9 => (b'0' + idx as u8) as char,
                    10..=35 => (b'a' + (idx - 10) as u8) as char,
                    36..=61 => (b'A' + (idx - 36) as u8) as char,
                    _ => unreachable!(),
                }
            })
            .collect();

        // Check against reserved names (exact match or with extension).
        // The 16-char generator never produces a ".", so only exact-match
        // matters here — but the dot check is kept for defense-in-depth.
        let upper = name.to_uppercase();
        let is_reserved = RESERVED_NAMES
            .iter()
            .any(|&r| upper == r || upper.starts_with(&format!("{}.", r)));

        if !is_reserved {
            return name;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_16_chars() {
        let n = generate_random_name();
        assert_eq!(n.len(), 16);
    }

    #[test]
    fn generates_alphanumeric() {
        for _ in 0..100 {
            let n = generate_random_name();
            assert!(
                n.chars().all(|c| c.is_ascii_alphanumeric()),
                "Non-alphanumeric char in {:?}",
                n
            );
        }
    }

    #[test]
    fn never_returns_reserved_name() {
        for _ in 0..1000 {
            let n = generate_random_name();
            let upper = n.to_uppercase();
            assert!(
                !RESERVED_NAMES.contains(&upper.as_str()),
                "Generated reserved name {:?}",
                n
            );
        }
    }
}
