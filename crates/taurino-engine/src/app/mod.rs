pub mod handler;
pub mod index;
use {
    std::thread::JoinHandle,
    tao::platform::run_return::EventLoopExtRunReturn,
    taurino_config::utils::DeviceRegistry,
    taurino_core::{EngineLoop, EngineLoopBuilder, EngineLoopProxy},
};
pub mod manager;
use {
    crate::{
        app::{handler::EventHandler, manager::RuntimeManager},
        spawn_shutdown_watcher,
    },
    anyhow::Result,
    pyo3::{Py, PyAny, Python},
    std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    taurino_aio::IpcClient,
    taurino_config::{Config, webview::WebContextStore},
};

#[allow(dead_code)]
pub struct Runtime {
    ipc_connection: IpcClient,
    event_loop: EngineLoop,
    manager: Arc<RuntimeManager>,
    watcher_running: Arc<AtomicBool>,
    watcher: JoinHandle<()>,
}

impl Runtime {
    pub fn new(config_bytes: Vec<u8>, shutdown_event: Py<PyAny>, notify_ready: Py<PyAny>) -> Result<Self> {
        let device_registry = DeviceRegistry::new()?;
        let config = Config::new(config_bytes)?;
        let context_manager = WebContextStore::default();
        // Tao's event loop must be created and run on the GUI process main thread.
        let mut loop_builder = EngineLoopBuilder::with_user_event();
        let event_loop = loop_builder.build();
        let proxy = EngineLoopProxy::new(event_loop.create_proxy());
        let ipc_proxy = event_loop.create_proxy();

        // IPC runs on its own Tokio thread and sends events through the proxy.
        let ipc_connection = IpcClient::new(&config.connection, move |event| {
            let _ = ipc_proxy.send_event(taurino_config::NativeMessage::IPC(event));
        })?;

        let watcher_running = Arc::new(AtomicBool::new(true));
        let watcher = spawn_shutdown_watcher(shutdown_event, proxy.clone(), Arc::clone(&watcher_running));

        // Notify Python only once the IPC client, window, and WebView exist.
        // This does NOT mean JavaScript has installed all application listeners.
        if let Err(error) = Python::attach(|py| notify_ready.call0(py).map(|_| ())) {
            watcher_running.store(false, Ordering::Release);
            let _ = watcher.join();
            return Err(error.into());
        }

        let manager = RuntimeManager::new(config, context_manager, device_registry, ipc_connection.clone(), proxy)?;
        Ok(Self { ipc_connection, event_loop, manager, watcher_running, watcher })
    }

    pub fn run(self) -> Result<()> {
        // Clone the manager because ownership of `self.manager` will later be
        // transferred into EngineEventHandler.
        let manager = self.manager.clone();
        let config = manager.config()?;
        let app_config = config.app_config();
        let target = &self.event_loop;

        for window_options in &app_config.windows {
            let mut window_manager = manager.window_manager()?;
            window_manager.open_window(window_options, target)?;
        }
        // application lifecycle behavior for incoming Tao events.

        let mut event_handler = EventHandler::new(self.manager);

        // ---------------------------------------------------------------------
        // Enter GUI event loop
        // ---------------------------------------------------------------------
        //
        // From this point onward, application lifecycle changes should normally
        // be driven by events rather than by direct calls from external threads.

        let mut main_loop = self.event_loop;
        let exit_code = main_loop.run_return(move |event, target, control_flow| {
            event_handler.handle_event(event, target, control_flow);
        });
        self.watcher_running.store(false, Ordering::Release);
        if self.watcher.join().is_err() {
            eprintln!("Taurino shutdown watcher panicked");
        }

        if exit_code != 0 {
            anyhow::bail!("Tao event loop exited with code {exit_code}");
        }
        Ok(())
    }
}
