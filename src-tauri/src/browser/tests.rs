// src-tauri/src/browser/tests.rs

#[cfg(test)]
mod tests {
    use crate::browser::detection::{detect_browsers, estimate_directory_size};
    use crate::browser::paths::{
        find_browser_profiles, get_browser_base_paths, windows_base_paths_for_roots,
        BrowserRunningDetection, ProfileLayout, WindowsRootPreference, BROWSER_PATHS,
    };
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
    fn firefox_windows_root_preference_orders_appdata_first() {
        let local_root = TempDir::new().expect("local app data root");
        let roaming_root = TempDir::new().expect("app data root");
        let firefox = BROWSER_PATHS
            .iter()
            .find(|browser| browser.id == "firefox")
            .expect("Firefox browser configuration");

        assert_eq!(
            firefox.windows_root_preference,
            WindowsRootPreference::AppDataFirst
        );

        let roaming_base = roaming_root.path().join("Mozilla\\Firefox");
        let local_base = local_root.path().join("Mozilla\\Firefox");
        let base_paths = windows_base_paths_for_roots(
            firefox,
            Some(local_root.path()),
            Some(roaming_root.path()),
        );

        assert_eq!(base_paths, vec![roaming_base.clone(), local_base.clone()]);

        let roaming_profile_root = match firefox.profile_layout {
            ProfileLayout::Direct => roaming_base,
            ProfileLayout::Subdirectory(directory) => roaming_base.join(directory),
        };
        let local_profile_root = match firefox.profile_layout {
            ProfileLayout::Direct => local_base,
            ProfileLayout::Subdirectory(directory) => local_base.join(directory),
        };
        let roaming_profile = roaming_profile_root.join("roaming.default-release");
        fs::create_dir_all(&roaming_profile).expect("create roaming Firefox profile");
        fs::create_dir_all(local_profile_root.join("local.default-release"))
            .expect("create local Firefox profile");

        let first_profile = base_paths
            .iter()
            .find_map(|base_path| find_browser_profiles(base_path, firefox).into_iter().next());
        assert_eq!(first_profile, Some(roaming_profile));
    }

    #[test]
    fn chrome_windows_root_preference_orders_local_app_data_first() {
        let local_root = TempDir::new().expect("local app data root");
        let roaming_root = TempDir::new().expect("app data root");
        let chrome = BROWSER_PATHS
            .iter()
            .find(|browser| browser.id == "chrome")
            .expect("Chrome browser configuration");

        assert_eq!(
            chrome.windows_root_preference,
            WindowsRootPreference::LocalAppDataFirst
        );

        let base_paths = windows_base_paths_for_roots(
            chrome,
            Some(local_root.path()),
            Some(roaming_root.path()),
        );

        assert_eq!(
            base_paths,
            vec![
                local_root.path().join("Google\\Chrome\\User Data"),
                roaming_root.path().join("Google\\Chrome\\User Data"),
                local_root.path().join("Google\\Chrome Beta\\User Data"),
                roaming_root.path().join("Google\\Chrome Beta\\User Data"),
                local_root.path().join("Google\\Chrome SxS\\User Data"),
                roaming_root.path().join("Google\\Chrome SxS\\User Data"),
            ]
        );
    }

    #[test]
    fn test_detect_browsers_runs_without_panic() {
        // detect_browsers should never panic; it may return an empty vec.
        let _ = detect_browsers();
    }

    #[test]
    fn unsupported_or_deferred_browsers_are_not_exposed() {
        let names: Vec<&str> = BROWSER_PATHS.iter().map(|browser| browser.name).collect();
        assert!(!names.contains(&"Internet Explorer"));
        assert!(!names.contains(&"Safari"));

        assert!(BROWSER_PATHS.iter().all(|browser| {
            matches!(
                browser.running_detection,
                BrowserRunningDetection::ChromiumUserData | BrowserRunningDetection::GeckoProfile
            )
        }));
    }

    #[cfg(any(target_os = "windows", target_os = "macos"))]
    #[test]
    fn firefox_profiles_are_discovered_under_profiles_directory() {
        let tmp = TempDir::new().expect("temporary directory");
        let base = tmp.path().join("Mozilla/Firefox");
        let profile = base.join("Profiles/abc.default-release");
        fs::create_dir_all(&profile).expect("create Firefox profile");

        let firefox = BROWSER_PATHS
            .iter()
            .find(|browser| browser.id == "firefox")
            .expect("Firefox browser configuration");
        assert!(matches!(
            firefox.profile_layout,
            ProfileLayout::Subdirectory("Profiles")
        ));
        let profiles = find_browser_profiles(&base, firefox);

        assert_eq!(profiles, vec![profile]);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn firefox_profiles_are_discovered_as_direct_children_on_linux() {
        let tmp = TempDir::new().expect("temporary directory");
        let base = tmp.path().join(".mozilla/firefox");
        let profile = base.join("abc.default-release");
        fs::create_dir_all(&profile).expect("create Firefox profile");

        let firefox = BROWSER_PATHS
            .iter()
            .find(|browser| browser.id == "firefox")
            .expect("Firefox browser configuration");
        assert!(matches!(firefox.profile_layout, ProfileLayout::Direct));
        let profiles = find_browser_profiles(&base, firefox);

        assert_eq!(profiles, vec![profile]);
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
