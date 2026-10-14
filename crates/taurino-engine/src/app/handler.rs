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
        EngineEvent, EngineWindowTarget, EventLoopMessage, IpcEvent,
        dispatcher::{PendingInvoke, RpcResponse, allocate_request_id, dispatch, reply_to_js, try_handle_notification},
        log_if_err,
        protocol::InvokeResponse,
    },
    taurino_core::events::WindowEventWrapper,
    wry::WebView,
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
}

impl EventHandler {
    pub fn new(manager: Arc<RuntimeManager>) -> Self { Self { manager, shutting_down: AtomicBool::new(false) } }

    /// Handles a single Tao event-loop event.
    pub fn handle_event<'a>(
        &self,
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
            let window_manager = self.manager.window()?;

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
            let window_manager = self.manager.window()?;

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
        &self,
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

            EventLoopMessage::JsInvoke { webview_label, request } => todo!(),
            EventLoopMessage::JsReply { webview_label, request, response } => todo!(),
            EventLoopMessage::IPC(ipc_event) => todo!(),
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
            let window_manager = self.manager.window()?;

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
            let window_manager = self.manager.window()?;

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
            let mut window_manager = self.manager.window()?;
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
            let mut window_manager = self.manager.window()?;
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
            let window_manager = self.manager.window()?;
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

        if self.manager.window()?.is_empty() {
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
            let mut window_manager = self.manager.window()?;

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
}

/*
 * These variants are intercepted in `handle_event` before the
 * generic UserEvent path reaches this function.
 */
/*             EventLoopMessage::SynthesizedWindowEvent(..) => {
    unreachable!("SynthesizedWindowEvent must be handled before handle_user_message");
}

EventLoopMessage::WebviewEvent(..) => {
    unreachable!("WebviewEvent must be handled before handle_user_message");
}
EventLoopMessage::ContainsFullScreenElementChanged(window_id, fullscreen) => {
    let window = {
        let window_manager = self.manager.window()?;

        window_manager
            .get_by_id(window_id)
            .cloned()
            .ok_or_else(|| anyhow!("window with id {window_id:?} not found"))?
    };

    window.set_fullscreen(fullscreen)?;
} */
/*             EventLoopMessage::EvalScript(window_id, webview_id, _script) => {
    let window = {
        let window_manager = self.manager.window()?;

        window_manager
            .get_by_id(window_id)
            .cloned()
            .ok_or_else(|| anyhow!("window with id {window_id:?} not found"))?
    };
    let _js_evaluator = window.js_evaluator_by_id(webview_id)?;
    // _js_evaluator.reject(id, error)
    // _js_evaluator.resolve(id, value)
} */
/*
pub enum ApplicationMessage {
  #[cfg(target_os = "macos")]
  Show,
  #[cfg(target_os = "macos")]
  Hide,
  #[cfg(any(target_os = "macos", target_os = "ios"))]
  FetchDataStoreIdentifiers(Box<dyn FnOnce(Vec<[u8; 16]>) + Send + 'static>),
  #[cfg(any(target_os = "macos", target_os = "ios"))]
  RemoveDataStore([u8; 16], Box<dyn FnOnce(Result<()>) + Send + 'static>),
}

    #[cfg(target_os = "macos")]
    Message::SetActivationPolicy(activation_policy) => {
      event_loop.set_activation_policy_at_runtime(tao_activation_policy(activation_policy))
    }
    #[cfg(target_os = "macos")]
    Message::SetDockVisibility(visible) => event_loop.set_dock_visibility(visible),

Message::Application(application_message) => match application_message {
      #[cfg(target_os = "macos")]
      ApplicationMessage::Show => {
        event_loop.show_application();
      }
      #[cfg(target_os = "macos")]
      ApplicationMessage::Hide => {
        event_loop.hide_application();
      }
      #[cfg(any(target_os = "macos", target_os = "ios"))]
      ApplicationMessage::FetchDataStoreIdentifiers(cb) => {
        if let Err(e) = WebView::fetch_data_store_identifiers(cb) {
          // this shouldn't ever happen because we're running on the main thread
          // but let's be safe and warn here
          log::error!("failed to fetch data store identifiers: {e}");
        }
      }
      #[cfg(any(target_os = "macos", target_os = "ios"))]
      ApplicationMessage::RemoveDataStore(uuid, cb) => {
        WebView::remove_data_store(&uuid, move |res| {
          cb(res.map_err(|_| Error::FailedToRemoveDataStore))
        })
      }
    },


*/

/* pub fn event(f:taurino_core::native::tao::event::WindowEvent){
    match f {
        TaoWindowEvent::Resized(physical_size){},
        TaoWindowEvent::Moved(physical_position){},
        TaoWindowEvent::CloseRequested{},
        TaoWindowEvent::Destroyed{},
        TaoWindowEvent::Started{},
        TaoWindowEvent::Suspended{},
        TaoWindowEvent::Resumed{},
        TaoWindowEvent::Stopped{},
        TaoWindowEvent::DroppedFile{},
        TaoWindowEvent::HoveredFile{},
        TaoWindowEvent::HoveredFileCancelled{},
        TaoWindowEvent::ReceivedImeText{},
        TaoWindowEvent::Focused{},
        TaoWindowEvent::KeyboardInput { device_id, event, is_synthetic }{},
        TaoWindowEvent::ModifiersChanged(modifiers_state){},
        TaoWindowEvent::CursorMoved { device_id, position, modifiers }{},
        TaoWindowEvent::CursorEntered { device_id }{},
        TaoWindowEvent::CursorLeft { device_id }{},
        TaoWindowEvent::MouseWheel { device_id, delta, phase, modifiers }{},
        TaoWindowEvent::MouseInput { device_id, state, button, modifiers }{},
        TaoWindowEvent::TouchpadPressure { device_id, pressure, stage }{},
        TaoWindowEvent::AxisMotion { device_id, axis, value }{},
        TaoWindowEvent::Touch(touch){},
        TaoWindowEvent::ScaleFactorChanged { scale_factor, new_inner_size }{},
        TaoWindowEvent::ThemeChanged(theme){},
        TaoWindowEvent::DecorationsClick{},
        _{},
    }
} */

/// Called on Tao's GUI thread, never by the Tokio IPC worker.
///
/// IPC wire protocol:
///   [method, payload]              = notification (Python -> JS)
///   [id, method, arguments]        = RPC request (Python -> Rust)
///   [id, status, message, data]    = RPC response (Python -> Rust)
pub fn handle_ipc_message(
    payload: &[u8],
    webview: &WebView,
    pending: &mut HashMap<u16, PendingInvoke>,
    ipc_client: &IpcClient,
) {
    let envelope: MsgPackValue = match rmp_serde::from_slice(payload) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("Invalid MessagePack IPC payload: {error}");
            return;
        }
    };

    let MsgPackValue::Array(elements) = envelope else {
        eprintln!("Invalid IPC envelope: expected MessagePack array");
        return;
    };

    match elements.len() {
        2 => {
            // Fire-and-forget notification: Python -> Rust -> JavaScript.
            // The handler in dispatch.rs calls WebView::evaluate_script,
            // so it must execute on this GUI thread.
            if !try_handle_notification(payload, webview) {
                eprintln!("Unsupported or malformed IPC notification");
            }
        }
        3 => {
            // Normal Python -> Rust RPC request. Keep RPC logic separate
            // from the notification system and the JavaScript commands.
            match dispatch(payload) {
                Ok(reply) => {
                    // IpcClient::send queues bytes with try_send; no GUI blocking.
                    if let Err(error) = ipc_client.send(reply) {
                        eprintln!("Failed to queue IPC RPC response: {error:#}");
                    }
                }
                Err(error) => {
                    eprintln!("Invalid Python -> Rust RPC request: {error:#}");
                }
            }
        }
        4 => {
            // Response to a previous JS invoke forwarded to Python.
            let (request_id, status, message, data): RpcResponse = match rmp_serde::from_slice(payload) {
                Ok(response) => response,
                Err(error) => {
                    eprintln!("Invalid IPC RPC response: {error}");
                    return;
                }
            };

            let Some(pending_request) = pending.remove(&request_id) else {
                eprintln!("Unknown or expired RPC response ID: {request_id}");
                return;
            };

            if pending_request.webview_label != MAIN_WEBVIEW_LABEL {
                eprintln!("Unknown WebView: {}", pending_request.webview_label);
                return;
            }

            let response = if status == 0 {
                serde_json::from_value::<InvokeResponse>(data)
                    .map_err(|error| format!("Invalid command response: {error}"))
            } else {
                Err(format!("RPC error {status}: {message}"))
            };

            reply_to_js(webview, &pending_request.request, response);
        }
        size => {
            eprintln!("Invalid IPC envelope length: {size}");
        }
    }
}

/*
// Internal application events dispatched through Tao.
//
// If Taurino already defines a user-event enum,
// use that type instead.

// Central event dispatcher and lifecycle coordinator.
pub struct EventHandler {
    /// Shared access to Taurino subsystem managers.
    manager: Arc<RuntimeManager>,

    /// Prevents application cleanup from executing twice.
    shutting_down: AtomicBool,
    id: Option<WindowId>,
}

impl EventHandler {
    pub fn new(manager: Arc<RuntimeManager>) -> Self {
        Self { manager, shutting_down: AtomicBool::new(false), id: None }
    }

    fn set_id(&mut self, id: WindowId) {
        self.id = Some(id)
    }

    fn get_id(&self) -> Result<WindowId> {
        Ok(self.id.unwrap_or_else(|| unsafe { WindowId::dummy() }))
    }
    /// Main Tao event-loop dispatcher.
    pub fn handle_event(
        &mut self,
        pending: &mut HashMap<u16, PendingInvoke>,
        next_request_id: &mut u16,
        ipc_connected: &mut bool,
        event: EngineEvent<'_>,
        target: &EngineWindowTarget,
        control_flow: &mut ControlFlow,
    ) {
        if self.shutting_down.load(Ordering::Acquire) {
            return;
        }

        // Preserve an already requested exit.
        if !matches!(*control_flow, ControlFlow::ExitWithCode(_)) {
            *control_flow = ControlFlow::Wait;
        }

        let result = match event {
            Event::WindowEvent { window_id, event, .. } => {
                self.set_id(window_id);
                self.handle_window_event(window_id, event, target, control_flow)
            }

            Event::UserEvent(event) => self.handle_user_event(
                pending,
                next_request_id,
                ipc_connected,
                event,
                target,
                control_flow,
            ),

            Event::LoopDestroyed => {
                self.shutdown();
                Ok(())
            }

            _ => Ok(()),
        };

        log_if_err!(result);

        if matches!(*control_flow, ControlFlow::ExitWithCode(_)) {
            self.shutdown();
        }
    }

    /// Handles native Tao window events.
    fn handle_window_event(
        &self,
        window_id: WindowId,
        event: WindowEvent<'_>,
        _target: &EngineWindowTarget,
        control_flow: &mut ControlFlow,
    ) -> Result<()> {
        match event {
            WindowEvent::CloseRequested => {
                // Current policy: closing any window exits Taurino.
                // Replace with manager-based window removal
                // when multi-window lifecycle is implemented.
                if window_id == self.get_id()? {
                    *control_flow = ControlFlow::Exit;
                }
            }

            WindowEvent::Destroyed => {
                // Window has been destroyed.
            }

            WindowEvent::Resized(_size) => {
                // Synchronize associated WebViews if needed.
            }

            WindowEvent::Focused(_focused) => {
                // Dispatch focus changes.
            }

            WindowEvent::Moved(_position) => {
                // Dispatch position changes.
            }

            _ => {}
        }

        Ok(())
    }

    /// Handles Taurino application user events.
    fn handle_user_event(
        &self,
        pending: &mut HashMap<u16, PendingInvoke>,
        next_request_id: &mut u16,
        ipc_connected: &mut bool,
        event: EventLoopMessage,
        _target: &EngineWindowTarget,
        control_flow: &mut ControlFlow,
    ) -> Result<()> {
        match event {
            EventLoopMessage::Shutdown => {
                *control_flow = ControlFlow::Exit;
            }
            EventLoopMessage::EngineTask(engine_task) => engine_task(_target, control_flow)?,
            EventLoopMessage::Task(task) => task(),
            EventLoopMessage::EvalScript(_, _, _) => {}
            EventLoopMessage::JsInvoke { webview_label, request } => {
                let window_id = self.get_id()?;
                if let Some((webview, _)) = self.windows.get(&window_id) {
                    if webview_label != MAIN_WEBVIEW_LABEL {
                        eprintln!("Unknown WebView: {webview_label}");
                    } else if !*ipc_connected {
                        reply_to_js(
                            webview,
                            &request,
                            Err("Python IPC connection is not available".into()),
                        );
                    } else if let Some(request_id) = allocate_request_id(pending, next_request_id)
                    {
                        // Same MessagePack format as Python's App._serve_peer().
                        let encoded =
                            rmp_serde::to_vec_named(&(request_id, COMMAND_METHOD, [&request]));

                        match encoded {
                            Ok(payload) => match self.manager.ipc_connection.send(payload) {
                                Ok(()) => {
                                    pending.insert(
                                        request_id,
                                        PendingInvoke { webview_label, request },
                                    );
                                }
                                Err(error) => reply_to_js(
                                    webview,
                                    &request,
                                    Err(format!("IPC queue error: {error:#}")),
                                ),
                            },
                            Err(error) => reply_to_js(
                                webview,
                                &request,
                                Err(format!("MessagePack encoding failed: {error}")),
                            ),
                        }
                    } else {
                        reply_to_js(
                            webview,
                            &request,
                            Err("Too many pending IPC requests".into()),
                        );
                    }
                }
            }
            EventLoopMessage::JsReply { webview_label, request, response } => {
                let window_id = self.get_id()?;
                if let Some((webview, _)) = self.windows.get(&window_id) {
                    if webview_label == MAIN_WEBVIEW_LABEL {
                        reply_to_js(webview, &request, response);
                    } else {
                        eprintln!("Unknown WebView: {webview_label}");
                    }
                }
            }
            EventLoopMessage::IPC(ipc_event) => match ipc_event {
                IpcEvent::Message(payload) => {
                    let window_id = self.get_id()?;
                    if let Some((webview, _)) = self.windows.get(&window_id) {
                        handle_ipc_message(
                            &payload,
                            webview,
                            pending,
                            &self.manager.ipc_connection,
                        )
                    }
                }
                IpcEvent::Disconnected(reason) => {
                    let window_id = self.get_id()?;
                    if let Some((webview, _)) = self.windows.get(&window_id) {
                        *ipc_connected = false;
                        eprintln!("IPC disconnected: {reason}");

                        // A transport failure must reject all pending JS Promises.
                        for (_, entry) in pending.drain() {
                            if entry.webview_label == MAIN_WEBVIEW_LABEL {
                                reply_to_js(
                                    webview,
                                    &entry.request,
                                    Err(format!("Python IPC disconnected: {reason}")),
                                );
                            }
                        }
                    }
                }
            },
        }

        Ok(())
    }

    /// Releases application-global resources once.
    fn shutdown(&self) {
        if self.shutting_down.swap(true, Ordering::AcqRel) {
            return;
        }

        log_if_err!(self.cleanup_app_resources());
    }

    fn cleanup_app_resources(&self) -> Result<()> {
        // Release resources through the manager here.
        let _ = &self.manager;

        Ok(())
    }
}




*/
