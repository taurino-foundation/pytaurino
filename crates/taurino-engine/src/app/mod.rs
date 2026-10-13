pub mod handler;
pub mod index;
pub mod manager;
use {
    crate::{
        app::{handler::EventHandler, manager::Manager},
        window::context::{WebContextManager, WebContextOptions},
    },
    anyhow::Result,
    std::{collections::HashMap, sync::Arc},
    tao::window::WindowBuilder,
    taurino_config::{Config, EngineLoop, EngineLoopBuilder, dispatcher::PendingInvoke},
    wry::WebViewBuilder,
};

pub struct Taurino {
    event_loop: EngineLoop,
    manager: Arc<Manager>,
}

impl Taurino {
    pub fn new(config: Vec<u8>) -> Result<Self> {
        let conf = Config::new(config)?;
        let context_manager = WebContextManager::new(&conf)?;
        let mut loop_builder = EngineLoopBuilder::with_user_event();
        let event_loop = loop_builder.build();
        let manager = Manager::new(context_manager)?;

        Ok(Self { event_loop, manager: manager.clone() })
    }

    pub fn run(self) -> Result<()> {
        // Clone the manager because ownership of `self.manager` will later be
        // transferred into EngineEventHandler.
        let _manager = self.manager.clone();
        // application lifecycle behavior for incoming Tao events.

        // Both WebViews use the same context.
        let options = WebContextOptions::new("default");

        let window_builder = WindowBuilder::new();
        let window = window_builder.build(&self.event_loop)?;
        let _webview = {
            let mut context_manager = _manager.webcontext()?;
            let webview_b = context_manager.create_webview(&options, "webview-b", |context| {
                let builder = WebViewBuilder::new_with_web_context(&mut context.inner).with_url("https://tauri.app");

                Ok(builder.build(&window)?)
            })?;
            webview_b
        };

        let event_handler = EventHandler::new(self.manager);

        // ---------------------------------------------------------------------
        // Enter GUI event loop
        // ---------------------------------------------------------------------
        //
        // From this point onward, application lifecycle changes should normally
        // be driven by events rather than by direct calls from external threads.
        let mut pending = HashMap::<u16, PendingInvoke>::new();
        let mut next_request_id = 0_u16;
        let mut ipc_connected = true;
        self.event_loop.run(move |event, target, control_flow| {
            event_handler.handle_event(event, target, control_flow);
        });
    }
}
