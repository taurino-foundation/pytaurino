use {
    crate::app::index::HTML,
    anyhow::{Context, Result},
    tao::window::{Window, WindowBuilder},
    taurino_config::{
        EngineLoopProxy, EngineWindowTarget,
        dispatcher::{all_initialization_scripts, install_ipc_bridge},
    },
    wry::{WebView, WebViewBuilder},
};
pub fn create_window(event_loop: &EngineWindowTarget, proxy: EngineLoopProxy) -> Result<(Window, WebView)> {
    let window =
        WindowBuilder::new().with_title("Taurino App").build(event_loop).context("Failed to create GUI window")?;

    let mut builder = WebViewBuilder::new();
    for script in all_initialization_scripts("main", "root", true, None)? {
        builder = builder.with_initialization_script_for_main_only(script.script, script.for_main_frame_only);
    }

    builder = install_ipc_bridge(builder, "root".to_owned(), proxy);
    let webview = builder.with_html(HTML).build(&window).context("Failed to create WebView")?;

    Ok((window, webview))
}
