pub mod handler;
pub mod index;
use {std::thread::JoinHandle, tao::platform::run_return::EventLoopExtRunReturn};
pub mod manager;
use {
    crate::{
        app::{handler::EventHandler, manager::Manager},
        spawn_shutdown_watcher,
        window::factory::create_window,
    },
    anyhow::Result,
    pyo3::{Py, PyAny, Python},
    std::{
        collections::HashMap,
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
    },
    taurino_aio::IpcClient,
    taurino_config::{
        Config, EngineLoop, EngineLoopBuilder, EventLoopMessage, dispatcher::PendingInvoke, webview::WebContextStore,
    },
};

#[allow(dead_code)]
pub struct Taurino {
    ipc_connection: IpcClient,
    event_loop: EngineLoop,
    manager: Arc<Manager>,
    watcher_running: Arc<AtomicBool>,
    watcher: JoinHandle<()>,
}

impl Taurino {
    pub fn new(config_bytes: Vec<u8>, shutdown_event: Py<PyAny>, notify_ready: Py<PyAny>) -> Result<Self> {
        let config = Config::new(config_bytes)?;
        let context_manager = WebContextStore::default();
        // Tao's event loop must be created and run on the GUI process main thread.
        let mut loop_builder = EngineLoopBuilder::with_user_event();
        let event_loop = loop_builder.build();

        let ipc_proxy = event_loop.create_proxy();

        // IPC runs on its own Tokio thread and sends events through the proxy.
        let ipc_connection = IpcClient::new(&config.connection, move |event| {
            let _ = ipc_proxy.send_event(EventLoopMessage::IPC(event));
        })?;

        let watcher_running = Arc::new(AtomicBool::new(true));
        let watcher = spawn_shutdown_watcher(shutdown_event, event_loop.create_proxy(), Arc::clone(&watcher_running));

        // Notify Python only once the IPC client, window, and WebView exist.
        // This does NOT mean JavaScript has installed all application listeners.
        if let Err(error) = Python::attach(|py| notify_ready.call0(py).map(|_| ())) {
            watcher_running.store(false, Ordering::Release);
            let _ = watcher.join();
            return Err(error.into());
        }

        let manager = Manager::new(config, context_manager, ipc_connection.clone());
        Ok(Self { ipc_connection, event_loop, manager, watcher_running, watcher })
    }

    pub fn run(self) -> Result<()> {
        // Clone the manager because ownership of `self.manager` will later be
        // transferred into EngineEventHandler.
        let _manager = self.manager.clone();
        let proxy = self.event_loop.create_proxy();
        // application lifecycle behavior for incoming Tao events.

        // Both WebViews use the same context.

        let (window, webview) = create_window(&self.event_loop, proxy)?;

        let event_handler = EventHandler::new(self.manager, window, webview);

        // ---------------------------------------------------------------------
        // Enter GUI event loop
        // ---------------------------------------------------------------------
        //
        // From this point onward, application lifecycle changes should normally
        // be driven by events rather than by direct calls from external threads.

        let mut pending = HashMap::<u16, PendingInvoke>::new();
        let mut next_request_id = 0_u16;
        let mut ipc_connected = true;
        let mut main_loop = self.event_loop;
        let exit_code = main_loop.run_return(move |event, target, control_flow| {
            event_handler.handle_event(
                &mut pending,
                &mut next_request_id,
                &mut ipc_connected,
                event,
                target,
                control_flow,
            );
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
