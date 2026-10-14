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
