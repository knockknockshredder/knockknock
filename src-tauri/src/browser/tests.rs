// src-tauri/src/browser/tests.rs

#[cfg(test)]
mod tests {
    use crate::browser::detection::{detect_browsers, detect_data_types, estimate_directory_size};
    use crate::browser::paths::{get_browser_base_paths, BROWSER_PATHS};
    use crate::browser::types::BrowserDataType;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[test]
    fn test_browser_paths_table_is_populated() {
        // Sanity check: ensure the static table has the major browsers
        let names: Vec<&str> = BROWSER_PATHS.iter().map(|b| b.name).collect();
        assert!(names.contains(&"Chrome"));
        assert!(names.contains(&"Firefox"));
        assert!(names.contains(&"Edge"));
        assert!(names.contains(&"Safari"));
    }

    #[test]
    fn test_safari_only_has_macos() {
        let safari = BROWSER_PATHS.iter().find(|b| b.name == "Safari").unwrap();
        assert!(safari.windows_paths.is_empty());
        assert!(safari.linux_paths.is_empty());
        assert!(!safari.macos_paths.is_empty());
    }

    #[test]
    fn test_get_browser_base_paths_returns_vec() {
        // For each browser, the function should at least not panic and return a Vec.
        // On platforms where the env var is missing, it should return an empty vec.
        for browser in BROWSER_PATHS {
            let paths = get_browser_base_paths(browser);
            // We don't assert on the content since it depends on the host environment.
            let _: Vec<PathBuf> = paths;
        }
    }

    #[test]
    fn test_detect_browsers_runs_without_panic() {
        // detect_browsers should never panic; it may return an empty vec.
        let _ = detect_browsers();
    }

    #[test]
    fn test_detect_data_types_finds_profile_in_real_dir() {
        let tmp = TempDir::new().unwrap();
        let profile = tmp.path();
        // Create a profile directory with a few recognizable artifacts.
        fs::create_dir_all(profile.join("Cache")).unwrap();
        fs::write(profile.join("Cookies"), b"cookies").unwrap();
        fs::write(profile.join("History"), b"history").unwrap();
        fs::write(profile.join("Login Data"), b"logins").unwrap();

        let types = detect_data_types(profile);
        assert!(types.contains(&BrowserDataType::Profile));
        assert!(types.contains(&BrowserDataType::Cache));
        assert!(types.contains(&BrowserDataType::Cookies));
        assert!(types.contains(&BrowserDataType::History));
        assert!(types.contains(&BrowserDataType::Passwords));
    }

    #[test]
    fn test_detect_data_types_finds_firefox_layout() {
        let tmp = TempDir::new().unwrap();
        let profile = tmp.path();
        fs::create_dir_all(profile.join("cache2")).unwrap();
        fs::write(profile.join("cookies.sqlite"), b"cookies").unwrap();
        fs::write(profile.join("places.sqlite"), b"places").unwrap();
        fs::write(profile.join("logins.json"), b"{}").unwrap();

        let types = detect_data_types(profile);
        assert!(types.contains(&BrowserDataType::Cache));
        assert!(types.contains(&BrowserDataType::Cookies));
        assert!(types.contains(&BrowserDataType::History));
        assert!(types.contains(&BrowserDataType::Passwords));
    }

    #[test]
    fn test_detect_data_types_empty_for_nonexistent() {
        let types = detect_data_types(std::path::Path::new("/nonexistent/path/zzz"));
        // No data types should be reported for a non-existent path
        assert!(types.is_empty());
    }

    #[test]
    fn test_estimate_directory_size_counts_files() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path();

        fs::write(dir.join("a.bin"), vec![0u8; 100]).unwrap();
        fs::write(dir.join("b.bin"), vec![0u8; 250]).unwrap();

        let nested = dir.join("nested");
        fs::create_dir_all(&nested).unwrap();
        fs::write(nested.join("c.bin"), vec![0u8; 500]).unwrap();

        let total = estimate_directory_size(dir);
        assert_eq!(total, 100 + 250 + 500);
    }

    #[test]
    fn test_estimate_directory_size_empty_dir_is_zero() {
        let tmp = TempDir::new().unwrap();
        assert_eq!(estimate_directory_size(tmp.path()), 0);
    }

    #[test]
    fn test_estimate_directory_size_nonexistent_is_zero() {
        let total = estimate_directory_size(std::path::Path::new("/nonexistent/zzz/yyy"));
        assert_eq!(total, 0);
    }
}
