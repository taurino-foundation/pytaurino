#[cfg(windows)]
use crate::platforms::windows::register_webview_events;
#[cfg(windows)]
use tao::platform::windows::WindowExtWindows;
#[cfg(windows)]
use taurino_config::{ArcMut, FocusState};
use {
    crate::wrappers::RectWrapper,
    anyhow::{Result, anyhow},
    dpi::LogicalPosition,
    std::{
        collections::HashMap,
        ops::Deref,
        path::PathBuf,
        rc::Rc,
        sync::{
            Arc, Mutex,
            atomic::{AtomicU32, Ordering},
        },
    },
    tao::window::Window,
    taurino_config::{
        EngineLoopProxy, Rect, WebContextStore,
        webview::{WebViewId, WebviewBounds},
        window::WindowId,
    },
    wry::WebViewBuilder,
};
pub struct WebView {
    pub(crate) label: String,
    id: WebViewId,
    window_id: Arc<Mutex<WindowId>>,
    inner: Rc<wry::WebView>,
    context_store: WebContextStore,
    context_key: Option<PathBuf>,
    bounds: Arc<Mutex<Option<WebviewBounds>>>,
}
impl WebView {
    /// Adopts an already-created native WebView.
    ///
    /// The associated context and its label registration must already have been
    /// set up by the calling code.
    ///
    /// To obtain additional handles to the same WebView, use `clone()` rather
    /// than calling `new()` again with the same `inner`.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        webview_id: WebViewId,
        webview_label: String,
        window_id: Arc<Mutex<WindowId>>,
        #[cfg(windows)] focused_webview: ArcMut<FocusState>,
        webview: wry::WebView,
        context_key: Option<PathBuf>,
        context_store: WebContextStore,
        bounds: Arc<Mutex<Option<WebviewBounds>>>,
        proxy: EngineLoopProxy,
    ) -> Self {
        #[cfg(windows)]
        register_webview_events(window_id.clone(), webview_id, webview_label.clone(), focused_webview, &webview, proxy);

        #[cfg(not(windows))]
        let _ = proxy;

        Self {
            label: webview_label,
            id: webview_id,
            window_id,
            inner: Rc::new(webview),
            context_store,
            context_key,
            bounds,
        }
    }

    // --------------------------------------------------------
    // Identity
    // --------------------------------------------------------
    pub fn id(&self) -> WebViewId { self.id }
    pub fn label(&self) -> &str { &self.label }
    // --------------------------------------------------------
    // Window association
    // --------------------------------------------------------
    pub fn window_id(&self) -> WindowId { *self.window_id.lock().expect("WebView window_id mutex is poisoned") }
    pub fn window_id_handle(&self) -> Arc<Mutex<WindowId>> { Arc::clone(&self.window_id) }
    /// Changes only the stored window ID.
    ///
    /// The native WebView is not moved by this operation.
    /// For an actual window change, normally use `reparent()`.
    pub fn set_window_id(&self, window_id: WindowId) {
        *self.window_id.lock().expect("WebView window_id mutex is poisoned") = window_id;
    }
    // --------------------------------------------------------
    // Native WebView
    // --------------------------------------------------------
    /// Returns the existing Rc without cloning it.
    ///
    /// An externally created Rc clone must not outlive all WebView wrappers if
    /// their drop is to reliably remove the context reference.
    pub fn inner(&self) -> &Rc<wry::WebView> { &self.inner }
    /// Unique access to Wry, even in the presence of identically named methods.
    pub fn as_wry(&self) -> &wry::WebView { self.inner.as_ref() }
    /// Existing low-level access from the original API.
    ///
    /// Replacing the Rc updates neither context references nor window
    /// association nor bounds.
    #[deprecated(note = "Replacing inner bypasses context management. \
                For normal Wry calls use as_wry().")]
    pub fn inner_mut(&mut self) -> &mut Rc<wry::WebView> { &mut self.inner }
    // --------------------------------------------------------
    // Context
    // --------------------------------------------------------
    /// Returns the unmodified key used for the context store.
    pub fn context_key(&self) -> &Option<PathBuf> { &self.context_key }
    pub fn context_store(&self) -> &WebContextStore { &self.context_store }
    /// Clones the shared store handle.
    pub fn context_store_handle(&self) -> WebContextStore { self.context_store.clone() }
    // --------------------------------------------------------
    // Cached layout bounds
    // --------------------------------------------------------
    /// Reads the stored bounds without removing them.
    ///
    /// This is NOT wry::WebView::bounds().
    /// WebviewBounds must implement Clone.
    pub fn bounds(&self) -> Option<WebviewBounds> {
        self.bounds.lock().expect("WebView bounds mutex is poisoned").clone()
    }
    pub fn bounds_handle(&self) -> Arc<Mutex<Option<WebviewBounds>>> { Arc::clone(&self.bounds) }
    /// Changes only the stored layout data.
    ///
    /// The position and size of the native WebView remain unchanged.
    pub fn set_cached_bounds(&self, bounds: Option<WebviewBounds>) {
        *self.bounds.lock().expect("WebView bounds mutex is poisoned") = bounds;
    }
    /// Takes the stored bounds.
    ///
    /// Afterwards all wrapper clones hold None at this location.
    pub fn take_bounds(&self) -> Option<WebviewBounds> {
        self.bounds.lock().expect("WebView bounds mutex is poisoned").take()
    }
    pub fn clear_bounds(&self) { self.set_cached_bounds(None); }
    // --------------------------------------------------------
    // Actual native geometry
    // --------------------------------------------------------
    /// Queries the current geometry directly from Wry.
    pub fn native_bounds(&self) -> Result<wry::Rect> {
        self.as_wry()
            .bounds()
            .map_err(|error| anyhow!("failed to read native bounds for webview '{}': {error}", self.label))
    }

    /// Changes the native geometry.
    ///
    /// The stored WebviewBounds are not adjusted automatically: their
    /// conversion belongs in your layout code.
    pub fn set_window_bounds(&self, bounds: wry::Rect) -> Result<()> {
        self.as_wry()
            .set_bounds(bounds)
            .map_err(|error| anyhow!("failed to set native bounds for webview '{}': {error}", self.label))
    }
}
// ------------------------------------------------------------
// Trait implementations
// ------------------------------------------------------------
impl Deref for WebView {
    type Target = wry::WebView;
    fn deref(&self) -> &Self::Target { self.as_wry() }
}
impl AsRef<wry::WebView> for WebView {
    fn as_ref(&self) -> &wry::WebView { self.as_wry() }
}
impl Drop for WebView {
    fn drop(&mut self) {
        // Only clean up if this wrapper holds the last strong Rc
        // to the native WebView.
        //
        // Unlike Rc::get_mut, this check does not additionally account for
        // existing Weak references.
        if Rc::strong_count(&self.inner) != 1 {
            return;
        }
        // Do not trigger another panic on a poisoned store.
        // In that error case the registration is left in place.
        //
        // Do not drop the wrapper while the same thread already holds the
        // context store lock.
        let Ok(mut context_store) = self.context_store.lock() else {
            return;
        };
        if let Some(web_context) = context_store.get_mut(&self.context_key) {
            web_context.referenced_by_webviews.remove(&self.label);
            // Linux/BSD: keep the context for reuse.
            // Other platforms: remove the unused context.
            #[cfg(not(any(
                target_os = "linux",
                target_os = "dragonfly",
                target_os = "freebsd",
                target_os = "netbsd",
                target_os = "openbsd"
            )))]
            if web_context.referenced_by_webviews.is_empty() {
                context_store.remove(&self.context_key);
            }
        }
    }
}

/// Manages the WebViews associated with a single window.
///
/// WebViews are stored in a contiguous [`Vec`] to preserve their insertion
/// order and allow callers to access them as a slice.
///
/// Additional lookup maps provide efficient access by WebView ID and label
/// without requiring a linear scan through the WebView collection.
pub struct WebViewManager {
    /// WebViews in registration order.
    webviews: Vec<WebView>,

    /// Maps a WebView ID to its current index inside [`Self::webviews`].
    id_index: HashMap<WebViewId, usize>,

    /// Maps a WebView label to its current index inside [`Self::webviews`].
    label_index: HashMap<String, usize>,

    /// Monotonically increasing source for WebView IDs.
    ///
    /// IDs start at `1` and are not reused after a WebView is removed.
    next_webview_id: Arc<AtomicU32>,
}

impl WebViewManager {
    /// Creates an empty WebView manager.
    pub fn new() -> Result<Self> {
        Ok(Self {
            webviews: Vec::new(),
            id_index: HashMap::new(),
            label_index: HashMap::new(),
            next_webview_id: Arc::new(AtomicU32::new(1)),
        })
    }

    // =========================================================================
    // IDs
    // =========================================================================

    /// Allocates and returns the next engine-level WebView ID.
    pub fn next_webview_id(&self) -> WebViewId { self.next_webview_id.fetch_add(1, Ordering::Relaxed).into() }

    #[cfg(any(target_os = "macos", target_os = "ios"))]
    pub fn fetch_data_store_identifiers<F>(&self, cb: F) -> Result<()>
    where
        F: FnOnce(Vec<[u8; 16]>) + Send + 'static,
    {
        wry::WebView::fetch_data_store_identifiers(cb)
            .map_err(|e| anyhow!("failed to fetch data store identifiers: {e}"))
    }

    #[cfg(any(target_os = "macos", target_os = "ios"))]
    pub fn remove_data_store<F>(uuid: [u8; 16], cb: F)
    where
        F: FnOnce(Result<()>) + Send + 'static,
    {
        wry::WebView::remove_data_store(&uuid, move |res| {
            cb(res.map_err(|e| anyhow!("failed to remove data store: {e}")));
        });
    }
    // =========================================================================
    // Registration
    // =========================================================================

    /// Registers an already-created WebView.
    ///
    /// The WebView collection and both lookup indices are updated together.
    pub fn insert(&mut self, webview: WebView) -> Result<()> {
        let id = webview.id();
        let label = webview.label().to_string();

        if self.id_index.contains_key(&id) {
            return Err(anyhow!("WebView with id {:?} is already registered", id));
        }

        if self.label_index.contains_key(&label) {
            return Err(anyhow!("WebView with label {:?} is already registered", label));
        }

        let index = self.webviews.len();

        self.webviews.push(webview);
        self.id_index.insert(id, index);
        self.label_index.insert(label, index);

        Ok(())
    }

    // =========================================================================
    // Lookup by ID
    // =========================================================================

    /// Returns a WebView by its engine-level ID.
    pub fn get_by_id(&self, id: WebViewId) -> Option<&WebView> {
        let index = *self.id_index.get(&id)?;
        self.webviews.get(index)
    }

    /// Returns a mutable WebView by its engine-level ID.
    pub fn get_by_id_mut(&mut self, id: WebViewId) -> Option<&mut WebView> {
        let index = *self.id_index.get(&id)?;
        self.webviews.get_mut(index)
    }

    /// Returns a WebView by ID or an error if it is not registered.
    pub fn get(&self, id: WebViewId) -> Result<&WebView> {
        self.get_by_id(id).ok_or_else(|| anyhow!("WebView with id {:?} is not registered", id))
    }

    // =========================================================================
    // Lookup by label
    // =========================================================================

    /// Returns a WebView by its label.
    ///
    /// Lookup is performed through the label index and does not scan
    /// the WebView collection.
    pub fn get_by_label(&self, label: &str) -> Option<&WebView> {
        let index = *self.label_index.get(label)?;
        self.webviews.get(index)
    }

    /// Returns a mutable WebView by its label.
    pub fn get_by_label_mut(&mut self, label: &str) -> Option<&mut WebView> {
        let index = *self.label_index.get(label)?;
        self.webviews.get_mut(index)
    }

    /// Resolves a WebView label to its engine-level ID.
    pub fn id_by_label(&self, label: &str) -> Option<WebViewId> { self.get_by_label(label).map(WebView::id) }

    /// Returns the label associated with a WebView ID.
    pub fn label(&self, id: WebViewId) -> Option<&str> { self.get_by_id(id).map(WebView::label) }

    // =========================================================================
    // Existence
    // =========================================================================

    /// Returns whether a WebView with the given ID is registered.
    pub fn contains(&self, id: WebViewId) -> bool { self.id_index.contains_key(&id) }

    /// Returns whether a WebView with the given label is registered.
    pub fn contains_label(&self, label: &str) -> bool { self.label_index.contains_key(label) }

    // =========================================================================
    // Removal
    // =========================================================================

    /// Removes and returns a WebView by its engine-level ID.
    ///
    /// The relative order of all remaining WebViews is preserved.
    pub fn remove(&mut self, id: WebViewId) -> Option<WebView> {
        let index = *self.id_index.get(&id)?;

        self.remove_at(index)
    }

    /// Removes and returns a WebView by its label.
    ///
    /// The relative order of all remaining WebViews is preserved.
    pub fn remove_by_label(&mut self, label: &str) -> Option<WebView> {
        let index = *self.label_index.get(label)?;

        self.remove_at(index)
    }

    /// Removes a WebView at the specified index and rebuilds the affected
    /// lookup indices.
    fn remove_at(&mut self, index: usize) -> Option<WebView> {
        if index >= self.webviews.len() {
            return None;
        }

        let webview = self.webviews.remove(index);

        self.id_index.remove(&webview.id());
        self.label_index.remove(webview.label());

        // `Vec::remove` shifts every element after `index` one position
        // to the left. Update both lookup maps to keep them synchronized
        // with the WebView collection.
        for current_index in index..self.webviews.len() {
            let current = &self.webviews[current_index];

            self.id_index.insert(current.id(), current_index);

            self.label_index.insert(current.label().to_string(), current_index);
        }

        Some(webview)
    }

    // =========================================================================
    // Collections
    // =========================================================================

    /// Returns all managed WebViews as a contiguous slice.
    pub fn webviews(&self) -> &[WebView] { &self.webviews }

    /// Returns all managed WebViews as a mutable slice.
    ///
    /// Callers must not modify properties used as lookup keys, such as the
    /// WebView ID or label, without updating the manager indices accordingly.
    pub fn webviews_mut(&mut self) -> &mut [WebView] { &mut self.webviews }

    /// Returns the number of registered WebViews.
    pub fn len(&self) -> usize { self.webviews.len() }

    /// Returns whether no WebViews are currently registered.
    pub fn is_empty(&self) -> bool { self.webviews.is_empty() }

    /// Returns whether more than one WebView is currently registered.
    pub fn has_multiple_webviews(&self) -> bool { self.webviews.len() > 1 }

    // =========================================================================
    // Clear
    // =========================================================================

    /// Removes all registered WebViews and clears all lookup indices.
    pub fn clear(&mut self) {
        self.webviews.clear();
        self.id_index.clear();
        self.label_index.clear();
    }
}

pub fn apply_build_webview<'a>(
    window: &Window,
    webview_builder: wry::WebViewBuilder<'a>,
    webview_label: &str,
    child: bool,
) -> Result<wry::WebView> {
    let webview = match child {
        #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "ios", target_os = "android")))]
        true => {
            let vbox = window.default_vbox().ok_or_else(|| {
                anyhow::anyhow!(
                    "failed to create child WebView `{}`: \
                     window does not provide a GTK default vbox",
                    webview_label
                )
            })?;
            webview_builder.build_gtk(vbox)
        }
        #[cfg(any(target_os = "windows", target_os = "macos", target_os = "ios", target_os = "android"))]
        true => webview_builder.build_as_child(window),
        false => {
            #[cfg(any(target_os = "windows", target_os = "macos", target_os = "ios", target_os = "android"))]
            let builder = webview_builder.build(window);
            #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "ios", target_os = "android")))]
            let builder = {
                let vbox = window.default_vbox().ok_or_else(|| {
                    anyhow::anyhow!(
                        "failed to create WebView `{}`: \
                         window does not provide a GTK default vbox",
                        webview_label
                    )
                })?;
                webview_builder.build_gtk(vbox)
            };
            builder
        }
    }
    .map_err(|error| anyhow::anyhow!("failed to build WebView `{}`: {error}", webview_label))?;
    if !child {
        #[cfg(any(
            target_os = "linux",
            target_os = "dragonfly",
            target_os = "freebsd",
            target_os = "netbsd",
            target_os = "openbsd"
        ))]
        crate::platforms::undecorated_resizing::attach_resize_handler(&webview);

        #[cfg(windows)]
        if window.is_resizable() && !window.is_decorated() {
            crate::platforms::undecorated_resizing::attach_resize_handler(
                window.hwnd(),
                window.has_undecorated_shadow(),
            );
        }
    }

    Ok(webview)
}

pub fn apply_webview_bounds<'a>(
    window: &'a Window,
    mut webview_builder: WebViewBuilder<'a>,
    bounds: Option<Rect>,
    auto_resize: bool,
    child: bool,
) -> (Option<WebviewBounds>, WebViewBuilder<'a>) {
    let webview_bounds = if let Some(bounds) = bounds {
        let bounds: RectWrapper = bounds.into();
        let bounds = bounds.0;
        let scale_factor = window.scale_factor();
        let position = bounds.position.to_logical::<f32>(scale_factor);
        let size = bounds.size.to_logical::<f32>(scale_factor);
        webview_builder = webview_builder.with_bounds(bounds);
        let window_size = window.inner_size().to_logical::<f32>(scale_factor);
        if auto_resize {
            Some(WebviewBounds {
                x_rate: position.x / window_size.width,
                y_rate: position.y / window_size.height,
                width_rate: size.width / window_size.width,
                height_rate: size.height / window_size.height,
            })
        } else {
            None
        }
    } else {
        if child {
            webview_builder = webview_builder.with_bounds(wry::Rect {
                position: LogicalPosition::new(0, 0).into(),
                size: window.inner_size().into(),
            });
            Some(WebviewBounds { x_rate: 0., y_rate: 0., width_rate: 1., height_rate: 1. })
        } else {
            None
        }
    };
    (webview_bounds, webview_builder)
}
