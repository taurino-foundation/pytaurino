pub mod os_native;
pub mod window;
pub mod webview;
pub mod wrappers;
pub mod platforms;
pub mod resources;
pub mod dispatcher;
pub mod events;
use {tao::window::Theme as TaoTheme, taurino_config::Theme};

pub fn map_from_tao_theme(theme: TaoTheme) -> Theme {
    match theme {
        TaoTheme::Light => Theme::Light,
        TaoTheme::Dark => Theme::Dark,
        _ => Theme::Light,
    }
}
pub fn map_from_theme(theme: Theme) -> TaoTheme {
    match theme {
        Theme::Light => TaoTheme::Light,
        Theme::Dark => TaoTheme::Dark,
        _ => TaoTheme::Light,
    }
}

/*

use std::{
    fmt,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
pub mod javascript_interfaces;
use crate::{
    platforms::WindowExt, tools::wrappers::TaoIcon, window::javascript_interfaces::JsEvaluator,
};
#[cfg(target_os = "macos")]
use objc2::{MainThreadMarker, rc::Retained};
#[cfg(target_os = "macos")]
use objc2_app_kit::NSView;
#[cfg(any(
    windows,
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd"
))]
pub mod undecorated_resizing;
#[cfg(windows)]
use crate::schema::FocusState;
#[cfg(target_os = "macos")]
use crate::schema::window::TitleBarStyle;
use crate::{
    WebView, WebViewManager,
    schema::window::{PreventOverflowConfig, WindowConfig},
    tools::lock_state,
    unsafe_impl_sync_send,
};
use anyhow::{Result, anyhow};
#[cfg(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd"
))]
use gtk;
use muda::MenuId;



*/
