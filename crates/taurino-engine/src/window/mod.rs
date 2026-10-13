use {
    crate::window::window::WindowFrame,
    std::{collections::HashMap, sync::Arc},
    tao::window::WindowId,
};

pub mod build;
pub mod context;
pub mod factory;
pub mod webview;
pub mod window;

pub struct WindowManager {
    windows: HashMap<String, Arc<WindowFrame>>,
    label_map: HashMap<WindowId, String>,
}
