use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
};
pub mod app;
pub mod window;
use {
    pyo3::prelude::*,
    taurino_config::{EngineLoopProxy, EventLoopMessage},
};

fn spawn_shutdown_watcher(
    shutdown_event: Py<PyAny>,
    proxy: EngineLoopProxy,
    running: Arc<AtomicBool>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        while running.load(Ordering::Acquire) {
            // multiprocessing.Event.wait(timeout), in Python's spawned child.
            let wait_result = Python::attach(|py| {
                shutdown_event.call_method1(py, "wait", (0.1_f64,)).and_then(|result| result.extract::<bool>(py))
            });

            match wait_result {
                Ok(true) => {
                    let _ = proxy.send_event(EventLoopMessage::Shutdown);
                    break;
                }
                Ok(false) => {}
                Err(error) => {
                    eprintln!("Shutdown watcher failed: {error}");
                    let _ = proxy.send_event(EventLoopMessage::Shutdown);
                    break;
                }
            }
        }
    })
}
/*
/// Python starts this entry point in its separately spawned GUI process.
#[pyfunction]
fn runtime(
    py: Python<'_>,
    config: Vec<u8>,
    shutdown_event: Py<PyAny>,
    notify_ready: Py<PyAny>,
) -> PyResult<()> {
    // Do not hold the Python GIL during Tao's event loop.
    py.detach(move || {
        run_native(config, shutdown_event, notify_ready)
            .map_err(|error| PyRuntimeError::new_err(format!("{error:#}")))
    })
}

fn run_native(
    config_bytes: Vec<u8>,
    shutdown_event: Py<PyAny>,
    notify_ready: Py<PyAny>,
) -> Result<()> {
    let config = Config::from_engine_bytes(&config_bytes).context("Invalid GUI configuration")?;

    // Tao's event loop must be created and run on the GUI process main thread.
    let mut event_loop = EventLoopBuilder::<EventLoopMessage>::with_user_event().build();
    let ipc_proxy = event_loop.create_proxy();

    // IPC runs on its own Tokio thread and sends events through the proxy.
    let ipc_client = IpcClient::new(&config.connection, move |event| {
        let _ = ipc_proxy.send_event(EventLoopMessage::IPC(event));
    })?;

    let window = WindowBuilder::new()
        .with_title("Taurino App")
        .build(&event_loop)
        .context("Failed to create GUI window")?;

    let mut builder = WebViewBuilder::new();
    for script in all_initialization_scripts("main", MAIN_WEBVIEW_LABEL, true, None)? {
        builder = builder
            .with_initialization_script_for_main_only(script.script, script.for_main_frame_only);
    }

    builder = install_ipc_bridge(
        builder,
        MAIN_WEBVIEW_LABEL.to_owned(),
        event_loop.create_proxy(),
    );
    let webview = builder
        .with_html(HTML)
        .build(&window)
        .context("Failed to create WebView")?;

    let watcher_running = Arc::new(AtomicBool::new(true));
    let watcher = spawn_shutdown_watcher(
        shutdown_event,
        event_loop.create_proxy(),
        Arc::clone(&watcher_running),
    );

    // Notify Python only once the IPC client, window, and WebView exist.
    // This does NOT mean JavaScript has installed all application listeners.
    if let Err(error) = Python::attach(|py| notify_ready.call0(py).map(|_| ())) {
        watcher_running.store(false, Ordering::Release);
        let _ = watcher.join();
        return Err(error.into());
    }



    let exit_code = event_loop.run_return(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::UserEvent(message) => match message {
                EventLoopMessage::Shutdown => {
                    *control_flow = ControlFlow::Exit;
                }
                EventLoopMessage::IPC(IpcEvent::Message(payload)) => {
                    handle_ipc_message(&payload, &webview, &mut pending, &ipc_client);
                }
                EventLoopMessage::IPC(IpcEvent::Disconnected(reason)) => {
                    ipc_connected = false;
                    eprintln!("IPC disconnected: {reason}");

                    // A transport failure must reject all pending JS Promises.
                    for (_, entry) in pending.drain() {
                        if entry.webview_label == MAIN_WEBVIEW_LABEL {
                            reply_to_js(
                                &webview,
                                &entry.request,
                                Err(format!("Python IPC disconnected: {reason}")),
                            );
                        }
                    }
                }
                EventLoopMessage::JsInvoke {
                    webview_label,
                    request,
                } => {
                    if webview_label != MAIN_WEBVIEW_LABEL {
                        eprintln!("Unknown WebView: {webview_label}");
                    } else if !ipc_connected {
                        reply_to_js(
                            &webview,
                            &request,
                            Err("Python IPC connection is not available".into()),
                        );
                    } else if let Some(request_id) =
                        allocate_request_id(&pending, &mut next_request_id)
                    {
                        // Same MessagePack format as Python's App._serve_peer().
                        let encoded =
                            rmp_serde::to_vec_named(&(request_id, COMMAND_METHOD, [&request]));

                        match encoded {
                            Ok(payload) => match ipc_client.send(payload) {
                                Ok(()) => {
                                    pending.insert(
                                        request_id,
                                        PendingInvoke {
                                            webview_label,
                                            request,
                                        },
                                    );
                                }
                                Err(error) => reply_to_js(
                                    &webview,
                                    &request,
                                    Err(format!("IPC queue error: {error:#}")),
                                ),
                            },
                            Err(error) => reply_to_js(
                                &webview,
                                &request,
                                Err(format!("MessagePack encoding failed: {error}")),
                            ),
                        }
                    } else {
                        reply_to_js(
                            &webview,
                            &request,
                            Err("Too many pending IPC requests".into()),
                        );
                    }
                }
                EventLoopMessage::JsReply {
                    webview_label,
                    request,
                    response,
                } => {
                    if webview_label == MAIN_WEBVIEW_LABEL {
                        reply_to_js(&webview, &request, response);
                    } else {
                        eprintln!("Unknown WebView: {webview_label}");
                    }
                }
            },
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                window_id,
                ..
            } if window_id == window.id() => {
                *control_flow = ControlFlow::Exit;
            }
            _ => {}
        }
    });

    watcher_running.store(false, Ordering::Release);
    if watcher.join().is_err() {
        eprintln!("Taurino shutdown watcher panicked");
    }

    if exit_code != 0 {
        anyhow::bail!("Tao event loop exited with code {exit_code}");
    }
    Ok(())
} */

#[pymodule]
fn _taurino(module: &Bound<'_, PyModule>) -> PyResult<()> {
    /* module.add_function(wrap_pyfunction!(runtime, module)?)?; */
    Ok(())
}
