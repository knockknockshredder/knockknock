// src-tauri/src/shredder/logging.rs

use std::path::Path;

/// Log obfuscation modes (user-configurable)
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LogObfuscation {
    /// No obfuscation — full paths in logs
    None,
    /// Numbered — File_1, File_2, etc.
    Numbered,
    /// Partial mask — first 3 + last 5 chars with *** in between
    PartialMask,
}

/// Obfuscate a file path for logging
pub fn obfuscate_path(path: &Path, mode: LogObfuscation, index: usize) -> String {
    match mode {
        LogObfuscation::None => path.to_string_lossy().to_string(),
        LogObfuscation::Numbered => format!("File_{}", index),
        LogObfuscation::PartialMask => {
            let s = path.to_string_lossy();
            if s.len() <= 8 {
                "***".to_string()
            } else {
                format!("{}***{}", &s[..3], &s[s.len() - 5..])
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn none_returns_full_path() {
        let p = Path::new("/home/user/secret.txt");
        assert_eq!(
            obfuscate_path(p, LogObfuscation::None, 1),
            "/home/user/secret.txt"
        );
    }

    #[test]
    fn numbered_returns_index_label() {
        let p = Path::new("/home/user/secret.txt");
        assert_eq!(obfuscate_path(p, LogObfuscation::Numbered, 42), "File_42");
    }

    #[test]
    fn partial_mask_short_path_returns_stars() {
        let p = Path::new("a.txt");
        assert_eq!(obfuscate_path(p, LogObfuscation::PartialMask, 1), "***");
    }

    #[test]
    fn partial_mask_long_path_masks_middle() {
        let p = Path::new("/home/user/secret.txt");
        // len > 8, so first 3 + *** + last 5 = "/ho" + "***" + "t.txt"
        let result = obfuscate_path(p, LogObfuscation::PartialMask, 0);
        assert!(result.starts_with("/ho"));
        assert!(result.contains("***"));
        assert!(result.ends_with("t.txt"));
    }
}
