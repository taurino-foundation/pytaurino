/*
#[derive(Clone)]
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
    register_webview_events(
      window_id.clone(),
      webview_id,
      webview_label.clone(),
      focused_webview,
      &webview,
      proxy,
    );

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
  pub fn id(&self) -> WebViewId {
    self.id
  }
  pub fn label(&self) -> &str {
    &self.label
  }
  // --------------------------------------------------------
  // Window association
  // --------------------------------------------------------
  pub fn window_id(&self) -> WindowId {
    *self
      .window_id
      .lock()
      .expect("WebView window_id mutex is poisoned")
  }
  pub fn window_id_handle(&self) -> Arc<Mutex<WindowId>> {
    Arc::clone(&self.window_id)
  }
  /// Changes only the stored window ID.
  ///
  /// The native WebView is not moved by this operation.
  /// For an actual window change, normally use `reparent()`.
  pub fn set_window_id(&self, window_id: WindowId) {
    *self
      .window_id
      .lock()
      .expect("WebView window_id mutex is poisoned") = window_id;
  }
  // --------------------------------------------------------
  // Native WebView
  // --------------------------------------------------------
  /// Returns the existing Rc without cloning it.
  ///
  /// An externally created Rc clone must not outlive all WebView wrappers if
  /// their drop is to reliably remove the context reference.
  pub fn inner(&self) -> &Rc<wry::WebView> {
    &self.inner
  }
  /// Unique access to Wry, even in the presence of identically named methods.
  pub fn as_wry(&self) -> &wry::WebView {
    self.inner.as_ref()
  }
  /// Existing low-level access from the original API.
  ///
  /// Replacing the Rc updates neither context references nor window
  /// association nor bounds.
  #[deprecated(note = "Replacing inner bypasses context management. \
                For normal Wry calls use as_wry().")]
  pub fn inner_mut(&mut self) -> &mut Rc<wry::WebView> {
    &mut self.inner
  }
  // --------------------------------------------------------
  // Context
  // --------------------------------------------------------
  /// Returns the unmodified key used for the context store.
  pub fn context_key(&self) -> &Option<PathBuf> {
    &self.context_key
  }
  pub fn context_store(&self) -> &WebContextStore {
    &self.context_store
  }
  /// Clones the shared store handle.
  pub fn context_store_handle(&self) -> WebContextStore {
    self.context_store.clone()
  }
  // --------------------------------------------------------
  // Cached layout bounds
  // --------------------------------------------------------
  /// Reads the stored bounds without removing them.
  ///
  /// This is NOT wry::WebView::bounds().
  /// WebviewBounds must implement Clone.
  pub fn bounds(&self) -> Option<WebviewBounds> {
    self
      .bounds
      .lock()
      .expect("WebView bounds mutex is poisoned")
      .clone()
  }
  pub fn bounds_handle(&self) -> Arc<Mutex<Option<WebviewBounds>>> {
    Arc::clone(&self.bounds)
  }
  /// Changes only the stored layout data.
  ///
  /// The position and size of the native WebView remain unchanged.
  pub fn set_cached_bounds(&self, bounds: Option<WebviewBounds>) {
    *self
      .bounds
      .lock()
      .expect("WebView bounds mutex is poisoned") = bounds;
  }
  /// Takes the stored bounds.
  ///
  /// Afterwards all wrapper clones hold None at this location.
  pub fn take_bounds(&self) -> Option<WebviewBounds> {
    self
      .bounds
      .lock()
      .expect("WebView bounds mutex is poisoned")
      .take()
  }
  pub fn clear_bounds(&self) {
    self.set_cached_bounds(None);
  }
  // --------------------------------------------------------
  // Actual native geometry
  // --------------------------------------------------------
  /// Queries the current geometry directly from Wry.
  pub fn native_bounds(&self) -> Result<wry::Rect> {
    self.as_wry().bounds().map_err(|error| {
      anyhow!(
        "failed to read native bounds for webview '{}': {error}",
        self.label
      )
    })
  }

  /// Changes the native geometry.
  ///
  /// The stored WebviewBounds are not adjusted automatically: their
  /// conversion belongs in your layout code.
  pub fn set_window_bounds(&self, bounds: wry::Rect) -> Result<()> {
    self.as_wry().set_bounds(bounds).map_err(|error| {
      anyhow!(
        "failed to set native bounds for webview '{}': {error}",
        self.label
      )
    })
  }
}
// ------------------------------------------------------------
// Trait implementations
// ------------------------------------------------------------
impl Deref for WebView {
  type Target = wry::WebView;
  fn deref(&self) -> &Self::Target {
    self.as_wry()
  }
}
impl AsRef<wry::WebView> for WebView {
  fn as_ref(&self) -> &wry::WebView {
    self.as_wry()
  }
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





*/
