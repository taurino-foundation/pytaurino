use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
};
pub mod js_ipc;
use anyhow::{Context, Result};
use pyo3::{exceptions::PyRuntimeError, prelude::*};

use tao::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy},
    platform::run_return::EventLoopExtRunReturn,
    window::WindowBuilder,
};
pub mod client;
pub mod config;
pub mod dispatch;
use crate::{
    client::{IpcClient, IpcEvent},
    config::Config,
};

#[derive(Debug, Clone)]
enum EventLoopMessage {
    Shutdown,
    IPC(IpcEvent),
}
// Called from Python's spawned child main thread.
#[pyfunction]
fn runtime(
    py: Python<'_>,
    config: Vec<u8>,
    shutdown_event: Py<PyAny>,
    notify_ready: Py<PyAny>,
) -> PyResult<()> {
    // Release GIL while native GUI is running.
    py.detach(move || {
        run_native(config, shutdown_event, notify_ready)
            .map_err(|error| PyRuntimeError::new_err(format!("{error:#}")))
    })
}

fn run_native(config: Vec<u8>, shutdown_event: Py<PyAny>, notify_ready: Py<PyAny>) -> Result<()> {
    let _config = Config::from_engine_bytes(&config).context("Invalid GUI configuration")?;

    // Must run on the child process main thread.
    let mut event_loop = EventLoopBuilder::<EventLoopMessage>::with_user_event().build();
    let base_proxy = event_loop.create_proxy();
    let ipc_proxy = base_proxy.clone();
    let _signal_proxy = base_proxy.clone();
    let _ipc_client = IpcClient::new(&_config.connection, move |event| {
        let _ = ipc_proxy.send_event(EventLoopMessage::IPC(event));
    })?;
    /*     ctrlc::set_handler(move || {
        println!("Keyboard interrupt detected. Exiting...");
        let _ = _signal_proxy.send_event(EventLoopMessage::Shutdown);
    })?; */

    let window = WindowBuilder::new()
        .with_title("Taurino App")
        // .with_inner_size(tao::dpi::LogicalSize::new(config.width, config.height))
        .build(&event_loop)
        .context("Failed to create GUI window")?;

    let proxy = event_loop.create_proxy();
    let watcher_running = Arc::new(AtomicBool::new(true));

    let watcher = spawn_shutdown_watcher(shutdown_event, proxy, Arc::clone(&watcher_running));

    // Notify parent only after native initialization.
    let ready_result = Python::attach(|py| notify_ready.call0(py).map(|_| ()));

    if let Err(error) = ready_result {
        watcher_running.store(false, Ordering::Release);
        let _ = watcher.join();
        return Err(error.into());
    }

    // Native event loop: child process main thread.
    let exit_code = event_loop.run_return(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::UserEvent(msg) => match msg {
                EventLoopMessage::Shutdown => {
                    *control_flow = ControlFlow::Exit;
                }
                EventLoopMessage::IPC(ipc_msg) => match ipc_msg {
                    IpcEvent::Message(payload) => match crate::dispatch::dispatch(&payload) {
                        Ok(response) => {
                            if let Err(error) = _ipc_client.send(response) {
                                eprintln!("IPC send failed: {error:#}");
                                *control_flow = ControlFlow::Exit;
                            }
                        }

                        Err(error) => {
                            eprintln!("IPC dispatch failed: {error:#}");
                            *control_flow = ControlFlow::Exit;
                        }
                    },
                    IpcEvent::Disconnected(_id) => {}
                },
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

    // Release watcher after GUI shutdown.
    watcher_running.store(false, Ordering::Release);

    let _ = watcher.join();

    if exit_code != 0 {
        anyhow::bail!("Tao event loop exited with code {exit_code}");
    }

    Ok(())
}

fn spawn_shutdown_watcher(
    shutdown_event: Py<PyAny>,
    proxy: EventLoopProxy<EventLoopMessage>,
    running: Arc<AtomicBool>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        while running.load(Ordering::Acquire) {
            // multiprocessing.Event.wait(timeout)
            let wait_result = Python::attach(|py| {
                shutdown_event
                    .call_method1(py, "wait", (0.1_f64,))
                    .and_then(|result| result.extract::<bool>(py))
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

#[pymodule]
fn _taurino(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(runtime, module)?)?;

    Ok(())
}
