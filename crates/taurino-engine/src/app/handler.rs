use {
    crate::app::manager::RuntimeManager,
    anyhow::{Result, anyhow},
    rmpv::Value as MsgPackValue,
    std::{
        collections::HashMap,
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
    },
    tao::{
        event::{Event, WindowEvent as TaoWindowEvent},
        event_loop::ControlFlow,
        window::WindowId as TaoWindowId,
    },
    taurino_aio::IpcClient,
    taurino_config::{
        EngineEvent, EngineWindowTarget, EventLoopMessage, IpcEvent, log_if_err,
        protocol::{InvokeRequest, InvokeResponse, NotificationTarget},
    },
    taurino_core::{
        dispatcher::{PendingInvoke, RpcResponse, allocate_request_id, dispatch, reply_to_js, try_handle_notification},
        events::WindowEventWrapper,
        webview::WebView,
    },
};

const MAIN_WEBVIEW_LABEL: &str = "root";
const COMMAND_METHOD: &str = "commands.invoke";

/// Central event dispatcher and lifecycle coordinator for the engine.
///
/// Native Tao window events, synthesized window events and WebView events are
/// intentionally handled through separate paths:
///
/// - native Tao events -> `WindowEventWrapper::parse(...)`
/// - synthesized window events -> `WindowEventWrapper::from(...)`
/// - WebView events -> forwarded as `WebViewEvent`
///
/// This mirrors the event routing used by Tauri's Wry runtime.
pub struct EventHandler {
    /// Shared access to all engine subsystem managers.
    manager: Arc<RuntimeManager>,

    /// Prevents shutdown logic from executing more than once.
    shutting_down: AtomicBool,
    pending: HashMap<u16, PendingInvoke>,
    next_request_id: u16,
    ipc_connected: bool,
}

impl EventHandler {
    pub fn new(manager: Arc<RuntimeManager>) -> Self {
        Self {
            manager,
            shutting_down: AtomicBool::new(false),
            pending: HashMap::new(),
            next_request_id: 0_u16,
            ipc_connected: true,
        }
    }

    /// Handles a single Tao event-loop event.
    pub fn handle_event<'a>(
        &mut self,
        event: EngineEvent<'a>,
        target: &EngineWindowTarget,
        control_flow: &mut ControlFlow,
    ) {
        if self.shutting_down.load(Ordering::Acquire) {
            return;
        }

        /*
         * `event` is consumed by the match below, so inspect LoopDestroyed
         * beforehand.
         */
        let loop_destroyed = matches!(&event, Event::LoopDestroyed);

        let result = (|| -> Result<()> {
            if !matches!(*control_flow, ControlFlow::ExitWithCode(_)) {
                *control_flow = ControlFlow::Wait;
            }

            match event {
                /*
                                 * Synthesized window events.
                                 *
                                 * These originate from Wry/WebView behavior but semantically
                                 * represent window events.
                                 *
                                 * This arm must come before the generic UserEvent arm.
                                 */
                /*                 Event::UserEvent(EventLoopMessage::SynthesizedWindowEvent(
                                    window_id,
                                    _webview_id,
                                    event,
                                )) => {
                                    self.handle_synthesized_window_event(window_id, event)?;
                                } */

                                /*
                                 * WebView-specific events.
                                 *
                                 * These remain WebView events and are not converted into
                                 * WindowEvent.
                                 */
                /*                 Event::UserEvent(EventLoopMessage::WebviewEvent(
                                    window_id,
                                    webview_id,
                                    event,
                                )) => {
                                    self.handle_webview_event(window_id, webview_id, event)?;
                                } */

                                /*
                                 * Native Tao window events.
                                 */
                Event::WindowEvent { event, window_id, .. } => {
                    self.handle_window_event(event, window_id, control_flow)?;
                }

                /*
                 * Remaining engine messages.
                 */
                Event::UserEvent(message) => {
                    self.handle_user_message(message, target, control_flow)?;
                }

                _ => {}
            }

            Ok(())
        })();

        log_if_err!(result);

        /*
         * Global cleanup must run at most once.
         */
        if (loop_destroyed || matches!(*control_flow, ControlFlow::ExitWithCode(_)))
            && !self.shutting_down.swap(true, Ordering::AcqRel)
        {
            log_if_err!(self.cleanup_app_resources());
        }
    }

    // /// Handles window events synthesized by WebView/Wry.
    // ///
    // /// This intentionally mirrors Tauri:
    // ///
    // /// `SynthesizedWindowEvent`
    // ///     -> `WindowEventWrapper::from(...)`
    // ///     -> public `WindowEvent`
    // ///     -> global window event handler
    /*     fn handle_synthesized_window_event(
        &self,
        window_id: WindowId,
        event: SynthesizedWindowEvent,
    ) -> Result<()> {
        let Some(event) = WindowEventWrapper::from(event).0 else {
            return Ok(());
        };

        /*
         * Resolve only the information required by the callback while the
         * WindowManager lock is held.
         */
        let window_label = {
            let window_manager = self.manager.window_manager()?;

            let window = window_manager.get_by_id(window_id).ok_or_else(|| {
                anyhow!("window with id {window_id:?} not found for synthesized window event")
            })?;

            window.label.clone()
        };

        /*
         * The manager lock is released before invoking external callbacks.
         */
        self.manager.emit_global_window_event(window_label, event)?;

        Ok(())
    } */

    // /// Handles WebView-specific events.
    // ///
    // /// Unlike synthesized window events, WebView events are not converted
    // /// through `WindowEventWrapper`. They remain `WebViewEvent` values.
    /*     fn handle_webview_event(
        &self,
        window_id: WindowId,
        webview_id: WebViewId,
        event: WebViewEvent,
    ) -> Result<()> {
        /*
         * Resolve labels while holding the WindowManager lock only briefly.
         */
        let (window_label, webview_label) = {
            let window_manager = self.manager.window_manager()?;

            let window = window_manager.get_by_id(window_id).ok_or_else(|| {
                anyhow!("window with id {window_id:?} not found for WebView event")
            })?;

            let webview = window.webview(webview_id).ok_or_else(|| {
                anyhow!("WebView with id {webview_id:?} not found in window {window_id:?}")
            })?;

            (window.label.clone(), webview.label().to_owned())
        };

        /*
         * Emit after releasing the WindowManager lock.
         */
        self.manager.emit_global_webview_event(window_label, webview_label, event)?;

        Ok(())
    } */

    /// Handles engine user messages which are not Window/WebView events.
    fn handle_user_message(
        &mut self,
        message: EventLoopMessage,
        target: &EngineWindowTarget,
        control_flow: &mut ControlFlow,
    ) -> Result<()> {
        match message {
            EventLoopMessage::Shutdown => {
                println!("Keyboard interrupt detected. Exiting...");
                self.shutdown(control_flow)?;
            }

            EventLoopMessage::EngineTask(task) => {
                task(target, control_flow)?;
            }

            EventLoopMessage::Task(task) => {
                task();
            }

            #[cfg(target_os = "macos")]
            EventLoopMessage::SetDockVisibility(visible) => target.set_dock_visibility(visible),

            EventLoopMessage::JsInvoke { window_label, webview_label, request } => {
                self.handle_js_invoke(window_label, webview_label, request)?;
            }

            EventLoopMessage::JsReply { window_label, webview_label, request, response } => {
                self.reply_invoke(&window_label, &webview_label, &request, response)?;
            }
            EventLoopMessage::IPC(ipc_event) => {
                self.handle_ipc_event(ipc_event)?;
            }
            EventLoopMessage::EvalScript(_, _, _) => todo!(),
        }

        Ok(())
    }

    /// Handles a native Tao window event.
    ///
    /// Native Tao events require `WindowEventWrapper::parse(...)` because the
    /// mapping may depend on the current window state.
    fn handle_window_event(
        &self,
        event: TaoWindowEvent<'_>,
        window_id: TaoWindowId,
        control_flow: &mut ControlFlow,
    ) -> Result<()> {
        /*
         * Destruction is lifecycle handling rather than ordinary event
         * forwarding.
         */
        if matches!(event, TaoWindowEvent::Destroyed) {
            return self.handle_window_destroyed(window_id, control_flow);
        }

        /*
         * CloseRequested also has dedicated lifecycle semantics.
         */
        if matches!(event, TaoWindowEvent::CloseRequested { .. }) {
            return self.handle_close_requested(window_id, control_flow);
        }

        let (window_label, mapped_event) = {
            let device_registry = self.manager.device_registry()?;
            let window_manager = self.manager.window_manager()?;

            let window = window_manager
                .get_by_tao_id(window_id)
                .ok_or_else(|| anyhow!("window with Tao id {window_id:?} not found"))?;

            let Some(mapped_event) = WindowEventWrapper::parse(window, &event, device_registry.clone())?.0 else {
                return Ok(());
            };

            (window.label.clone(), mapped_event)
        };

        /* self.manager.emit_global_window_event(window_label, mapped_event)?; */

        Ok(())
    }

    /// Handles a native close request.
    fn handle_close_requested(&self, window_id: TaoWindowId, control_flow: &mut ControlFlow) -> Result<()> {
        let window_label = {
            let window_manager = self.manager.window_manager()?;

            let window = window_manager
                .get_by_tao_id(window_id)
                .ok_or_else(|| anyhow!("window with Tao id {window_id:?} not found"))?;

            window.label.clone()
        };

        /* self.manager
        .emit_global_window_event(window_label.clone(), WindowEvent::CloseRequested)?; */

        self.close_window(window_id, &window_label, control_flow)
    }

    /// Handles Tao's final native destruction notification.
    fn handle_window_destroyed(&self, window_id: TaoWindowId, control_flow: &mut ControlFlow) -> Result<()> {
        /*
         * The window may already have been removed when CloseRequested was
         * handled. In that case Tao is only confirming destruction.
         */
        let window = {
            let mut window_manager = self.manager.window_manager()?;
            window_manager.remove_by_tao_id(window_id)
        };

        let Some(window) = window else {
            return Ok(());
        };

        let window_label = window.label.clone();

        /* self.manager.emit_global_window_event(window_label.clone(), WindowEvent::Destroyed)?; */

        self.cleanup_window_resources(&window_label, control_flow)?;

        Ok(())
    }

    /// Removes a window and releases its associated resources.
    fn close_window(&self, window_id: TaoWindowId, window_label: &str, control_flow: &mut ControlFlow) -> Result<()> {
        let removed_window = {
            let mut window_manager = self.manager.window_manager()?;
            window_manager.remove_by_tao_id(window_id)
        };

        /*
         * Cleanup may acquire other manager locks, therefore it must happen
         * after the WindowManager guard has been released.
         */
        if removed_window.is_some() {
            self.cleanup_window_resources(window_label, control_flow)?;
        }

        let no_windows_left = {
            let window_manager = self.manager.window_manager()?;
            window_manager.is_empty()
        };

        if no_windows_left {
            *control_flow = ControlFlow::Exit;
        }

        Ok(())
    }

    /// Releases resources owned by one window.
    fn cleanup_window_resources(&self, _window_label: &str, control_flow: &mut ControlFlow) -> Result<()> {
        /*
         * Window-specific manager cleanup belongs here.
         *
         * Example:
         *
         * self.manager
         *     .trayicon()?
         *     .destroy_for_window(window_label)?;
         *
         * self.manager
         *     .menu()?
         *     .remove_window_menu(window_label)?;
         */

        if self.manager.window_manager()?.is_empty() {
            *control_flow = ControlFlow::Exit;
        }

        Ok(())
    }

    /// Releases application-global resources.
    fn cleanup_app_resources(&self) -> Result<()> { Ok(()) }

    /// Performs an explicit application shutdown.
    fn shutdown(&self, control_flow: &mut ControlFlow) -> Result<()> {
        /*
         * Shutdown is idempotent.
         */
        if self.shutting_down.swap(true, Ordering::AcqRel) {
            return Ok(());
        }

        /*
         * Collect window labels and clear the registry while the manager is
         * locked.
         */
        let window_labels = {
            let mut window_manager = self.manager.window_manager()?;

            let labels = window_manager.labels().map(str::to_owned).collect::<Vec<_>>();

            window_manager.clear();

            labels
        };

        /*
         * The WindowManager lock is released before cleanup and callbacks.
         */
        for window_label in window_labels {
            log_if_err!(self.cleanup_window_resources(&window_label, control_flow));

            /*             log_if_err!(
                self.manager.emit_global_window_event(window_label, WindowEvent::Destroyed,)
            ); */
        }

        self.cleanup_app_resources()?;

        *control_flow = ControlFlow::Exit;

        Ok(())
    }

    fn reply_invoke(
        &self,
        window_label: &str,
        webview_label: &str,
        request: &InvokeRequest,
        response: std::result::Result<InvokeResponse, String>,
    ) -> Result<()> {
        // WindowManager nur kurz sperren.
        let window = {
            let windows = self.manager.window_manager()?;

            windows
                .get_by_label(window_label)
                .cloned()
                .ok_or_else(|| anyhow!("Window '{}' does not exist", window_label))?
        };

        // WebView innerhalb des richtigen Fensters suchen.
        let webview = window
            .webview_by_label(webview_label)
            .ok_or_else(|| anyhow!("WebView '{}' does not exist in window '{}'", webview_label, window_label))?;

        // Python-Ergebnis an das ursprüngliche JS-Promise.
        taurino_core::dispatcher::resolve_invoke(webview, request, response)?;

        Ok(())
    }

    fn handle_js_invoke(&mut self, window_label: String, webview_label: String, request: InvokeRequest) -> Result<()> {
        if !self.ipc_connected {
            return self.reply_invoke(
                &window_label,
                &webview_label,
                &request,
                Err("Python IPC is disconnected".into()),
            );
        }

        let Some(request_id) = allocate_request_id(&self.pending, &mut self.next_request_id) else {
            return self.reply_invoke(
                &window_label,
                &webview_label,
                &request,
                Err("Too many pending IPC requests".into()),
            );
        };

        let arguments = serde_json::json!({
            "cmd": &request.cmd,
            "window_label": &window_label,
            "webview_label": &webview_label,
            "request": &request,
        });

        // RPC: [request_id, method, arguments]
        let message = (request_id, COMMAND_METHOD, [arguments]);

        let payload = match rmp_serde::to_vec_named(&message) {
            Ok(payload) => payload,
            Err(error) => {
                return self.reply_invoke(
                    &window_label,
                    &webview_label,
                    &request,
                    Err(format!("IPC encoding failed: {error}")),
                );
            }
        };

        self.pending.insert(request_id, PendingInvoke { window_label, webview_label, request });

        // Nicht blockierend: IpcClient legt die Daten in
        // seine Tokio-mpsc-Queue.
        if let Err(error) = self.manager.ipc_connection.send(payload) {
            if let Some(pending) = self.pending.remove(&request_id) {
                self.reply_invoke(
                    &pending.window_label,
                    &pending.webview_label,
                    &pending.request,
                    Err(format!("IPC send failed: {error:#}")),
                )?;
            }
        }

        Ok(())
    }

    fn handle_rpc_response(&mut self, payload: &[u8]) -> Result<()> {
        // Python -> Rust:
        // [request_id, status, message, data]
        let (request_id, status, message, data): RpcResponse = rmp_serde::from_slice(payload)?;

        let Some(pending) = self.pending.remove(&request_id) else {
            eprintln!("Unknown or expired RPC request ID: {request_id}");
            return Ok(());
        };

        let response: std::result::Result<InvokeResponse, String> = if status == 0 {
            serde_json::from_value(data).map_err(|error| format!("Invalid Python response: {error}"))
        } else {
            Err(format!("Python RPC error {status}: {message}"))
        };

        self.reply_invoke(&pending.window_label, &pending.webview_label, &pending.request, response)
    }

    fn handle_ipc_event(&mut self, event: IpcEvent) -> Result<()> {
        match event {
            IpcEvent::Message(payload) => {
                handle_ipc_message(&payload, &self.manager, &mut self.pending)?;
            }

            IpcEvent::Disconnected(reason) => {
                self.ipc_connected = false;

                eprintln!("IPC disconnected: {reason}");

                let pending: Vec<_> = self.pending.drain().map(|(_, entry)| entry).collect();

                for entry in pending {
                    if let Err(error) = self.reply_invoke(
                        &entry.window_label,
                        &entry.webview_label,
                        &entry.request,
                        Err(format!("Python IPC disconnected: {reason}")),
                    ) {
                        eprintln!("Failed to reject JS invoke: {error:#}");
                    }
                }
            }
        }

        Ok(())
    }
}

/// Handles incoming IPC messages on Tao's GUI thread.
///
/// Wire protocol:
/// [method, payload]              -> notification
/// [id, method, arguments]        -> RPC request
/// [id, status, message, data]    -> RPC response
pub fn handle_ipc_message(
    payload: &[u8],
    manager: &RuntimeManager,
    pending: &mut HashMap<u16, PendingInvoke>,
) -> Result<()> {
    let envelope: MsgPackValue = rmp_serde::from_slice(payload)?;

    let MsgPackValue::Array(elements) = envelope else {
        anyhow::bail!("Invalid IPC envelope: expected array");
    };

    match elements.len() {
        // --------------------------------------------------
        // Python -> JavaScript notification
        // --------------------------------------------------
        2 => {
            let (method, notification): (String, NotificationTarget) = rmp_serde::from_slice(payload)?;

            if method != "webview.emit" {
                anyhow::bail!("Unsupported notification: {method}");
            }

            // Compatibility with the existing Python emit().
            let window_label = notification.window_label.as_deref().unwrap_or("main");

            let window = {
                let windows = manager.window_manager()?;

                windows
                    .get_by_label(window_label)
                    .cloned()
                    .ok_or_else(|| anyhow!("Window not found: {window_label}"))?
            };

            let webview = window.webview_by_label(&notification.webview_label).ok_or_else(|| {
                anyhow!("WebView '{}' not found in window '{}'", notification.webview_label, window_label)
            })?;

            if !try_handle_notification(payload, webview) {
                eprintln!("Invalid WebView notification");
            }
        }

        // --------------------------------------------------
        // Python -> Rust RPC request
        // --------------------------------------------------
        3 => {
            let response = dispatch(payload)?;

            // Nonblocking enqueue to Tokio IPC worker.
            manager.ipc_connection.send(response)?;
        }

        // --------------------------------------------------
        // Python -> JavaScript RPC response
        // --------------------------------------------------
        4 => {
            let (request_id, status, message, data): RpcResponse = rmp_serde::from_slice(payload)?;

            let Some(entry) = pending.remove(&request_id) else {
                eprintln!("Unknown or expired RPC response ID: {request_id}");
                return Ok(());
            };

            let response: std::result::Result<InvokeResponse, String> = if status == 0 {
                serde_json::from_value(data).map_err(|error| format!("Invalid command response: {error}"))
            } else {
                Err(format!("RPC error {status}: {message}"))
            };

            let window = {
                let windows = manager.window_manager()?;

                windows
                    .get_by_label(&entry.window_label)
                    .cloned()
                    .ok_or_else(|| anyhow!("Window not found: {}", entry.window_label))?
            };

            let webview = window.webview_by_label(&entry.webview_label).ok_or_else(|| {
                anyhow!("WebView '{}' not found in window '{}'", entry.webview_label, entry.window_label)
            })?;

            // Resolve or reject the original JS Promise.
            reply_to_js(webview, &entry.request, response);
        }

        length => {
            anyhow::bail!("Invalid IPC envelope length: {length}");
        }
    }

    Ok(())
}
