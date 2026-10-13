use {
    anyhow::{Context, Result, anyhow, bail},
    std::{
        collections::{HashMap, HashSet},
        fs,
        path::{Path, PathBuf},
        sync::{Arc, Mutex},
        thread::{self, ThreadId},
    },
    taurino_config::Config,
    wry::{WebContext as WryWebContext, WebView},
};

pub type WebContextId = String;
pub type WebViewId = String;

const DEFAULT_CONTEXT: &str = "default";
const AUTOMATION_ENV: &str = "TAURINO_WEBVIEW_AUTOMATION";

/// Persistent or ephemeral browser context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WebContextStorage {
    /// Stored in the application data directory.
    Persistent,

    /// Stored in a custom directory.
    Custom(PathBuf),

    /// No explicit persistent data directory.
    Ephemeral,
}

/// Configuration for a browser context.
#[derive(Debug, Clone)]
pub struct WebContextOptions {
    pub id: WebContextId,
    pub storage: WebContextStorage,
    pub automation: bool,
}

impl Default for WebContextOptions {
    fn default() -> Self {
        Self { id: DEFAULT_CONTEXT.into(), storage: WebContextStorage::Persistent, automation: false }
    }
}

impl WebContextOptions {
    pub fn new(id: impl Into<String>) -> Self { Self { id: id.into(), ..Self::default() } }

    pub fn with_storage(mut self, storage: WebContextStorage) -> Self {
        self.storage = storage;
        self
    }

    pub fn with_automation(mut self, enabled: bool) -> Self {
        self.automation = enabled;
        self
    }
}

/// Internal context owned by the manager.
#[derive(Debug)]
pub struct WebContext {
    pub inner: WryWebContext,

    pub referenced_by_webviews: HashSet<WebViewId>,

    pub registered_custom_protocols: HashSet<String>,

    pub data_directory: Option<PathBuf>,

    pub automation_enabled: bool,
}

/// Metadata for a managed browser context.
#[derive(Debug, Clone)]
pub struct WebContextInfo {
    pub id: WebContextId,
    pub data_directory: Option<PathBuf>,
    pub webview_count: usize,
    pub automation_enabled: bool,
    pub registered_custom_protocols: Vec<String>,
}

/// GUI-thread-owned browser context manager.
#[derive(Debug)]
pub struct WebContextManager {
    contexts: HashMap<WebContextId, WebContext>,
    webview_contexts: HashMap<WebViewId, WebContextId>,

    root_directory: PathBuf,
    gui_thread: ThreadId,
}

impl WebContextManager {
    /// Create a manager from the resolved Taurino configuration.
    pub fn new(config: &Config) -> Result<Arc<Mutex<Self>>> {
        let root_directory = config.webview_data_dir();

        fs::create_dir_all(&root_directory)
            .with_context(|| format!("Failed to create WebView root: {}", root_directory.display()))?;

        let root_directory = fs::canonicalize(root_directory)?;

        Ok(Arc::new(Mutex::new(Self {
            contexts: HashMap::new(),
            webview_contexts: HashMap::new(),
            root_directory,
            gui_thread: thread::current().id(),
        })))
    }

    /// Ensure native GUI operations run on the owner thread.
    fn assert_gui_thread(&self) -> Result<()> {
        if thread::current().id() != self.gui_thread {
            bail!(
                "WebContextManager must be accessed \
                 from its GUI owner thread"
            );
        }

        Ok(())
    }

    /// Resolve a context's profile directory.
    fn resolve_directory(&self, options: &WebContextOptions) -> Result<Option<PathBuf>> {
        validate_context_id(&options.id)?;

        let directory = match &options.storage {
            WebContextStorage::Persistent => Some(self.root_directory.join(&options.id)),

            WebContextStorage::Custom(path) => {
                if path.as_os_str().is_empty() {
                    bail!("Custom WebContext path is empty");
                }

                if !path.is_absolute() {
                    bail!("Custom WebContext path must be absolute");
                }

                Some(path.clone())
            }

            WebContextStorage::Ephemeral => None,
        };

        directory
            .map(|path| {
                fs::create_dir_all(&path)
                    .with_context(|| format!("Failed to create WebView profile: {}", path.display()))?;

                fs::canonicalize(&path)
                    .with_context(|| format!("Failed to canonicalize WebView profile: {}", path.display()))
            })
            .transpose()
    }

    /// Create a context or reuse an existing one.
    ///
    /// The returned reference allows multiple WebViews
    /// to use the very same native WryWebContext.
    pub fn get_or_create(&mut self, options: &WebContextOptions) -> Result<&mut WebContext> {
        self.assert_gui_thread()?;

        let directory = self.resolve_directory(options)?;

        if let Some(existing) = self.contexts.get(&options.id) {
            if existing.data_directory != directory {
                bail!(
                    "WebContext '{}' already exists \
                     with a different data directory",
                    options.id
                );
            }

            if existing.automation_enabled != options.automation {
                bail!(
                    "WebContext '{}' already exists \
                     with different automation settings",
                    options.id
                );
            }

            return Ok(self.contexts.get_mut(&options.id).unwrap());
        }

        // Two different native contexts must not attempt
        // to own the same persistent profile directory.
        if let Some(path) = &directory {
            if self.contexts.values().any(|ctx| ctx.data_directory.as_ref() == Some(path)) {
                bail!(
                    "WebView profile directory already belongs \
                     to another context: {}",
                    path.display()
                );
            }
        }

        let automation_allowed = options.automation && std::env::var(AUTOMATION_ENV).as_deref() == Ok("true");

        // Wry enforces at most one automation-enabled
        // context on Linux.
        if automation_allowed && self.contexts.values().any(|ctx| ctx.automation_enabled) {
            bail!(
                "Only one automation-enabled context \
                 is supported per manager"
            );
        }

        let mut inner = WryWebContext::new(directory.clone());

        inner.set_allows_automation(automation_allowed);

        self.contexts.insert(options.id.clone(), WebContext {
            inner,
            referenced_by_webviews: HashSet::new(),
            registered_custom_protocols: HashSet::new(),
            data_directory: directory,
            automation_enabled: automation_allowed,
        });

        Ok(self.contexts.get_mut(&options.id).unwrap())
    }

    /// Build a native WebView and register its ownership.
    ///
    /// The closure must finish building the WebView.
    /// Registrations are committed only after success.
    pub fn create_webview<F>(
        &mut self,
        options: &WebContextOptions,
        webview_id: impl Into<WebViewId>,
        build: F,
    ) -> Result<WebView>
    where
        F: FnOnce(&mut WebContext) -> Result<WebView>,
    {
        self.assert_gui_thread()?;

        let webview_id = webview_id.into();

        if webview_id.is_empty() {
            bail!("WebView ID must not be empty");
        }

        if self.webview_contexts.contains_key(&webview_id) {
            bail!("WebView '{}' is already registered", webview_id);
        }

        let context = self.get_or_create(options)?;

        // No logical WebView registration before build succeeds.
        let webview = build(context)?;

        context.referenced_by_webviews.insert(webview_id.clone());

        self.webview_contexts.insert(webview_id, options.id.clone());

        Ok(webview)
    }

    /// Return a mutable native context reference.
    pub fn get_mut(&mut self, context_id: &str) -> Result<&mut WebContext> {
        self.assert_gui_thread()?;

        self.contexts.get_mut(context_id).ok_or_else(|| anyhow!("Unknown WebContext: {context_id}"))
    }

    /// Check whether a context is registered.
    pub fn contains(&self, context_id: &str) -> bool { self.contexts.contains_key(context_id) }

    /// Find the context used by a WebView.
    pub fn context_for_webview(&self, webview_id: &str) -> Option<&str> {
        self.webview_contexts.get(webview_id).map(String::as_str)
    }

    /// Return metadata without exposing native internals.
    pub fn info(&self, context_id: &str) -> Option<WebContextInfo> {
        let context = self.contexts.get(context_id)?;

        let mut protocols: Vec<String> = context.registered_custom_protocols.iter().cloned().collect();

        protocols.sort();

        Some(WebContextInfo {
            id: context_id.to_owned(),
            data_directory: context.data_directory.clone(),
            webview_count: context.referenced_by_webviews.len(),
            automation_enabled: context.automation_enabled,
            registered_custom_protocols: protocols,
        })
    }

    /// Remove the logical association of a destroyed WebView.
    ///
    /// Call this only after the native WebView has been
    /// removed and dropped by the WebViewManager.
    pub fn release_webview(&mut self, webview_id: &str) -> Result<()> {
        self.assert_gui_thread()?;

        let context_id =
            self.webview_contexts.remove(webview_id).ok_or_else(|| anyhow!("Unknown WebView: {webview_id}"))?;

        let context = self.contexts.get_mut(&context_id).ok_or_else(|| anyhow!("Missing context: {context_id}"))?;

        context.referenced_by_webviews.remove(webview_id);

        Ok(())
    }

    /// Remove an unused native browser context.
    ///
    /// Persistent profile data is not deleted.
    pub fn remove_context(&mut self, context_id: &str) -> Result<bool> {
        self.assert_gui_thread()?;

        let Some(context) = self.contexts.get(context_id) else {
            return Ok(false);
        };

        if !context.referenced_by_webviews.is_empty() {
            bail!(
                "Cannot remove WebContext '{}': \
                 {} WebViews still reference it",
                context_id,
                context.referenced_by_webviews.len()
            );
        }

        self.contexts.remove(context_id);

        Ok(true)
    }

    /// Number of managed browser contexts.
    pub fn context_count(&self) -> usize { self.contexts.len() }

    /// Number of registered WebViews.
    pub fn webview_count(&self) -> usize { self.webview_contexts.len() }

    /// Base directory for persistent WebView profiles.
    pub fn root_directory(&self) -> &Path { &self.root_directory }
}

fn validate_context_id(id: &str) -> Result<()> {
    if id.is_empty()
        || id.len() > 128
        || !id.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
    {
        bail!("Invalid WebContext ID: {id:?}");
    }

    Ok(())
}
