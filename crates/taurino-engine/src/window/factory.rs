#[cfg(windows)]
use taurino_config::{ArcMut, FocusState};
#[cfg(windows)]
use taurino_core::platforms::windows::apply_shadow_correction;
#[cfg(any(target_os = "windows", target_os = "android"))]
use wry::WebViewBuilderExtWindows;
use {
    crate::app::{index::HTML, manager::RuntimeManager},
    anyhow::{Result, anyhow},
    dpi::PhysicalSize,
    std::{
        collections::{
            HashSet,
            hash_map::Entry::{Occupied, Vacant},
        },
        sync::Arc,
    },
    taurino_config::{
        EngineWindowTarget, WebContext, arc_mut,
        dispatcher::{all_initialization_scripts, install_ipc_bridge},
        webview::{BackgroundThrottlingPolicy, WebViewConfig, WebViewId},
        window::{WindowConfig, WindowId},
    },
    taurino_core::{
        platforms::{MonitorExt, util::calculate_window_center_position},
        webview::{WebViewManager, apply_build_webview, apply_webview_bounds},
        window::{Window, WindowBuilder, find_monitor_for_position},
    },
    wry::{WebContext as WryContext, WebViewBuilder},
};
pub fn create_window(
    event_loop: &EngineWindowTarget,
    manager: Arc<RuntimeManager>,
    id: WindowId,
    window_options: &WindowConfig,
) -> Result<Arc<Window>> {
    let _proxy = manager.proxy.clone();
    let mut window_builder = WindowBuilder::with_config(window_options);
    if window_builder.prevent_overflow.is_some() || window_builder.center {
        let monitor = if let Some(window_position) = &window_builder.inner.window.position {
            find_monitor_for_position(event_loop.available_monitors(), *window_position)
        } else {
            event_loop.primary_monitor()
        };
        if let Some(monitor) = monitor {
            let scale_factor = monitor.scale_factor();
            let desired_size =
                window_builder.inner.window.inner_size.unwrap_or_else(|| PhysicalSize::new(800, 600).into());
            let mut inner_size = window_builder
                .inner
                .window
                .inner_size_constraints
                .clamp(desired_size, scale_factor)
                .to_physical::<u32>(scale_factor);
            let mut window_size = inner_size;
            #[cfg(windows)]
            let decorations = window_builder.inner.window.decorations;
            // Left and right window shadow counts as part of the window on Windows
            // We need to include it when calculating positions, but not size
            #[cfg(windows)]
            let shadow_width = apply_shadow_correction(decorations, &mut window_size)?;
            #[cfg(not(windows))]
            let shadow_width = 0;
            if let Some(margin) = window_builder.prevent_overflow {
                let work_area = monitor.work_area();
                let margin = margin.to_physical::<u32>(scale_factor);
                let constraint =
                    PhysicalSize::new(work_area.size.width - margin.width, work_area.size.height - margin.height);
                if window_size.width > constraint.width || window_size.height > constraint.height {
                    if window_size.width > constraint.width {
                        inner_size.width = inner_size.width.saturating_sub(window_size.width - constraint.width);
                        window_size.width = constraint.width;
                    }
                    if window_size.height > constraint.height {
                        inner_size.height = inner_size.height.saturating_sub(window_size.height - constraint.height);
                        window_size.height = constraint.height;
                    }
                    window_builder.inner.window.inner_size = Some(inner_size.into());
                }
            }
            if window_builder.center {
                window_size.width += shadow_width;
                let position = calculate_window_center_position(window_size, monitor);
                let logical_position = position.to_logical::<f64>(scale_factor);
                window_builder = window_builder.position(logical_position.x, logical_position.y);
            }
        }
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        let (initial_position, is_fullscreen) =
            (window_builder.inner.window.position, window_builder.inner.window.fullscreen.is_some());
        // If fullscreen is requested with an explicit position, resolve the target
        // monitor up front so the window is created fullscreen on that display.
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        if let (true, Some(position)) = (is_fullscreen, initial_position) {
            if let Some(target_monitor) = find_monitor_for_position(window_target.available_monitors(), position) {
                window_builder.inner.window.fullscreen = Some(Fullscreen::Borderless(Some(target_monitor)));
            }
        }
    };
    #[cfg(windows)]
    let background_color = window_builder.inner.window.background_color;
    #[cfg(windows)]
    let is_window_transparent = window_builder.inner.window.transparent;
    #[cfg(target_os = "macos")]
    {
        if window_builder.tabbing_identifier.is_none()
            || window_builder.inner.window.transparent
            || !window_builder.inner.window.decorations
        {
            window_builder.inner = window_builder.inner.with_automatic_window_tabbing(false);
        }
    }
    let window = window_builder.inner.build(event_loop)?;

    // On macOS, `with_position` uses the content origin; the title bar is added
    // above it. `set_outer_position` is needed for precise window placement.
    #[cfg(target_os = "macos")]
    if !is_fullscreen {
        if let Some(position) = initial_position {
            window.set_outer_position(position);
        }
    }
    #[cfg(windows)]
    let focused_webview = arc_mut(FocusState::default());
    let mut webviews_manager = WebViewManager::new()?;
    let window_id = arc_mut(id);
    let has_children = window_options.webviews.len() > 1;
    for view_options in &window_options.webviews {
        let mut view_options = view_options.clone();
        // If only a single WebView exists in the list and the user has still set
        // `child` to `true`, we must ensure that it is reset to `false`, because
        // a single WebView in the list cannot be a child WebView of a window.
        if !has_children {
            view_options.child = false;
        }
        attach_webview(
            &mut webviews_manager,
            &window,
            &view_options,
            window_options,
            manager.clone(),
            window_id.clone(),
            #[cfg(windows)]
            focused_webview.clone(),
        )?;
    }
    Ok(Window::new(
        window,
        id,
        window_options.label.clone(),
        #[cfg(windows)]
        background_color,
        #[cfg(windows)]
        is_window_transparent,
        #[cfg(windows)]
        focused_webview,
        has_children,
        webviews_manager,
    ))
}

pub(crate) fn create_webview(
    manager: Arc<RuntimeManager>,
    window_id: ArcMut<WindowId>,
    id: WebViewId,
    options: &WebViewConfig,
    window_options: &WindowConfig,
    window: &tao::window::Window,
    #[cfg(windows)] focused_webview: ArcMut<FocusState>,
) -> Result<taurino_core::webview::WebView> {
    let window_label = window_options.label.clone();
    let webview_label = options.label.clone();
    let child = options.child;
    let all_initialization_scripts =
        all_initialization_scripts(&window_label, &webview_label, options.use_https_scheme, None)?;
    let config = manager.config()?;
    let proxy = manager.proxy.clone();
    let mut web_context = config.context.lock().expect("poisoned WebContext store");
    let web_context_key =
        config.prepare_webview_data_directory(&window_options.label, options.data_directory.clone())?;

    let is_first_context = web_context.is_empty();
    // the context must be stored on the HashMap because it must outlive the WebView on macOS
    let automation_enabled = std::env::var("TAURINO_WEBVIEW_AUTOMATION").as_deref() == Ok("true");
    let entry = web_context.entry(web_context_key.clone());
    let web_context = match entry {
        Occupied(occupied) => {
            let occupied = occupied.into_mut();
            occupied.referenced_by_webviews.insert(webview_label.clone());
            occupied
        }
        Vacant(vacant) => {
            let mut web_context = WryContext::new(web_context_key.clone());
            web_context.set_allows_automation(if automation_enabled { is_first_context } else { false });
            vacant.insert(WebContext {
                inner: web_context,
                referenced_by_webviews: [webview_label.clone()].into(),
                registered_custom_protocols: HashSet::new(),
            })
        }
    };
    let mut webview_builder = WebViewBuilder::new_with_web_context(&mut web_context.inner)
        .with_id(&webview_label)
        .with_focused(window_options.focus)
        .with_transparent(window_options.transparent)
        .with_accept_first_mouse(window_options.accept_first_mouse)
        .with_incognito(options.incognito)
        .with_clipboard(options.enable_clipboard_access)
        .with_hotkeys_zoom(options.zoom_hotkeys_enabled)
        .with_general_autofill_enabled(options.general_autofill_enabled);

    #[cfg(target_os = "macos")]
    if let Some(webview_configuration) = options.webview_configuration {
        webview_builder = webview_builder.with_webview_configuration(webview_configuration);
    }

    #[cfg(any(target_os = "windows", target_os = "android"))]
    {
        webview_builder = webview_builder.with_https_scheme(options.use_https_scheme);
    }
    let (webview_bounds, mut webview_builder) =
        apply_webview_bounds(window, webview_builder, options.bounds, options.auto_resize, child);
    if let Some(background_throttling) = options.background_throttling.as_ref() {
        webview_builder = webview_builder.with_background_throttling(match background_throttling {
            BackgroundThrottlingPolicy::Disabled => wry::BackgroundThrottlingPolicy::Disabled,
            BackgroundThrottlingPolicy::Suspend => wry::BackgroundThrottlingPolicy::Suspend,
            BackgroundThrottlingPolicy::Throttle => wry::BackgroundThrottlingPolicy::Throttle,
        });
    }
    webview_builder = install_ipc_bridge(webview_builder, webview_label.clone(), proxy.clone());
    webview_builder = webview_builder.with_html(HTML);
    if options.javascript_disabled {
        webview_builder = webview_builder.with_javascript_disabled();
    }
    if let Some(color) = window_options.background_color {
        webview_builder = webview_builder.with_background_color(color.into());
    }
    for initialization_script in all_initialization_scripts {
        webview_builder = if initialization_script.for_main_frame_only {
            webview_builder.with_initialization_script_for_main_only(initialization_script.script, true)
        } else {
            webview_builder.with_initialization_script(initialization_script.script)
        };
    }
    let webview = apply_build_webview(window, webview_builder, &options.label, child)?;
    // webview.evaluate_script("console.log(window.__TAURINO_INTERNALS__)")?;
    let webview = taurino_core::webview::WebView::new(
        id,
        webview_label,
        window_id,
        focused_webview,
        webview,
        if automation_enabled { None } else { web_context_key },
        config.context.clone(),
        arc_mut(webview_bounds),
        proxy,
    );
    Ok(webview)
}

// =========================================================================
// Creation
// =========================================================================

/// Creates and registers a new WebView.
///
/// The WebView is inserted only after its native creation succeeds.
///
/// # Errors
///
/// Returns an error if:
///
/// - another WebView already uses the requested label,
/// - the generated WebView ID is already registered,
/// - native WebView creation fails.
pub fn attach_webview(
    webview_manager: &mut WebViewManager,
    window: &tao::window::Window,
    options: &WebViewConfig,
    window_options: &WindowConfig,
    manager: Arc<RuntimeManager>,
    window_id: ArcMut<WindowId>,
    #[cfg(windows)] focused_webview: ArcMut<FocusState>,
) -> Result<WebViewId> {
    if webview_manager.contains_label(&options.label) {
        return Err(anyhow!("WebView with label {:?} is already registered", options.label));
    }

    let id = webview_manager.next_webview_id();

    if webview_manager.contains(id) {
        return Err(anyhow!("WebView with id {:?} is already registered", id));
    }

    let webview = create_webview(
        manager,
        window_id,
        id,
        options,
        window_options,
        window,
        #[cfg(windows)]
        focused_webview,
    )
    .map_err(|error| anyhow!("failed to create WebView {:?}: {error}", options.label))?;

    webview_manager.insert(webview)?;

    Ok(id)
}
