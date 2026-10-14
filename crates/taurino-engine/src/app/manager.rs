use {
    crate::window::WindowManager,
    anyhow::Result,
    std::{
        collections::HashMap,
        path::PathBuf,
        sync::{Arc, MutexGuard},
    },
    taurino_aio::IpcClient,
    taurino_config::{
        ArcMut, Config, EngineLoop, EngineLoopProxy,
        utils::{DeviceRegistry, lock_state},
        webview::{WebContext, WebContextStore},
    },
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
        event_loop: &mut EngineLoop,
    ) -> Result<Arc<Self>> {
        let proxy = event_loop.create_proxy();

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

    pub fn window(&self) -> Result<MutexGuard<'_, WindowManager>> { lock_state(&self._window, "WindowManager") }
}
