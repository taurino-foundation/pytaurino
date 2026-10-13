use {pyo3::prelude::*, std::sync::OnceLock};

pub fn get_taurino_version() -> &'static str {
    static VERSION: OnceLock<String> = OnceLock::new();

    VERSION.get_or_init(|| {
        let version = env!("CARGO_PKG_VERSION");
        version.replace("-alpha", "a").replace("-beta", "b")
    })
}

#[pymodule]
fn _taurino(module: &Bound<'_, PyModule>) -> PyResult<()> {
    /* module.add_function(wrap_pyfunction!(runtime, module)?)?; */
    Ok(())
}
