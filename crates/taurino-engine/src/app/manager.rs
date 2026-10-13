use {
    crate::window::context::WebContextManager,
    anyhow::Result,
    std::sync::{Arc, Mutex, MutexGuard},
    taurino_config::utils::lock_state,
};

pub struct Manager {
    _context_manager: Arc<Mutex<WebContextManager>>,
}

impl Manager {
    pub fn new(context_manager: Arc<Mutex<WebContextManager>>) -> Result<Arc<Self>> {
        let manager = Arc::new(Self { _context_manager: context_manager });
        Ok(manager)
    }

    pub fn webcontext<'a>(self: &'a Arc<Self>) -> Result<MutexGuard<'a, WebContextManager>> {
        lock_state(&self._context_manager, "WebContextManager")
    }
}
