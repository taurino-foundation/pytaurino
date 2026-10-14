use {
    anyhow::Result,
    std::{
        collections::HashMap,
        path::PathBuf,
        sync::{Arc, MutexGuard},
    },
    taurino_aio::IpcClient,
    taurino_config::{
        Config,
        utils::lock_state,
        webview::{WebContext, WebContextStore},
    },
};

#[allow(dead_code)]
pub struct Manager {
    config: Config,
    context: WebContextStore,
    pub ipc_connection: IpcClient,
}

impl Manager {
    pub fn new(config: Config, context: WebContextStore, ipc_connection: IpcClient) -> Arc<Self> {
        Arc::new(Self { config, context, ipc_connection })
    }

    pub fn webcontext(&self) -> Result<MutexGuard<'_, HashMap<Option<PathBuf>, WebContext>>> {
        lock_state(&self.context, "WebContextStore")
    }
}
