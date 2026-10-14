use {
    crate::window::main_window::WindowFrame,
    std::{collections::HashMap, sync::Arc},
    tao::window::WindowId,
};

pub mod factory;
pub mod webview;
pub mod main_window;

pub struct WindowManager {
    windows: HashMap<String, Arc<WindowFrame>>,
    label_map: HashMap<WindowId, String>,
}
