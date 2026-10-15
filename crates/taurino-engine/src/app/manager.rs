use {
    crate::window::WindowManager,
    anyhow::Result,
    serde::Serialize,
    serde_json::json,
    std::{
        collections::HashMap,
        path::PathBuf,
        sync::{Arc, MutexGuard},
    },
    taurino_aio::IpcClient,
    taurino_config::{
        ArcMut, Config,
        utils::{DeviceRegistry, lock_state},
        webview::{WebContext, WebContextStore},
    },
    taurino_core::EngineLoopProxy,
};

#[allow(dead_code)]
pub struct RuntimeManager {
    config: Config,
    context: WebContextStore,
    device_registry: ArcMut<DeviceRegistry>,
    pub proxy: EngineLoopProxy,
    pub ipc_connection: IpcClient,

    _window: ArcMut<WindowManager>,
}

impl RuntimeManager {
    pub fn new(
        config: Config,
        context: WebContextStore,
        device_registry: ArcMut<DeviceRegistry>,
        ipc_connection: IpcClient,
        proxy: EngineLoopProxy,
    ) -> Result<Arc<Self>> {
        let window_manager = WindowManager::new()?;
        let manager =
            Arc::new(Self { config, context, device_registry, proxy, ipc_connection, _window: window_manager.clone() });

        lock_state(&window_manager, "WindowManager")?.bind_manager(manager.clone());

        Ok(manager)
    }

    pub fn config(&self) -> Result<Config> { Ok(self.config.clone()) }
    pub fn device_registry(self: &Arc<Self>) -> Result<MutexGuard<'_, DeviceRegistry>> {
        lock_state(&self.device_registry, "DeviceRegistry")
    }
    pub fn webcontext(&self) -> Result<MutexGuard<'_, HashMap<Option<PathBuf>, WebContext>>> {
        lock_state(&self.context, "WebContextStore")
    }

    pub fn window_manager(&self) -> Result<MutexGuard<'_, WindowManager>> { lock_state(&self._window, "WindowManager") }
}

impl RuntimeManager {
    /// Sends a native event to the Python application.
    ///
    /// Wire protocol:
    /// ["taurino.event", {"type": "...", "payload": {...}}]
    pub fn emit_native_event<T: Serialize>(&self, event_type: &str, payload: T) -> Result<()> {
        self.ipc_connection.data_transfer(&(
            "taurino.event",
            json!({
                "type": event_type,
                "payload": payload,
            }),
        ))
    }

    /// Emits an application event-loop event.
    pub fn emit_loop_event<T: Serialize>(&self, event: T) -> Result<()> {
        self.emit_native_event(
            "loop",
            json!({
                "event": event,
            }),
        )
    }

    /// Emits a native or synthesized window event.
    pub fn emit_window_event<T: Serialize>(&self, window_label: &str, event: T) -> Result<()> {
        self.emit_native_event(
            "window",
            json!({
                "window_label": window_label,
                "event": event,
            }),
        )
    }

    /// Emits a WebView-specific event.
    pub fn emit_webview_event<T: Serialize>(&self, window_label: &str, webview_label: &str, event: T) -> Result<()> {
        self.emit_native_event(
            "webview",
            json!({
                "window_label": window_label,
                "webview_label": webview_label,
                "event": event,
            }),
        )
    }

    /// Emits a system tray event.
    pub fn emit_tray_event<T: Serialize>(&self, tray_id: &str, event: T) -> Result<()> {
        self.emit_native_event(
            "tray",
            json!({
                "tray_id": tray_id,
                "event": event,
            }),
        )
    }

    /// Emits a global shortcut event.
    pub fn emit_shortcut_event<T: Serialize>(&self, shortcut: &str, event: T) -> Result<()> {
        self.emit_native_event(
            "shortcut",
            json!({
                "shortcut": shortcut,
                "event": event,
            }),
        )
    }
}
