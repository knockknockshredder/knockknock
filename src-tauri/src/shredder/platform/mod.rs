// src-tauri/src/shredder/platform/mod.rs

pub mod common;

#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(target_os = "linux")]
pub mod linux;

use crate::shredder::traits::PlatformIo;

pub fn create_platform_io() -> Box<dyn PlatformIo> {
    #[cfg(target_os = "windows")]
    { Box::new(windows::WindowsIo::new()) }

    #[cfg(target_os = "macos")]
    { Box::new(macos::MacOsIo::new()) }

    #[cfg(target_os = "linux")]
    { Box::new(linux::LinuxIo::new()) }
}