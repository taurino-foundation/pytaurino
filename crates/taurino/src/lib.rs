use anyhow::{Context, Result};
use pyo3::exceptions::PyRuntimeError;
use taurino_engine::app::Taurino;
use {pyo3::prelude::*, std::sync::OnceLock};

pub fn get_taurino_version() -> &'static str {
    static VERSION: OnceLock<String> = OnceLock::new();

    VERSION.get_or_init(|| {
        let version = env!("CARGO_PKG_VERSION");
        version.replace("-alpha", "a").replace("-beta", "b")
    })
}

fn run_runtime(
    config: Vec<u8>,
    shutdown_event: Py<PyAny>,
    notify_ready: Py<PyAny>,
) -> Result<()> {
    let app = Taurino::new(config, shutdown_event, notify_ready)
        .context("Failed to initialize Taurino")?;

    app.run().context("Taurino runtime failed")
}

#[pyfunction]
fn runtime(
    py: Python<'_>,
    config: Vec<u8>,
    shutdown_event: Py<PyAny>,
    notify_ready: Py<PyAny>,
) -> PyResult<()> {
    py.detach(move || {
        run_runtime(config, shutdown_event, notify_ready)
            .map_err(|error| PyRuntimeError::new_err(format!("{error:#}")))
    })
}

#[pymodule]
fn _taurino(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add("__version__", get_taurino_version())?;
    module.add_function(wrap_pyfunction!(runtime, module)?)?;
    Ok(())
}
