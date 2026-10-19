#![cfg(target_os = "windows")]

mod clipboard;
mod destination_list;
mod dialog;
mod direct_manipulation;
mod direct_write;
mod directx_atlas;
mod directx_devices;
mod directx_renderer;
mod dispatcher;
mod display;
mod events;
mod hdr_output;
mod keyboard;
mod platform;
mod system_notifications;
mod system_settings;
mod util;
mod vsync;
mod window;
mod wrapper;

pub(crate) use clipboard::*;
pub(crate) use destination_list::*;
pub(crate) use direct_write::*;
pub(crate) use directx_atlas::*;
pub(crate) use directx_devices::*;
pub(crate) use directx_renderer::*;
pub(crate) use dispatcher::*;
pub(crate) use display::*;
pub(crate) use events::*;
pub(crate) use keyboard::*;
pub(crate) use platform::*;
pub(crate) use system_notifications::*;
pub(crate) use system_settings::*;
pub(crate) use util::*;
pub(crate) use vsync::*;
pub(crate) use window::*;
pub(crate) use wrapper::*;

pub use hdr_output::{register_hdr_image, unregister_hdr_image};
pub use platform::WindowsPlatform;

/// AraView fork: HDR 출력은 DirectComposition 스왑체인에서 DWM이 FP16 scRGB 버퍼를
/// 표시하지 못하므로 hwnd 스왑체인을 써야 한다. 플랫폼을 만들기 전에 호출한다.
pub fn set_hdr_output(enabled: bool) {
    HDR_OUTPUT.store(enabled, std::sync::atomic::Ordering::Relaxed);
}

/// [`set_hdr_output`]으로 켠 HDR 출력 모드인지 돌려준다.
pub fn hdr_output() -> bool {
    HDR_OUTPUT.load(std::sync::atomic::Ordering::Relaxed)
}

static HDR_OUTPUT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub(crate) use windows::Win32::Foundation::HWND;
