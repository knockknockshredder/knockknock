// src-tauri/src/lib.rs

mod browser;
mod commands;
mod drive;
mod paths;
mod pin;
mod shredder;
mod tray;
mod vault;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Validate portable data directory BEFORE Tauri builder — if the
    // exe is in an unwritable location, surface an error and exit
    // instead of crashing mid-startup.
    let data_dir = match crate::paths::portable_data_dir() {
        Ok(d) => d,
        Err(msg) => {
            startup_fatal(&msg);
        }
    };

    // Restore persisted PIN lockout state AFTER data dir validation —
    // init_lockout_state() writes to a path derived from the portable
    // dir. If portable dir failed, we already exited above.
    if let Err(e) = pin::init_lockout_state() {
        eprintln!("[KnockKnock] failed to load PIN lockout state: {}", e);
    }

    // Create webview data subdirectory. Failure is fatal — without
    // it, WebView will silently fall back to OS-managed location.
    let webview_dir = match std::fs::create_dir_all(data_dir.join("webview")) {
        Ok(_) => data_dir.join("webview"),
        Err(e) => {
            startup_fatal(&format!("Failed to create webview data dir: {e}"));
        }
    };

    // Windows: set WebView2 user data folder via env var (must be
    // set BEFORE Builder::run() — WebView2 reads it once).
    #[cfg(target_os = "windows")]
    {
        std::env::set_var(
            "WEBVIEW2_USER_DATA_FOLDER",
            webview_dir.to_string_lossy().as_ref(),
        );
    }

    #[cfg_attr(not(target_os = "linux"), allow(unused_variables))]
    let webview_dir_clone = webview_dir.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(move |app| {
            // Tray setup is non-essential — failure shouldn't crash startup.
            if let Err(e) = tray::setup_tray(app.handle()) {
                eprintln!("[KnockKnock] Tray setup failed (non-fatal): {e}");
            }

            // Linux: close the auto-created window from tauri.conf.json
            // and recreate it with the portable webview data dir.
            #[cfg(target_os = "linux")]
            {
                create_main_window_linux(app, &webview_dir_clone)?;
            }

            // Windows/macOS: window already created by tauri.conf.json.
            // Windows: WEBVIEW2_USER_DATA_FOLDER env var redirects WebView2.
            // macOS:    WKWebView uses fixed path (documented limitation).

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::shred::shred_files,
            commands::shred::cancel_shred,
            commands::shred::cleanup_orphans,
            commands::shred::get_algorithms,
            commands::shred::validate_paths,
            commands::shred::get_drive_info,
            commands::shred::get_all_drive_info,
            commands::shred::request_elevation,
            commands::browser::detect_browsers,
            commands::browser::shred_browser_data,
            commands::tray::quick_shred_from_clipboard,
            commands::tray::minimize_to_tray,
            commands::pin::setup_pin,
            commands::pin::verify_pin,
            commands::pin::is_pin_enabled,
            commands::pin::set_pin_enabled,
            commands::pin::has_pin,
            commands::pin::is_pin_locked,
            commands::pin::get_lockout_remaining,
            commands::pin::change_pin,
            commands::pin::reset_app,
            commands::pin::disable_pin,
            commands::vault::save_vault,
            commands::vault::load_vault,
            commands::vault::clear_vault,
            commands::vault::vault_exists,
            commands::settings::get_settings,
            commands::settings::save_settings,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Show a fatal startup error via native-dialog AND write a log
/// to the OS temp dir (which is always writable). Always exits.
fn startup_fatal(msg: &str) -> ! {
    let _ = native_dialog::DialogBuilder::message()
        .set_title("KnockKnock — Startup Error")
        .set_text(&format!(
            "{}\n\nTip: KnockKnock is portable — move the app to a\n\
             writable folder (e.g. Desktop, Documents, or ~/Applications).",
            msg
        ))
        .set_level(native_dialog::MessageLevel::Error)
        .alert()
        .show();

    // Fallback log file in OS temp dir — guaranteed writable even if
    // the exe dir is read-only or CWD is a network mount.
    let log_path = std::env::temp_dir().join("knockknock-startup-error.log");
    if let Ok(mut f) = std::fs::File::create(&log_path) {
        use std::io::Write;
        let _ = writeln!(f, "KnockKnock failed to start:\n\n{msg}");
        let _ = writeln!(f, "\nLog path: {}", log_path.display());
    }

    std::process::exit(1);
}

/// Close the auto-created main window from tauri.conf.json and
/// recreate it with a custom data directory for portable WebView.
#[cfg(target_os = "linux")]
fn create_main_window_linux(
    app: &tauri::AppHandle,
    webview_dir: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    use tauri::WebviewUrl;
    use tauri::WebviewWindowBuilder;

    if let Some(w) = app.get_webview_window("main") {
        let _ = w.close();
    }
    WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
        .title("KnockKnock")
        .inner_size(1200.0, 800.0)
        .min_inner_size(900.0, 600.0)
        .decorations(false)
        .drag_and_drop(true)
        .data_directory(webview_dir.to_path_buf())
        .build()?;
    Ok(())
}
