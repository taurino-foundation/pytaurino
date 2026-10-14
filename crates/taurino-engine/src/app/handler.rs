use {
    crate::app::{NativeWindows, manager::Manager},
    anyhow::Result,
    rmpv::Value as MsgPackValue,
    std::{
        collections::HashMap,
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
    },
    tao::{
        event::{Event, WindowEvent},
        event_loop::ControlFlow,
        window::WindowId,
    },
    taurino_aio::IpcClient,
    taurino_config::{
        EngineEvent, EngineWindowTarget, EventLoopMessage, IpcEvent,
        dispatcher::{PendingInvoke, RpcResponse, allocate_request_id, dispatch, reply_to_js, try_handle_notification},
        log_if_err,
        protocol::InvokeResponse,
    },
    wry::WebView,
};

const MAIN_WEBVIEW_LABEL: &str = "root";
const COMMAND_METHOD: &str = "commands.invoke";

// Internal application events dispatched through Tao.
//
// If Taurino already defines a user-event enum,
// use that type instead.

// Central event dispatcher and lifecycle coordinator.
pub struct EventHandler {
    /// Shared access to Taurino subsystem managers.
    manager: Arc<Manager>,

    /// Prevents application cleanup from executing twice.
    shutting_down: AtomicBool,
    windows: NativeWindows,
    id: Option<WindowId>,
}

impl EventHandler {
    pub fn new(manager: Arc<Manager>, windows: NativeWindows) -> Self {
        Self { manager, shutting_down: AtomicBool::new(false), windows, id: None }
    }

    fn set_id(&mut self, id: WindowId) { self.id = Some(id) }

    fn get_id(&self) -> Result<WindowId> { Ok(self.id.unwrap_or_else(|| unsafe { WindowId::dummy() })) }
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

            Event::UserEvent(event) => {
                self.handle_user_event(pending, next_request_id, ipc_connected, event, target, control_flow)
            }

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
                        reply_to_js(webview, &request, Err("Python IPC connection is not available".into()));
                    } else if let Some(request_id) = allocate_request_id(pending, next_request_id) {
                        // Same MessagePack format as Python's App._serve_peer().
                        let encoded = rmp_serde::to_vec_named(&(request_id, COMMAND_METHOD, [&request]));

                        match encoded {
                            Ok(payload) => match self.manager.ipc_connection.send(payload) {
                                Ok(()) => {
                                    pending.insert(request_id, PendingInvoke { webview_label, request });
                                }
                                Err(error) => {
                                    reply_to_js(webview, &request, Err(format!("IPC queue error: {error:#}")))
                                }
                            },
                            Err(error) => {
                                reply_to_js(webview, &request, Err(format!("MessagePack encoding failed: {error}")))
                            }
                        }
                    } else {
                        reply_to_js(webview, &request, Err("Too many pending IPC requests".into()));
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
                        handle_ipc_message(&payload, webview, pending, &self.manager.ipc_connection)
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
                                reply_to_js(webview, &entry.request, Err(format!("Python IPC disconnected: {reason}")));
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
