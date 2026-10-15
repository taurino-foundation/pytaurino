pub mod os_native;
pub mod window;
pub mod webview;
pub mod wrappers;
pub mod platforms;
pub mod resources;
pub mod dispatcher;
pub mod events;
use {
    crate::events::{SynthesizedEvent, WebViewEvent},
    tao::window::Theme as TaoTheme,
    taurino_config::{
        NativeEvent, NativeLoop, NativeLoopBuilder, NativeLoopClosed, NativeLoopProxy, NativeWindowTarget, Theme,
        webview::WebViewId, window::WindowId,
    },
};

pub enum EventLoopMessage {
    FullScreenElementChanged(WindowId, bool),
    SynthesizedEvent(WindowId, WebViewId, SynthesizedEvent),
    WebviewEvent(WindowId, WebViewId, WebViewEvent),
}

pub type EngineLoop = NativeLoop<EventLoopMessage>;
pub type EngineLoopBuilder = NativeLoopBuilder<EventLoopMessage>;
pub type LoopProxy = NativeLoopProxy<EventLoopMessage>;
pub type EngineWindowTarget = NativeWindowTarget<EventLoopMessage>;
pub type EngineLoopClosed = NativeLoopClosed<EventLoopMessage>;
pub type EngineEvent<'a> = NativeEvent<'a, EventLoopMessage>;

pub struct EngineLoopProxy(LoopProxy);

impl Clone for EngineLoopProxy {
    fn clone(&self) -> Self { Self(self.0.clone()) }
}

impl EngineLoopProxy {
    pub fn new(proxy: LoopProxy) -> Self { Self(proxy) }

    pub fn send_event(&self, event: EventLoopMessage) -> anyhow::Result<()> {
        self.0
            .send_event(taurino_config::NativeMessage::UserEvents(event))
            .map_err(|_| anyhow::anyhow!("Taurino event loop is closed"))
    }

    pub fn send_native_event(&self, event: taurino_config::NativeMessage<EventLoopMessage>) -> anyhow::Result<()> {
        self.0.send_event(event).map_err(|_| anyhow::anyhow!("Taurino event loop is closed"))
    }
}

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
