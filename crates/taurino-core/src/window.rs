#[cfg(windows)]
// Import path corrected: `TitleBarStyle` lives in `config`, not in `types`.
#[cfg(target_os = "macos")]
use crate::config::TitleBarStyle;
#[cfg(windows)]
use crate::platforms::WindowExt;
#[cfg(windows)]
use crate::platforms::undecorated_resizing;
#[cfg(windows)]
use softbuffer;
#[cfg(windows)]
use std::sync::Arc;
#[cfg(windows)]
use std::sync::Mutex;
#[cfg(target_os = "android")]
use tao::platform::android::WindowExtAndroid;
#[cfg(target_os = "ios")]
use tao::platform::ios::WindowExtIOS;
#[cfg(target_os = "macos")]
use tao::platform::macos::WindowBuilderExtMacOS;
#[cfg(target_os = "macos")]
use tao::platform::macos::WindowExtMacOS;
#[cfg(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd"
))]
use tao::platform::unix::WindowBuilderExtUnix;
#[cfg(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd"
))]
use tao::platform::unix::WindowExtUnix;
#[cfg(windows)]
use tao::platform::windows::WindowBuilderExtWindows;
#[cfg(windows)]
use tao::platform::windows::WindowExtWindows;
#[cfg(windows)]
use taurino_config::FocusState;
#[cfg(windows)]
use taurino_config::arc_mut;
/* pub mod util;
use crate::menu::{
    WindowMenu,
    prelude::{
        CheckMenuItem, IconMenuItem, IsMenuItem, Menu, MenuItem, MenuItemKind, NativeIcon,
        PredefinedMenuItem, Submenu,
    },
}; */
#[cfg(windows)]
use windows::Win32::Foundation::HWND;
use {
    crate::{
        map_from_theme,
        webview::{WebView, WebViewManager},
        wrappers::{
            CursorIconWrapper, MonitorHandleWrapper, ProgressBarStateWrapper, TaoIcon, UserAttentionTypeWrapper,
        },
    },
    anyhow::{Result, anyhow},
    dpi::{LogicalPosition, LogicalSize, PhysicalPosition, PhysicalSize, Position, Size},
    raw_window_handle::{DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, WindowHandle},
    std::{
        fmt,
        sync::atomic::{AtomicBool, Ordering},
    },
    tao::window::{Fullscreen, Theme as TaoTheme, Window as TaoWindow, WindowBuilder as TaoWindowBuilder},
    taurino_config::{
        Color, Theme,
        utils::lock_state,
        webview::WebViewId,
        window::{
            CursorIcon, Icon, Monitor, PreventOverflowConfig, ProgressBarState, ResizeDirection, UserAttentionType,
            WindowConfig, WindowId, WindowSizeConstraints,
        },
    },
};

#[cfg(windows)]
type WindowSurface = softbuffer::Surface<Arc<TaoWindow>, Arc<TaoWindow>>;

#[cfg(windows)]
type SharedWindowSurface = Arc<Mutex<Option<WindowSurface>>>;

/* use taurino_config::{arc_mut, map_theme}; */
/* use crate::schema::{
    Color,  Theme,
    webview::WebViewId,
    window::{
        CursorIcon, Icon, Monitor, ProgressBarState, ResizeDirection, UserAttentionType,
        WindowId, WindowSizeConstraints,
    },
}; */
/// Builder used to configure and create a native application window.
///
/// It wraps Tao's `WindowBuilder` and adds Taurino-specific configuration
/// such as centering, overflow prevention, and macOS tabbing identity.
#[derive(Clone, Default)]
pub struct WindowBuilder {
    /// Underlying Tao window builder that receives the platform configuration.
    pub inner: TaoWindowBuilder,
    /// Whether the window should be centered after creation.
    pub center: bool,
    /// Optional margin used to keep the window inside the monitor's working area.
    pub prevent_overflow: Option<Size>,
    /// macOS tabbing identifier used to group windows into native tabs.
    #[cfg(target_os = "macos")]
    pub tabbing_identifier: Option<String>,
}

impl std::fmt::Debug for WindowBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut s = f.debug_struct("WindowBuilder");
        s.field("inner", &self.inner).field("center", &self.center).field("prevent_overflow", &self.prevent_overflow);
        #[cfg(target_os = "macos")]
        {
            s.field("tabbing_identifier", &self.tabbing_identifier);
        }
        s.finish()
    }
}

impl WindowBuilder {
    /// Creates a new builder with sensible defaults.
    /// /// The builder starts focused, is titled "Taurino App", and applies /// platform-specific defaults (e.g. a visible title bar style on macOS  /// and a window class name on Windows).
    pub fn new() -> Self {
        #[allow(unused_mut)]
        let mut builder = Self::default().focused(true);
        #[cfg(target_os = "macos")]
        {
            // TODO: find a proper way to prevent webview being pushed out of the window.
            // Workaround for issue: https://github.com/tauri-apps/tauri/issues/10225
            // The window requires `NSFullSizeContentViewWindowMask` flag to prevent devtools
            // pushing the content view out of the window.
            // By setting the default style to `TitleBarStyle::Visible` should fix the issue for most of the users.
            builder = builder.title_bar_style(TitleBarStyle::Visible);
        }
        builder = builder.title("Taurino App");
        #[cfg(windows)]
        {
            builder = builder.window_classname("Taurino Window");
        }
        builder
    }
    pub fn with_config(config: &WindowConfig) -> Self {
        let mut window = WindowBuilder::new();

        #[cfg(target_os = "macos")]
        {
            window = window.hidden_title(config.hidden_title).title_bar_style(config.title_bar_style);
            if let Some(identifier) = &config.tabbing_identifier {
                window = window.tabbing_identifier(identifier);
            }
            if let Some(position) = &config.traffic_light_position {
                window = window.traffic_light_position(crate::schema::LogicalPosition::new(position.x, position.y));
            }
        }

        #[cfg(any(
            target_os = "linux",
            target_os = "dragonfly",
            target_os = "freebsd",
            target_os = "netbsd",
            target_os = "openbsd"
        ))]
        {
            // Mouse event is disabled on Linux since sudden event bursts could block event loop.
            window.inner = window.inner.with_cursor_moved_event(false);
        }

        #[cfg(target_os = "android")]
        {
            if let Some(activity_name) = &config.activity_name {
                window.inner = window.inner.with_activity_name(activity_name.clone());
            }
            if let Some(activity_name) = &config.created_by_activity_name {
                window.inner = window.inner.with_created_by_activity_name(activity_name.clone());
            }
        }

        #[cfg(target_os = "ios")]
        {
            if let Some(scene_identifier) = &config.requested_by_scene_identifier {
                window.inner = window.inner.with_requesting_scene_identifier(scene_identifier.clone());
            }
        }

        // ignore size from config for mobile for backward compatibility
        #[cfg(not(any(target_os = "ios", target_os = "android")))]
        {
            window = window.inner_size(config.width, config.height);
        }

        window = window
            .title(config.title.to_string())
            .focused(config.focus)
            .focusable(config.focusable)
            .visible(config.visible)
            .resizable(config.resizable)
            .fullscreen(config.fullscreen)
            .decorations(config.decorations)
            .maximized(config.maximized)
            .always_on_bottom(config.always_on_bottom)
            .always_on_top(config.always_on_top)
            .visible_on_all_workspaces(config.visible_on_all_workspaces)
            .content_protected(config.content_protected)
            .skip_taskbar(config.skip_taskbar)
            .theme(config.theme)
            .no_redirection_bitmap(config.no_redirection_bitmap)
            .closable(config.closable)
            .maximizable(config.maximizable)
            .minimizable(config.minimizable)
            .shadow(config.shadow)
            .transparent(config.transparent);

        let mut constraints = WindowSizeConstraints::default();

        if let Some(min_width) = config.min_width {
            constraints.min_width = Some(tao::dpi::LogicalUnit::new(min_width).into());
        }
        if let Some(min_height) = config.min_height {
            constraints.min_height = Some(tao::dpi::LogicalUnit::new(min_height).into());
        }
        if let Some(max_width) = config.max_width {
            constraints.max_width = Some(tao::dpi::LogicalUnit::new(max_width).into());
        }
        if let Some(max_height) = config.max_height {
            constraints.max_height = Some(tao::dpi::LogicalUnit::new(max_height).into());
        }
        if let Some(color) = config.background_color {
            window = window.background_color(color);
        }
        window = window.inner_size_constraints(constraints);

        if let (Some(x), Some(y)) = (config.x, config.y) {
            window = window.position(x, y);
        }

        if config.center {
            window = window.center();
        }

        if let Some(window_classname) = &config.window_classname {
            window = window.window_classname(window_classname);
        }

        if let Some(prevent_overflow) = &config.prevent_overflow {
            window = match prevent_overflow {
                PreventOverflowConfig::Enable(true) => window.prevent_overflow(),
                PreventOverflowConfig::Margin(margin) => {
                    window.prevent_overflow_with_margin(PhysicalSize::new(margin.width, margin.height).into())
                }
                _ => window,
            };
        }

        window
    }
    /// Marks the window to be centered after creation.
    /// /// The actual centering is performed later by the window creation logic.
    pub fn center(mut self) -> Self {
        self.center = true;
        self
    }
    /// Sets the initial outer position of the window in logical coordinates.
    pub fn position(mut self, x: f64, y: f64) -> Self {
        self.inner = self.inner.with_position(LogicalPosition::new(x, y));
        self
    }
    /// Sets the initial inner (client-area) size in logical coordinates.
    pub fn inner_size(mut self, width: f64, height: f64) -> Self {
        self.inner = self.inner.with_inner_size(LogicalSize::new(width, height));
        self
    }
    /// Sets the minimum inner size of the window in logical coordinates.
    pub fn min_inner_size(mut self, min_width: f64, min_height: f64) -> Self {
        self.inner = self.inner.with_min_inner_size(LogicalSize::new(min_width, min_height));
        self
    }
    /// Sets the maximum inner size of the window in logical coordinates.
    pub fn max_inner_size(mut self, max_width: f64, max_height: f64) -> Self {
        self.inner = self.inner.with_max_inner_size(LogicalSize::new(max_width, max_height));
        self
    }
    /// Applies a complete set of min/max inner size constraints at once.
    pub fn inner_size_constraints(mut self, constraints: WindowSizeConstraints) -> Self {
        self.inner.window.inner_size_constraints = tao::window::WindowSizeConstraints {
            min_width: constraints.min_width,
            min_height: constraints.min_height,
            max_width: constraints.max_width,
            max_height: constraints.max_height,
        };
        self
    }
    /// Prevent the window from overflowing the working area (e.g. monitor size - taskbar size) on creation
    /// /// ## Platform-specific
    /// /// - **iOS / Android:** Unsupported.
    pub fn prevent_overflow(mut self) -> Self {
        self.prevent_overflow.replace(PhysicalSize::new(0, 0).into());
        self
    }
    /// Prevent the window from overflowing the working area (e.g. monitor size - taskbar size) /// on creation with a margin
    /// /// ## Platform-specific
    /// /// - **iOS / Android:** Unsupported.
    pub fn prevent_overflow_with_margin(mut self, margin: Size) -> Self {
        self.prevent_overflow.replace(margin);
        self
    }
    /// Enables or disables the resizable flag of the window.
    pub fn resizable(mut self, resizable: bool) -> Self {
        self.inner = self.inner.with_resizable(resizable);
        self
    }
    /// Enables or disables the native maximize action.
    pub fn maximizable(mut self, maximizable: bool) -> Self {
        self.inner = self.inner.with_maximizable(maximizable);
        self
    }
    /// Enables or disables the native minimize action.
    pub fn minimizable(mut self, minimizable: bool) -> Self {
        self.inner = self.inner.with_minimizable(minimizable);
        self
    }
    /// Enables or disables the native close action.
    pub fn closable(mut self, closable: bool) -> Self {
        self.inner = self.inner.with_closable(closable);
        self
    }
    /// Sets the initial native window title.
    pub fn title<S: Into<String>>(mut self, title: S) -> Self {
        self.inner = self.inner.with_title(title.into());
        self
    }
    /// Configures the window to start in borderless fullscreen mode or in  /// windowed mode.
    pub fn fullscreen(mut self, fullscreen: bool) -> Self {
        self.inner = if fullscreen {
            self.inner.with_fullscreen(Some(Fullscreen::Borderless(None)))
        } else {
            self.inner.with_fullscreen(None)
        };
        self
    }
    /// Sets whether the window should receive focus on creation.
    pub fn focused(mut self, focused: bool) -> Self {
        self.inner = self.inner.with_focused(focused);
        self
    }
    /// Sets whether the window is allowed to be focused at all.
    pub fn focusable(mut self, focusable: bool) -> Self {
        self.inner = self.inner.with_focusable(focusable);
        self
    }
    /// Sets whether the window should start maximized.
    pub fn maximized(mut self, maximized: bool) -> Self {
        self.inner = self.inner.with_maximized(maximized);
        self
    }
    /// Sets whether the window should be visible immediately after creation.
    pub fn visible(mut self, visible: bool) -> Self {
        self.inner = self.inner.with_visible(visible);
        self
    }
    /// Enables or disables transparent window rendering.
    pub fn transparent(mut self, transparent: bool) -> Self {
        self.inner = self.inner.with_transparent(transparent);
        self
    }
    /// Enables or disables native window decorations (title bar and borders).
    pub fn decorations(mut self, decorations: bool) -> Self {
        self.inner = self.inner.with_decorations(decorations);
        self
    }
    /// Configures whether the window stays below normal windows.
    pub fn always_on_bottom(mut self, always_on_bottom: bool) -> Self {
        self.inner = self.inner.with_always_on_bottom(always_on_bottom);
        self
    }
    /// Configures whether the window stays above normal windows.
    pub fn always_on_top(mut self, always_on_top: bool) -> Self {
        self.inner = self.inner.with_always_on_top(always_on_top);
        self
    }
    /// Configures whether the window is visible on all workspaces.
    pub fn visible_on_all_workspaces(mut self, visible_on_all_workspaces: bool) -> Self {
        self.inner = self.inner.with_visible_on_all_workspaces(visible_on_all_workspaces);
        self
    }
    /// Enables or disables protection against capture of the window contents by other apps.
    pub fn content_protected(mut self, protected: bool) -> Self {
        self.inner = self.inner.with_content_protection(protected);
        self
    }
    /// Enables or disables the native window shadow on supported platforms.
    /// /// On Windows this maps to the undecorated shadow; on macOS this maps to    /// the native window shadow flag.
    pub fn shadow(#[allow(unused_mut)] mut self, _enable: bool) -> Self {
        #[cfg(windows)]
        {
            self.inner = self.inner.with_undecorated_shadow(_enable);
        }
        #[cfg(target_os = "macos")]
        {
            self.inner = self.inner.with_has_shadow(_enable);
        }
        self
    }
    /// Sets the native owner window on Windows.
    #[cfg(windows)]
    pub fn owner(mut self, owner: HWND) -> Self {
        self.inner = self.inner.with_owner_window(owner.0 as _);
        self
    }
    /// Sets the native parent window on Windows.
    #[cfg(windows)]
    pub fn parent(mut self, parent: HWND) -> Self {
        self.inner = self.inner.with_parent_window(parent.0 as _);
        self
    }
    /// Sets the native parent window on macOS.
    #[cfg(target_os = "macos")]
    pub fn parent(mut self, parent: *mut std::ffi::c_void) -> Self {
        self.inner = self.inner.with_parent_window(parent);
        self
    }
    /// Sets the transient parent window on Linux/BSD (GTK).
    #[cfg(any(
        target_os = "linux",
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd"
    ))]
    pub fn transient_for(mut self, parent: &impl gtk::glib::IsA<gtk::Window>) -> Self {
        self.inner = self.inner.with_transient_for(parent);
        self
    }
    /// Enables or disables native drag-and-drop on Windows.
    #[cfg(windows)]
    pub fn drag_and_drop(mut self, enabled: bool) -> Self {
        use tao::platform::windows::WindowBuilderExtWindows as _;

        self.inner = self.inner.with_drag_and_drop(enabled);
        self
    }
    /// Applies the selected macOS title-bar transparency and content layout behavior.
    /// /// The three variants control title-bar transparency and fullsize content   /// view independently to achieve visible, transparent or overlay styles.
    #[cfg(target_os = "macos")]
    pub fn title_bar_style(mut self, style: TitleBarStyle) -> Self {
        match style {
            TitleBarStyle::Visible => {
                self.inner = self.inner.with_titlebar_transparent(false);
                // Fixes rendering issue when resizing window with devtools open (https://github.com/tauri-apps/tauri/issues/3914)
                self.inner = self.inner.with_fullsize_content_view(true);
            }
            TitleBarStyle::Transparent => {
                self.inner = self.inner.with_titlebar_transparent(true);
                self.inner = self.inner.with_fullsize_content_view(false);
            }
            TitleBarStyle::Overlay => {
                self.inner = self.inner.with_titlebar_transparent(true);
                self.inner = self.inner.with_fullsize_content_view(true);
            }
            #[allow(unreachable_patterns)]
            unknown => {
                eprintln!("unknown title bar style applied: {unknown}");
            }
        }
        self
    }
    /// Moves the macOS traffic-light window controls to the supplied inset position.
    #[cfg(target_os = "macos")]
    pub fn traffic_light_position<P: Into<Position>>(mut self, position: P) -> Self {
        self.inner = self.inner.with_traffic_light_inset(position.into());
        self
    }
    /// Hides or shows the macOS window title while keeping the title bar.
    #[cfg(target_os = "macos")]
    pub fn hidden_title(mut self, hidden: bool) -> Self {
        self.inner = self.inner.with_title_hidden(hidden);
        self
    }
    /// Sets the macOS tabbing identifier used to group windows into native tabs.
    #[cfg(target_os = "macos")]
    pub fn tabbing_identifier(mut self, identifier: &str) -> Self {
        self.inner = self.inner.with_tabbing_identifier(identifier);
        self.tabbing_identifier.replace(identifier.into());
        self
    }
    /// Converts and applies the supplied image as the initial native window icon.
    pub fn icon(mut self, icon: Icon) -> anyhow::Result<Self> {
        self.inner = self.inner.with_window_icon(Some(TaoIcon::try_from(icon)?.0));
        Ok(self)
    }
    /// Sets the initial native background color of the window.
    pub fn background_color(mut self, color: Color) -> Self {
        self.inner = self.inner.with_background_color(color.into());
        self
    }
    /// Configures whether the window is omitted from the taskbar on supported platforms.
    #[cfg(any(
        windows,
        target_os = "linux",
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd"
    ))]
    pub fn skip_taskbar(mut self, skip: bool) -> Self {
        self.inner = self.inner.with_skip_taskbar(skip);
        self
    }
    /// No-op on macOS, iOS and Android, where taskbar skipping is unsupported.
    #[cfg(any(target_os = "macos", target_os = "ios", target_os = "android"))]
    pub fn skip_taskbar(self, _skip: bool) -> Self { self }
    /// Sets the preferred native window theme or restores system theme selection.
    pub fn theme(mut self, theme: Option<Theme>) -> Self {
        self.inner = self.inner.with_theme(if let Some(t) = theme {
            match t {
                Theme::Dark => Some(TaoTheme::Dark),
                _ => Some(TaoTheme::Light),
            }
        } else {
            None
        });
        self
    }
    /// Returns whether an initial window icon has been configured.
    pub fn has_icon(&self) -> bool { self.inner.window.window_icon.is_some() }
    /// Returns the currently configured preferred theme, if any.
    pub fn get_theme(&self) -> Option<Theme> {
        self.inner.window.preferred_theme.map(|theme| match theme {
            TaoTheme::Dark => Theme::Dark,
            _ => Theme::Light,
        })
    }
    /// Sets the Windows window class name.
    #[cfg(windows)]
    pub fn window_classname<S: Into<String>>(mut self, window_classname: S) -> Self {
        self.inner = self.inner.with_window_classname(window_classname);
        self
    }
    /// No-op on non-Windows platforms, where a window class name is not applicable.
    #[cfg(not(windows))]
    pub fn window_classname<S: Into<String>>(self, _window_classname: S) -> Self { self }
    /// Enables or disables the no-redirection-bitmap flag on Windows.
    /// /// This is a no-op on other platforms.
    pub fn no_redirection_bitmap(#[allow(unused_mut)] mut self, _enable: bool) -> Self {
        #[cfg(windows)]
        {
            self.inner = self.inner.with_no_redirection_bitmap(_enable);
        }
        self
    }
    /// Sets the Android activity name that should host the window.
    #[cfg(target_os = "android")]
    pub fn activity_name<S: Into<String>>(mut self, class_name: S) -> Self {
        self.inner = self.inner.with_activity_name(class_name.into());
        self
    }
    /// Sets the Android activity name that created the window.
    #[cfg(target_os = "android")]
    pub fn created_by_activity_name<S: Into<String>>(mut self, class_name: S) -> Self {
        self.inner = self.inner.with_created_by_activity_name(class_name.into());
        self
    }
    /// Sets the iOS scene identifier that requested the window.
    #[cfg(target_os = "ios")]
    pub fn requested_by_scene_identifier<S: Into<String>>(mut self, identifier: S) -> Self {
        self.inner = self.inner.with_requesting_scene_identifier(identifier.into());
        self
    }
}

pub struct Window {
    /// Manages all WebViews associated with this window.    /// WebViews are window-scoped resources and are released as part of the    /// controlled window destruction sequence.
    pub webviews_manager: WebViewManager,
    /// Drawing surface used for transparent-window rendering on Windows.
    /// /// The surface depends on the native Tao window and must therefore be   /// released before the locally owned native window handle during explicit /// destruction.
    #[cfg(windows)]
    pub surface: SharedWindowSurface,
    /// Stable application-visible label identifying this window.
    pub label: String,
    /// Taurino-level identifier assigned to this window. /// This is distinct from Tao's native [`tao::window::WindowId`].
    pub id: WindowId,
    /// Configured native background color on Windows. /// The value is shared with redraw handling for transparent windows.
    #[cfg(windows)]
    pub background_color: Arc<Mutex<Option<tao::window::RGBA>>>,
    /// Whether this window uses transparent native rendering on Windows.
    #[cfg(windows)]
    pub is_window_transparent: bool,
    /// Effective focus state for the window and its child WebViews on Windows. /// Tao's native focus state alone is insufficient when focus may reside in  /// an embedded child WebView, so Taurino tracks the combined state   /// explicitly.
    #[cfg(windows)]
    pub focused_webview: Arc<Mutex<FocusState>>,
    /// Indicates whether child WebViews are currently attached to the window.  /// Atomic storage permits inexpensive reads from code paths that only need   /// the current structural state without locking the complete WebView  /// manager.
    pub has_children: AtomicBool,
    /// Locally owned reference to the underlying Tao window.  /// `None` means that the native handle has already been taken or released    /// as part of the destruction sequence. Methods requiring the native window    /// must access it through [`Window::tao`] or one of the optional accessors.
    pub(crate) inner: Option<Arc<TaoWindow>>,
}
impl Window {
    /// Creates a window wrapper and initializes its window-scoped platform resources.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        window: TaoWindow,
        window_id: WindowId,
        window_label: String,
        /* menu: Option<WindowMenu>, */
        #[cfg(windows)] background_color: Option<tao::window::RGBA>,
        #[cfg(windows)] is_window_transparent: bool,
        #[cfg(windows)] focused_webview: Arc<Mutex<FocusState>>,
        has_children: bool,
        webviews_manager: WebViewManager,
    ) -> Arc<Self> {
        let inner = Arc::new(window);

        #[cfg(windows)]
        let surface = arc_mut(if is_window_transparent {
            if let Ok(context) = softbuffer::Context::new(inner.clone()) {
                if let Ok(mut surface) = softbuffer::Surface::new(&context, inner.clone()) {
                    inner.draw_surface(&mut surface, background_color);

                    Some(surface)
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        });
        #[allow(clippy::arc_with_non_send_sync)]
        Arc::new(Self {
            webviews_manager,
            /* menu: arc_mut(menu), */
            id: window_id,
            label: window_label,
            #[cfg(windows)]
            background_color: arc_mut(background_color),
            #[cfg(windows)]
            is_window_transparent,
            #[cfg(windows)]
            surface,
            #[cfg(windows)]
            focused_webview,
            has_children: AtomicBool::new(has_children),
            inner: Some(inner),
        })
    }
}

impl Window {
    /// Returns the window label.
    /// Returns the application-visible label of this window.
    pub fn label(&self) -> &str { &self.label }
    /// Returns the internal u32 window ID (not the Tao WindowId).
    /// Returns the Taurino window identifier assigned to this window.
    pub fn id(&self) -> WindowId { self.id }
    /// Returns the native window or an error after it has been taken.
    /// Returns the underlying Tao window or an error if its native handle is no longer available.
    pub fn tao(&self) -> Result<&TaoWindow> {
        self.inner.as_deref().ok_or_else(|| anyhow!("Window '{}' is not available", self.label))
    }
    /// Returns the underlying Tao window.
    /// Returns `None` if the native window has already been taken or destroyed.
    /// Returns the underlying Tao window reference when the native handle is still available.
    pub fn inner(&self) -> Option<&Arc<TaoWindow>> { self.inner.as_ref() }
    /// Returns a cloned reference to the underlying Tao window.
    /// Returns a cloned shared reference to the underlying Tao window when available.
    pub fn inner_arc(&self) -> Option<Arc<TaoWindow>> { self.inner.clone() }

    /// Takes only the native handle; WebViews, surface and menu remain intact.
    /// For the complete close sequence use the manager and destroy().
    /// Removes and returns the native Tao window handle without destroying other window-scoped resources.
    pub fn take_inner_window(&mut self) -> Option<Arc<TaoWindow>> { self.inner.take() }

    #[cfg(windows)]
    /// Returns the stored Windows background color.
    pub fn background_color(&self) -> Option<tao::window::RGBA> { *self.background_color.lock().unwrap() }

    #[cfg(windows)]
    /// Returns whether this Windows window is configured for transparent rendering.
    pub fn is_transparent(&self) -> bool { self.is_window_transparent }

    #[cfg(windows)]
    /// Returns the shared Windows focus state used by the window and its child WebViews.
    pub fn focused_webview(&self) -> &Arc<Mutex<FocusState>> { &self.focused_webview }
}

impl HasDisplayHandle for Window {
    /// Returns the raw display handle of the underlying native window.
    fn display_handle(&self) -> std::result::Result<DisplayHandle<'_>, HandleError> {
        self.inner.as_ref().ok_or(HandleError::Unavailable)?.display_handle()
    }
}

impl HasWindowHandle for Window {
    /// Returns the raw window handle of the underlying native window.
    fn window_handle(&self) -> std::result::Result<WindowHandle<'_>, HandleError> {
        self.inner.as_ref().ok_or(HandleError::Unavailable)?.window_handle()
    }
}

impl Window {
    // -------------------------------------------------------------------------

    // Getters

    // -------------------------------------------------------------------------
    /// Returns the scale factor that can be used to map logical pixels to physical pixels, and vice versa.
    /// Returns the native scale factor used to convert between logical and physical pixels.
    pub fn scale_factor(&self) -> Result<f64> {
        let inner = self.tao()?;

        Ok(inner.scale_factor())
    }
    /// Returns the position of the top-left hand corner of the window's client area relative to the top-left hand corner of the desktop.
    /// Returns the desktop-relative position of the window's client area.
    pub fn inner_position(&self) -> Result<PhysicalPosition<i32>> {
        let inner = self.tao()?;

        inner.inner_position().map_err(Into::into)
    }
    /// Returns the position of the top-left hand corner of the window relative to the top-left hand corner of the desktop.
    /// Returns the desktop-relative position of the complete native window.
    pub fn outer_position(&self) -> Result<PhysicalPosition<i32>> {
        let inner = self.tao()?;

        inner.outer_position().map_err(Into::into)
    }
    /// Returns the physical size of the window's client area.
    /// Returns the effective physical client-area size, accounting for child WebViews when required.
    pub fn inner_size(&self) -> Result<PhysicalSize<u32>> {
        let has_children = self.has_children();
        Ok(inner_size(self.tao()?, &self.webviews_manager.webviews(), has_children)?)
    }
    /// Returns the physical size of the entire window.
    /// Returns the physical size of the complete native window.
    pub fn outer_size(&self) -> Result<PhysicalSize<u32>> {
        let inner = self.tao()?;

        Ok(inner.outer_size())
    }
    /// Gets the window's current fullscreen state.
    /// Returns whether the window is currently fullscreen.
    pub fn is_fullscreen(&self) -> Result<bool> {
        let inner = self.tao()?;

        Ok(inner.fullscreen().is_some())
    }
    /// Gets the window's current minimized state.
    /// Returns whether the window is currently minimized.
    pub fn is_minimized(&self) -> Result<bool> {
        let inner = self.tao()?;

        Ok(inner.is_minimized())
    }
    /// Gets the window's current maximized state.
    /// Returns whether the window is currently maximized.
    pub fn is_maximized(&self) -> Result<bool> {
        let inner = self.tao()?;

        Ok(inner.is_maximized())
    }
    /// Returns whether the window or, on Windows, one of its child WebViews currently owns focus.
    pub fn is_focused(&self) -> Result<bool> {
        let inner = self.tao()?;

        #[cfg(windows)]
        if self.has_children() {
            let state = lock_state(&self.focused_webview, "focused_webview")?;

            return Ok(matches!(&*state, FocusState::WindowFocused | FocusState::WebviewFocused { .. }));
        }

        Ok(inner.is_focused())
    }
    /// Returns whether this window currently has child webviews.
    /// Returns whether the window currently contains child WebViews.
    pub fn has_children(&self) -> bool { self.has_children.load(Ordering::Acquire) }
    /// Sets whether this window has child webviews.
    /// Updates the atomic flag indicating whether the window contains child WebViews.
    pub fn set_has_child_webviews(&self, has_children: bool) {
        self.has_children.store(has_children, Ordering::Release);
    }
    /// Gets the window's current decoration state.
    /// Returns whether native window decorations are enabled.
    pub fn is_decorated(&self) -> Result<bool> {
        let inner = self.tao()?;

        Ok(inner.is_decorated())
    }
    /// Gets the window's current resizable state.
    /// Returns whether the window is currently resizable.
    pub fn is_resizable(&self) -> Result<bool> {
        let inner = self.tao()?;

        Ok(inner.is_resizable())
    }
    /// Gets the window's native maximize button state.
    /// Returns whether the native maximize action is enabled.
    pub fn is_maximizable(&self) -> Result<bool> {
        let inner = self.tao()?;

        Ok(inner.is_maximizable())
    }
    /// Gets the window's native minimize button state.
    /// Returns whether the native minimize action is enabled.
    pub fn is_minimizable(&self) -> Result<bool> {
        let inner = self.tao()?;

        Ok(inner.is_minimizable())
    }
    /// Gets the window's native close button state.
    /// Returns whether the native close action is enabled.
    pub fn is_closable(&self) -> Result<bool> {
        let inner = self.tao()?;

        Ok(inner.is_closable())
    }
    /// Gets the window's current visibility state.
    /// Returns whether the window is currently visible.
    pub fn is_visible(&self) -> Result<bool> {
        let inner = self.tao()?;

        Ok(inner.is_visible())
    }
    /// Whether the window is enabled or disabled.
    /// Returns whether the window currently accepts native user interaction.
    pub fn is_enabled(&self) -> Result<bool> {
        let inner = self.tao()?;

        Ok(inner.is_enabled())
    }
    /// Gets the window alwaysOnTop flag state.
    /// Returns whether the window is configured to stay above normal windows.
    pub fn is_always_on_top(&self) -> Result<bool> {
        let inner = self.tao()?;

        Ok(inner.is_always_on_top())
    }
    /// Gets the window's current title.
    /// Returns the current native window title.
    pub fn title(&self) -> Result<String> {
        let inner = self.tao()?;

        Ok(inner.title())
    }
    /// Returns the monitor on which the window currently resides.
    /// Returns the monitor that currently contains the window, if one can be determined.
    pub fn current_monitor(&self) -> Result<Option<Monitor>> {
        let inner = self.tao()?;

        Ok(inner.current_monitor().map(|m| MonitorHandleWrapper(m).into()))
    }
    /// Returns the primary monitor of the system.
    /// Returns the system's primary monitor, if one is available.
    pub fn primary_monitor(&self) -> Result<Option<Monitor>> {
        let inner = self.tao()?;

        Ok(inner.primary_monitor().map(|m| MonitorHandleWrapper(m).into()))
    }
    /// Returns the monitor that contains the given point.
    /// Returns the monitor containing the supplied desktop coordinate, if any.
    pub fn monitor_from_point(&self, x: f64, y: f64) -> Result<Option<Monitor>> {
        let inner = self.tao()?;

        Ok(inner.monitor_from_point(x, y).map(|m| MonitorHandleWrapper(m).into()))
    }
    /// Returns the list of all the monitors available on the system.
    /// Returns all monitors currently reported by the windowing system.
    pub fn available_monitors(&self) -> Result<Vec<Monitor>> {
        let inner = self.tao()?;

        Ok(inner.available_monitors().map(|m| MonitorHandleWrapper(m).into()).collect())
    }
    /// Returns the `ApplicationWindow` from gtk crate that is used by this window.

    #[cfg(any(
        target_os = "linux",
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd"
    ))]
    /// Returns the underlying GTK application window used by this Tao window.
    pub fn gtk_window(&self) -> Result<gtk::ApplicationWindow> {
        let inner = self.tao()?;

        Ok(inner.gtk_window().clone())
    }
    /// Returns the vertical [`gtk::Box`] that is added by default as the sole child of this window.

    #[cfg(any(
        target_os = "linux",
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd"
    ))]
    /// Returns the default GTK vertical box owned by this window.
    pub fn default_vbox(&self) -> Result<gtk::Box> {
        self.tao()?.default_vbox().cloned().ok_or_else(|| anyhow!("Window '{}' has no default GTK vbox", self.label))
    }
    /// Returns the name of the Android activity associated with this window.

    #[cfg(target_os = "android")]
    /// Returns the Android activity name associated with this window.
    pub fn activity_name(&self) -> Result<String> {
        let inner = self.tao()?;

        Ok(inner.activity_name().to_string())
    }
    /// Returns the identifier of the UIScene tied to this UIWindow.

    #[cfg(target_os = "ios")]
    /// Returns the iOS scene identifier associated with this window.
    pub fn scene_identifier(&self) -> Result<String> {
        let inner = self.tao()?;

        Ok(inner.scene_identifier().to_string())
    }
    /// Returns the current window theme.
    /// Returns the current native window theme mapped to Taurino's theme type.
    pub fn theme(&self) -> Result<Theme> { Ok(map_theme_from_tao(self.tao()?.theme())) }

    // -------------------------------------------------------------------------

    // Setters

    // -------------------------------------------------------------------------
    /// Centers the window.
    /// Centers the window using the platform window manager.
    pub fn center(&self) -> Result<()> {
        let inner = self.tao()?;

        inner.center();

        Ok(())
    }
    /// Requests user attention to the window.
    /// Requests platform-specific user attention for this window.
    pub fn request_user_attention(&self, request_type: Option<UserAttentionType>) -> Result<()> {
        let inner = self.tao()?;

        inner.request_user_attention(request_type.map(|r| UserAttentionTypeWrapper::from(r).0));

        Ok(())
    }
    /// Updates the window resizable flag.
    /// Enables or disables resizing and updates undecorated resize handling on Windows.
    pub fn set_resizable(&self, resizable: bool) -> Result<()> {
        let inner = self.tao()?;

        inner.set_resizable(resizable);

        #[cfg(windows)]
        {
            if !resizable {
                undecorated_resizing::detach_resize_handler(inner.hwnd());
            } else if !inner.is_decorated() {
                undecorated_resizing::attach_resize_handler(inner.hwnd(), inner.has_undecorated_shadow());
            }
        }

        Ok(())
    }
    /// Enable or disable the window.
    /// Enables or disables native user interaction with the window.
    pub fn set_enabled(&self, enabled: bool) -> Result<()> {
        let inner = self.tao()?;

        inner.set_enabled(enabled);

        Ok(())
    }
    /// Updates the window's native maximize button state.
    /// Enables or disables the native maximize action.
    pub fn set_maximizable(&self, maximizable: bool) -> Result<()> {
        let inner = self.tao()?;

        inner.set_maximizable(maximizable);

        Ok(())
    }
    /// Updates the window's native minimize button state.
    /// Enables or disables the native minimize action.
    pub fn set_minimizable(&self, minimizable: bool) -> Result<()> {
        let inner = self.tao()?;

        inner.set_minimizable(minimizable);

        Ok(())
    }
    /// Updates the window's native close button state.
    /// Enables or disables the native close action.
    pub fn set_closable(&self, closable: bool) -> Result<()> {
        let inner = self.tao()?;

        inner.set_closable(closable);

        Ok(())
    }
    /// Updates the window title.
    /// Updates the native window title.
    pub fn set_title<S: Into<String>>(&self, title: S) -> Result<()> {
        let inner = self.tao()?;

        inner.set_title(&title.into());

        Ok(())
    }
    /// Maximizes the window.
    /// Maximizes the window.
    pub fn maximize(&self) -> Result<()> {
        let inner = self.tao()?;

        inner.set_maximized(true);

        Ok(())
    }
    /// Unmaximizes the window.
    /// Restores the window from the maximized state.
    pub fn unmaximize(&self) -> Result<()> {
        let inner = self.tao()?;

        inner.set_maximized(false);

        Ok(())
    }
    /// Minimizes the window.
    /// Minimizes the window.
    pub fn minimize(&self) -> Result<()> {
        let inner = self.tao()?;

        inner.set_minimized(true);

        Ok(())
    }
    /// Unminimizes the window.
    /// Restores the window from the minimized state.
    pub fn unminimize(&self) -> Result<()> {
        let inner = self.tao()?;

        inner.set_minimized(false);

        Ok(())
    }
    /// Shows the window.
    /// Makes the window visible.
    pub fn show(&self) -> Result<()> {
        let inner = self.tao()?;

        inner.set_visible(true);

        Ok(())
    }
    /// Hides the window.
    /// Hides the window without destroying it.
    pub fn hide(&self) -> Result<()> {
        let inner = self.tao()?;

        inner.set_visible(false);

        Ok(())
    }
    /// Releases the local WebView handles, the shared surface and the local
    /// native window handle. Does not send a close event and does not check for a veto.
    /// Beforehand, detach the native menu binding in the manager and clean up the
    /// window/WebView registrations. Do not destroy the app-wide menu wholesale.
    /// External WebView/Tao clones are not invalidated by this method;
    /// in particular, immediate destruction of the OS window is not guaranteed.
    /// Do not call while the calling thread holds the surface/context lock.
    /// Releases this wrapper's WebViews and platform resources before dropping its native window handle.
    pub fn destroy(&mut self) -> Result<()> {
        // On a lock error, no resource has yet been removed from this wrapper.

        #[cfg(windows)]
        let surface = {
            let mut guard = lock_state(&self.surface, "surface")?;

            guard.take()
        };

        // Release dependent resources while self.inner still exists.

        self.webviews_manager.clear();

        self.set_has_child_webviews(false);

        #[cfg(windows)]
        drop(surface);

        drop(self.inner.take());

        Ok(())
    }
    /// Updates the decorations flag.
    /// Enables or disables native decorations and updates undecorated resize handling on Windows.
    pub fn set_decorations(&self, decorations: bool) -> Result<()> {
        let inner = self.tao()?;

        inner.set_decorations(decorations);

        #[cfg(windows)]
        {
            if decorations {
                undecorated_resizing::detach_resize_handler(inner.hwnd());
            } else if inner.is_resizable() {
                undecorated_resizing::attach_resize_handler(inner.hwnd(), inner.has_undecorated_shadow());
            }
        }

        Ok(())
    }
    /// Toggles the native window shadow on Windows/macOS.
    /// Enables or disables the native window shadow on supported platforms.
    pub fn set_shadow(&self, enable: bool) -> Result<()> {
        let inner = self.tao()?;

        #[cfg(windows)]
        {
            inner.set_undecorated_shadow(enable);

            return Ok(());
        }

        #[cfg(target_os = "macos")]
        {
            inner.set_has_shadow(enable);

            return Ok(());
        }

        #[cfg(not(any(windows, target_os = "macos")))]
        {
            let _ = (inner, enable);

            Err(anyhow!("set_shadow is not supported on this platform"))
        }
    }
    /// Updates the window alwaysOnBottom flag.
    /// Configures whether the window stays below normal windows.
    pub fn set_always_on_bottom(&self, always_on_bottom: bool) -> Result<()> {
        let inner = self.tao()?;

        inner.set_always_on_bottom(always_on_bottom);

        Ok(())
    }
    /// Updates the window alwaysOnTop flag.
    /// Configures whether the window stays above normal windows.
    pub fn set_always_on_top(&self, always_on_top: bool) -> Result<()> {
        let inner = self.tao()?;

        inner.set_always_on_top(always_on_top);

        Ok(())
    }
    /// Updates the window visibleOnAllWorkspaces flag.
    /// Configures whether the window is visible on all workspaces.
    pub fn set_visible_on_all_workspaces(&self, visible_on_all_workspaces: bool) -> Result<()> {
        let inner = self.tao()?;

        inner.set_visible_on_all_workspaces(visible_on_all_workspaces);

        Ok(())
    }
    /// Changes the background color. On Windows a redraw is subsequently
    /// requested; the actual surface drawing occurs in the event handler.
    /// Updates the native background color and requests a redraw on Windows.
    pub fn set_background_color(&self, color: Option<Color>) -> Result<()> {
        let inner = self.tao()?;

        let rgba: Option<tao::window::RGBA> = color.map(Into::into);

        #[cfg(windows)]
        {
            *lock_state(&self.background_color, "background_color")? = rgba;
        }

        // No color lock is held during the native call.

        inner.set_background_color(rgba);

        #[cfg(windows)]
        inner.request_redraw();

        Ok(())
    }
    /// Prevents the window contents from being captured by other apps.
    /// Enables or disables protection against capture of the window contents by other applications.
    pub fn set_content_protected(&self, protected: bool) -> Result<()> {
        self.tao()?.set_content_protection(protected);

        Ok(())
    }
    /// Resizes the window.
    /// Sets the window's inner size.
    pub fn set_size(&self, size: Size) -> Result<()> {
        self.tao()?.set_inner_size(size);

        Ok(())
    }
    /// Updates the window min inner size.
    /// Sets or clears the minimum inner size constraint.
    pub fn set_min_size(&self, size: Option<Size>) -> Result<()> {
        let inner = self.tao()?;

        inner.set_min_inner_size(size);

        Ok(())
    }
    /// Updates the window max inner size.
    /// Sets or clears the maximum inner size constraint.
    pub fn set_max_size(&self, size: Option<Size>) -> Result<()> {
        let inner = self.tao()?;

        inner.set_max_inner_size(size);

        Ok(())
    }
    /// Sets this window's minimum inner width.
    /// Applies the configured minimum and maximum inner size constraints.
    pub fn set_size_constraints(&self, constraints: WindowSizeConstraints) -> Result<()> {
        self.tao()?.set_inner_size_constraints(tao::window::WindowSizeConstraints {
            min_width: constraints.min_width,

            min_height: constraints.min_height,

            max_width: constraints.max_width,

            max_height: constraints.max_height,
        });

        Ok(())
    }
    /// Updates the window position.
    /// Moves the window to the supplied outer desktop position.
    pub fn set_position(&self, position: Position) -> Result<()> {
        let inner = self.tao()?;

        inner.set_outer_position(position);

        Ok(())
    }
    /// Updates the window fullscreen state.
    /// Enables or disables borderless fullscreen mode.
    pub fn set_fullscreen(&self, fullscreen: bool) -> Result<()> {
        let inner = self.tao()?;

        if fullscreen {
            inner.set_fullscreen(Some(tao::window::Fullscreen::Borderless(None)));
        } else {
            inner.set_fullscreen(None);
        }

        Ok(())
    }
    /// Sets the window as fullscreen on the monitor that contains the given physical position.
    /// Enables borderless fullscreen on the monitor containing the supplied physical position.
    pub fn set_fullscreen_on_monitor(&self, position: PhysicalPosition<f64>) -> Result<()> {
        let inner = self.tao()?;

        let monitor = inner
            .monitor_from_point(position.x, position.y)
            .ok_or_else(|| anyhow!("No monitor contains position ({}, {})", position.x, position.y))?;

        inner.set_fullscreen(Some(tao::window::Fullscreen::Borderless(Some(monitor))));

        Ok(())
    }

    #[cfg(target_os = "macos")]
    /// Enables or disables macOS simple fullscreen mode.
    pub fn set_simple_fullscreen(&self, enable: bool) -> Result<()> {
        let inner = self.tao()?;

        if inner.simple_fullscreen() == enable {
            return Ok(());
        }

        if !inner.set_simple_fullscreen(enable) {
            return Err(anyhow!("Could not change simple fullscreen for '{}'", self.label));
        }

        Ok(())
    }
    /// Bring the window to front and focus.
    /// Brings the window to the foreground and requests keyboard focus.
    pub fn set_focus(&self) -> Result<()> {
        let inner = self.tao()?;

        inner.set_focus();

        Ok(())
    }
    /// Sets whether the window can be focused.
    /// Configures whether the window is allowed to receive focus.
    pub fn set_focusable(&self, focusable: bool) -> Result<()> {
        let inner = self.tao()?;

        inner.set_focusable(focusable);

        Ok(())
    }
    /// Updates the window icon.
    /// Converts and applies the supplied image as the native window icon.
    pub fn set_icon(&self, icon: Icon<'_>) -> Result<()> {
        let tao_icon = to_tao_icon(icon)?;

        self.tao()?.set_window_icon(Some(tao_icon));

        Ok(())
    }
    /// Whether to hide the window icon from the taskbar or not.
    /// Configures whether the window is omitted from the taskbar on supported platforms.
    pub fn set_skip_taskbar(&self, skip: bool) -> Result<()> {
        let inner = self.tao()?;

        #[cfg(any(
            windows,
            target_os = "linux",
            target_os = "dragonfly",
            target_os = "freebsd",
            target_os = "netbsd",
            target_os = "openbsd"
        ))]
        {
            inner.set_skip_taskbar(skip)?;

            return Ok(());
        }

        #[cfg(not(any(
            windows,
            target_os = "linux",
            target_os = "dragonfly",
            target_os = "freebsd",
            target_os = "netbsd",
            target_os = "openbsd"
        )))]
        {
            let _ = (inner, skip);

            Err(anyhow!("set_skip_taskbar is not supported on this platform"))
        }
    }
    /// Grabs the cursor, preventing it from leaving the window.
    /// Grabs or releases the cursor within the window.
    pub fn set_cursor_grab(&self, grab: bool) -> Result<()> {
        self.tao()?.set_cursor_grab(grab)?;

        Ok(())
    }
    /// Modifies the cursor's visibility.
    /// Shows or hides the cursor while it is over the window.
    pub fn set_cursor_visible(&self, visible: bool) -> Result<()> {
        let inner = self.tao()?;

        inner.set_cursor_visible(visible);

        Ok(())
    }
    /// Modifies the cursor icon of the window.
    /// Changes the native cursor icon used by the window.
    pub fn set_cursor_icon(&self, icon: CursorIcon) -> Result<()> {
        let inner = self.tao()?;

        inner.set_cursor_icon(CursorIconWrapper::from(icon).0);

        Ok(())
    }
    /// Changes the position of the cursor in window coordinates.
    /// Moves the cursor to the supplied position in window coordinates.
    pub fn set_cursor_position<Pos: Into<Position>>(&self, position: Pos) -> Result<()> {
        let inner = self.tao()?;

        Ok(inner.set_cursor_position(position)?)
    }
    /// Ignores the window cursor events.
    /// Configures whether the window ignores cursor input events.
    pub fn set_ignore_cursor_events(&self, ignore: bool) -> Result<()> {
        let inner = self.tao()?;

        Ok(inner.set_ignore_cursor_events(ignore)?)
    }
    /// Starts dragging the window.
    /// Starts a native interactive window drag operation.
    pub fn start_dragging(&self) -> Result<()> {
        let inner = self.tao()?;

        Ok(inner.drag_window()?)
    }
    /// Starts resize-dragging the window.
    /// Starts a native interactive resize operation in the requested direction.
    pub fn start_resize_dragging(&self, direction: ResizeDirection) -> Result<()> {
        let result = self.tao()?.drag_resize_window(match direction {
            ResizeDirection::East => tao::window::ResizeDirection::East,

            ResizeDirection::North => tao::window::ResizeDirection::North,

            ResizeDirection::NorthEast => tao::window::ResizeDirection::NorthEast,

            ResizeDirection::NorthWest => tao::window::ResizeDirection::NorthWest,

            ResizeDirection::South => tao::window::ResizeDirection::South,

            ResizeDirection::SouthEast => tao::window::ResizeDirection::SouthEast,

            ResizeDirection::SouthWest => tao::window::ResizeDirection::SouthWest,

            ResizeDirection::West => tao::window::ResizeDirection::West,
        });

        Ok(result?)
    }

    #[cfg(any(
        target_os = "linux",
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd"
    ))]
    /// Returns the underlying GTK application window wrapped in `GtkWindow`.
    pub fn gtk_window_wrapped(&self) -> Result<GtkWindow> { self.gtk_window().map(GtkWindow) }

    #[cfg(any(
        target_os = "linux",
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd"
    ))]
    /// Returns the default GTK vertical box wrapped in `GtkBox`.
    pub fn default_vbox_wrapped(&self) -> Result<GtkBox> { self.default_vbox().map(GtkBox) }
    /// Sets a badge counter on the platforms supported here.
    /// On Windows, no silent success is pretended.
    /// Sets or clears the application badge count using the platform-specific implementation.
    pub fn set_badge_count(&self, count: Option<i64>, desktop_filename: Option<String>) -> Result<()> {
        let inner = self.tao()?;

        #[cfg(target_os = "ios")]
        {
            let _ = desktop_filename;

            inner.set_badge_count(count.map_or(0, |x| x.clamp(i32::MIN as i64, i32::MAX as i64) as i32));

            return Ok(());
        }

        #[cfg(target_os = "macos")]
        {
            let _ = desktop_filename;

            inner.set_badge_label(count.map(|x| x.to_string()));

            return Ok(());
        }

        #[cfg(any(
            target_os = "linux",
            target_os = "dragonfly",
            target_os = "freebsd",
            target_os = "netbsd",
            target_os = "openbsd"
        ))]
        {
            inner.set_badge_count(count, desktop_filename);

            return Ok(());
        }

        #[cfg(not(any(
            target_os = "ios",
            target_os = "macos",
            target_os = "linux",
            target_os = "dragonfly",
            target_os = "freebsd",
            target_os = "netbsd",
            target_os = "openbsd"
        )))]
        {
            let _ = (inner, count, desktop_filename);

            Err(anyhow!("set_badge_count is not supported on this platform"))
        }
    }
    /// Sets the badge count on the taskbar **macOS only**.

    #[cfg(target_os = "macos")]
    /// Sets or clears the macOS Dock badge label.
    pub fn set_badge_label(&self, label: Option<String>) -> Result<()> {
        self.tao()?.set_badge_label(label);

        Ok(())
    }
    /// On Windows, sets an overlay icon; None removes it.
    /// Sets or clears the Windows taskbar overlay icon.
    pub fn set_overlay_icon(&self, icon: Option<Icon<'_>>) -> Result<()> {
        let inner = self.tao()?;

        #[cfg(windows)]
        {
            let icon = icon.map(to_tao_icon).transpose()?;

            inner.set_overlay_icon(icon.as_ref());

            return Ok(());
        }

        #[cfg(not(windows))]
        {
            let _ = (inner, icon);

            Err(anyhow!("set_overlay_icon is only supported on Windows"))
        }
    }
    /// Sets the taskbar progress state.
    /// Updates the platform taskbar or dock progress indicator.
    pub fn set_progress_bar(&self, progress_state: ProgressBarState) -> Result<()> {
        self.tao()?.set_progress_bar(ProgressBarStateWrapper::from(progress_state).0);

        Ok(())
    }
    /// Sets the title bar style. Available on macOS only.

    // Mapping of the three variants intentionally taken from the template.

    // The enum definition and the intended layout semantics are not available.

    #[cfg(target_os = "macos")]
    /// Applies the selected macOS title-bar transparency and content layout behavior.
    pub fn set_title_bar_style(&self, style: TitleBarStyle) -> Result<()> {
        let inner = self.tao()?;

        match style {
            TitleBarStyle::Visible => {
                inner.set_titlebar_transparent(false);

                inner.set_fullsize_content_view(true);
            }

            TitleBarStyle::Transparent => {
                inner.set_titlebar_transparent(true);

                inner.set_fullsize_content_view(false);
            }

            TitleBarStyle::Overlay => {
                inner.set_titlebar_transparent(true);

                inner.set_fullsize_content_view(true);
            }

            #[allow(unreachable_patterns)]
            _ => return Err(anyhow!("Unsupported title bar style")),
        }

        Ok(())
    }
    /// Moves the window buttons under macOS.
    /// Moves the macOS traffic-light window controls to the supplied inset position.
    pub fn set_traffic_light_position(&self, position: Position) -> Result<()> {
        let inner = self.tao()?;

        #[cfg(target_os = "macos")]
        {
            inner.set_traffic_light_inset(position);

            return Ok(());
        }

        #[cfg(not(target_os = "macos"))]
        {
            let _ = (inner, position);

            Err(anyhow!("set_traffic_light_position is only supported on macOS"))
        }
    }

    #[cfg(windows)]
    /// Redraws only the matching transparent Windows window.
    /// Late events after taking the native handle are ignored.
    /// Redraws the matching transparent Windows surface when a valid redraw is required.
    pub fn redraw_requested(&self, window_id: tao::window::WindowId) -> Result<()> {
        let Some(inner) = self.inner.as_ref() else {
            return Ok(());
        };

        if inner.id() != window_id || !self.is_window_transparent {
            return Ok(());
        }

        let size = inner.inner_size();

        if size.width == 0 || size.height == 0 {
            return Ok(());
        }

        // Take a snapshot and release the color lock before the surface lock.

        let color = *lock_state(&self.background_color, "background_color")?;

        let mut guard = lock_state(&self.surface, "surface")?;

        if let Some(surface) = guard.as_mut() {
            // Existing project helper; its implementation is not available.

            // It must handle resizing/presentation and must not re-lock the same

            // surface mutex.

            inner.draw_surface(surface, color);
        }

        Ok(())
    }
    /// Sets the preferred native window theme or restores system theme selection.
    pub fn set_theme(&self, theme: Option<Theme>) -> Result<()> {
        let inner = self.tao()?;

        inner.set_theme(theme.map(map_from_theme));

        Ok(())
    }
}

#[cfg(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd"
))]

pub struct GtkWindow(pub gtk::ApplicationWindow);

#[cfg(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd"
))]

pub struct GtkBox(pub gtk::Box);
/*
// -----------------------------------------------------------------------------

// window/menu_ops.rs  (or append directly in builder.rs)

// -----------------------------------------------------------------------------

impl Window {
    // -------------------------------------------------------------------------

    // internal helper

    // -------------------------------------------------------------------------
    /// Clones the menu bound to this window, if present.
    /// Otherwise error – this keeps the API symmetric to `tao()`.

    #[inline]
    /// Returns the attached menu or an error when this window has no menu.
    pub fn menu_or_err(&self) -> Result<Menu> {
        self.try_menu()?.ok_or_else(|| anyhow!("Window '{}' has no menu attached", self.label))
    }
    /// Only `true` if a menu is present **and** it is not app-wide.
    /// Such menus may be manipulated by the window itself without disturbing the
    /// global MenuManager (macOS).
    /// Returns whether this window has a menu that is not application-wide.
    pub fn has_local_menu(&self) -> bool {
        self.menu.lock().map(|m| m.as_ref().is_some_and(|wm| !wm.is_app_wide)).unwrap_or(false)
    }

    // -------------------------------------------------------------------------

    // Search / traversal (delegates to Menu)

    // -------------------------------------------------------------------------
    /// Direct children of the menu.
    /// Returns the direct top-level items of this window's menu.
    pub fn menu_items(&self) -> Result<Vec<MenuItemKind>> {
        self.menu_or_err()?.items()
    }
    /// Direct match only among the top-level entries.
    /// Returns the top-level menu item with the supplied identifier, if present.
    pub fn get_menu_item_by_id(&self, id: &MenuId) -> Result<Option<MenuItemKind>> {
        self.menu_or_err()?.get(id)
    }
    /// Recursive search in the entire menu tree.
    /// Recursively searches the complete menu tree for the supplied identifier.
    pub fn find_menu_item(&self, id: &MenuId) -> Result<Option<MenuItemKind>> {
        self.menu_or_err()?.find(id)
    }
    /// All entries in the tree, depth-first.
    /// Returns all menu items in depth-first traversal order.
    pub fn all_menu_items(&self) -> Result<Vec<MenuItemKind>> {
        self.menu_or_err()?.all_items()
    }
    /// Visits every entry in the tree.
    /// Visits every item in this window's menu tree with the supplied callback.
    pub fn visit_menu_items<F>(&self, visitor: F) -> Result<()>
    where
        F: FnMut(&MenuItemKind) -> Result<()>,
    {
        self.menu_or_err()?.visit(visitor)
    }

    // -------------------------------------------------------------------------

    // Typed getters (delegates to Menu)

    // -------------------------------------------------------------------------
    /// Returns the regular menu item with the supplied identifier, if present.
    pub fn get_menu_item(&self, id: &MenuId) -> Result<Option<MenuItem>> {
        self.menu_or_err()?.get_menu_item(id)
    }
    /// Returns the submenu with the supplied identifier, if present.
    pub fn get_submenu(&self, id: &MenuId) -> Result<Option<Submenu>> {
        self.menu_or_err()?.get_submenu(id)
    }
    /// Returns the check-menu item with the supplied identifier, if present.
    pub fn get_check_menu_item(&self, id: &MenuId) -> Result<Option<CheckMenuItem>> {
        self.menu_or_err()?.get_check(id)
    }
    /// Returns the icon-menu item with the supplied identifier, if present.
    pub fn get_icon_menu_item(&self, id: &MenuId) -> Result<Option<IconMenuItem>> {
        self.menu_or_err()?.get_icon(id)
    }
    /// Returns the predefined menu item with the supplied identifier, if present.
    pub fn get_predefined_menu_item(&self, id: &MenuId) -> Result<Option<PredefinedMenuItem>> {
        self.menu_or_err()?.get_predefined(id)
    }

    /// Returns whether this window currently has a menu.
    /// Returns whether this window currently has an attached menu reference.
    pub fn has_menu(&self) -> bool {
        self.menu.lock().map(|menu| menu.is_some()).unwrap_or(false)
    }
    /// Returns whether the menu attached to this window is application-wide.
    /// This is mainly relevant on macOS.
    /// Returns whether this window references an application-wide menu.
    pub fn has_app_wide_menu(&self) -> bool {
        self.menu
            .lock()
            .map(|menu| menu.as_ref().is_some_and(|menu| menu.is_app_wide))
            .unwrap_or(false)
    }
    /// Returns a clone of the menu attached to this window.
    /// Returns a cloned menu handle, or `None` if no menu is attached or the menu lock is poisoned.
    pub fn menu(&self) -> Option<Menu> {
        self.menu.lock().ok().and_then(|menu| menu.as_ref().map(|menu| menu.menu.clone()))
    }
    /// Removes the menu reference from this window and returns it.
    /// For an application-wide macOS menu this only removes this window's
    /// reference. Actual application-wide ownership should remain with the
    /// MenuManager.
    /// Removes and returns this window's stored menu reference without changing global menu ownership.
    pub fn take_window_menu(&self) -> Option<WindowMenu> {
        self.menu.lock().ok().and_then(|mut menu| menu.take())
    }
    // -------------------------------------------------------------------------

    // Mutation (delegates to Menu)

    // -------------------------------------------------------------------------
    /// Appends one item to the end of this window's menu.
    pub fn append_menu_item(&self, item: &dyn IsMenuItem) -> Result<()> {
        self.menu_or_err()?.append(item)
    }
    /// Appends multiple items to the end of this window's menu.
    pub fn append_menu_items(&self, items: &[&dyn IsMenuItem]) -> Result<()> {
        self.menu_or_err()?.append_items(items)
    }
    /// Inserts one item at the beginning of this window's menu.
    pub fn prepend_menu_item(&self, item: &dyn IsMenuItem) -> Result<()> {
        self.menu_or_err()?.prepend(item)
    }
    /// Inserts a menu item at the supplied position.
    pub fn insert_menu_item(&self, item: &dyn IsMenuItem, position: usize) -> Result<()> {
        self.menu_or_err()?.insert(item, position)
    }
    /// Removes the supplied item from this window's menu.
    pub fn remove_menu_item(&self, item: &dyn IsMenuItem) -> Result<()> {
        self.menu_or_err()?.remove(item)
    }

    // -------------------------------------------------------------------------

    // Submenu helpers: directly access a submenu of the window

    // -------------------------------------------------------------------------
    /// Appends an item to a submenu of the window.
    /// Appends an item to the submenu identified by `submenu_id`.
    pub fn append_to_submenu(&self, submenu_id: &MenuId, item: &dyn IsMenuItem) -> Result<()> {
        let submenu = self
            .get_submenu(submenu_id)?
            .ok_or_else(|| anyhow!("Submenu {:?} not found", submenu_id))?;

        submenu.append(item)
    }
    /// Set iconized submenu (macOS/Windows).
    /// Sets or clears the native icon of the specified submenu.
    pub fn set_submenu_native_icon(
        &self,
        submenu_id: &MenuId,
        icon: Option<NativeIcon>,
    ) -> Result<()> {
        let submenu = self
            .get_submenu(submenu_id)?
            .ok_or_else(|| anyhow!("Submenu {:?} not found", submenu_id))?;

        submenu.set_native_icon(icon)
    }


    /// In contrast to menu(), a poisoned mutex is reported as an error.
    /// Returns a cloned menu handle and reports a poisoned menu lock as an error.
    pub fn try_menu(&self) -> Result<Option<Menu>> {
        let guard = lock_state(&self.menu, "menu")?;

        Ok(guard.as_ref().map(|menu| menu.menu.clone()))
    }
    /// Takes only the stored reference; does not detach the native menu binding.
    /// Removes and returns the stored menu reference while reporting lock poisoning as an error.
    pub fn try_take_window_menu(&self) -> Result<Option<WindowMenu>> {
        Ok(lock_state(&self.menu, "menu")?.take())
    }
} */

/// Converts Taurino RGBA icon data into a Tao native window icon.
fn to_tao_icon(icon: Icon<'_>) -> Result<tao::window::Icon> {
    tao::window::Icon::from_rgba(icon.rgba.into_owned(), icon.width, icon.height)
        .map_err(|error| anyhow!("failed to create Tao icon from RGBA data: {error}"))
}

/// Maps Tao's native window theme to Taurino's theme type.
pub fn map_theme_from_tao(theme: tao::window::Theme) -> Theme {
    match theme {
        tao::window::Theme::Light => Theme::Light,

        tao::window::Theme::Dark => Theme::Dark,

        #[allow(unreachable_patterns)]
        _ => Theme::Light,
    }
}

// -----------------------------------------------------------------------------

// Additional accessors without changing the native window mapping

// -----------------------------------------------------------------------------

impl Window {
    /// Tao ID, not the u32 ID from the registry or from WebView::window_id().
    /// Returns the native Tao identifier of this window.
    pub fn tao_window_id(&self) -> Result<tao::window::WindowId> { Ok(self.tao()?.id()) }
    /// Returns whether this wrapper still owns a native Tao window handle.
    pub fn is_tao_window_available(&self) -> bool { self.inner.is_some() }
    /// Returns all WebViews currently managed by this window.
    pub fn webviews(&self) -> &[WebView] { self.webviews_manager.webviews() }
    /// Returns the WebView with the supplied Taurino identifier, if present.
    pub fn webview(&self, id: WebViewId) -> Option<&WebView> { self.webviews_manager.get_by_id(id) }
    /// Returns the WebView with the supplied label, if present.
    pub fn webview_by_label(&self, label: &str) -> Option<&WebView> { self.webviews_manager.get_by_label(label) }
    /// Requests a redraw; does not draw synchronously at this call site.
    /// Queues a redraw request for the native window.
    pub fn request_redraw(&self) -> Result<()> {
        self.tao()?.request_redraw();

        Ok(())
    }

    #[cfg(windows)]
    /// Returns the stored Windows background color while reporting lock poisoning as an error.
    pub fn try_background_color(&self) -> Result<Option<tao::window::RGBA>> {
        Ok(*lock_state(&self.background_color, "background_color")?)
    }

    /*     pub fn js_evaluator_by_label<'a>(&'a self, label: &str) -> Result<JsEvaluator<'a>> {
        let webview = self
            .webviews_manager
            .get_by_label(label)
            .ok_or_else(|| anyhow!("WebView with label '{label}' not found"))?;

        Ok(JsEvaluator::new(webview))
    }
    pub fn js_evaluator_by_id<'a>(&'a self, id: WebViewId) -> Result<JsEvaluator<'a>> {
        let webview = self
            .webviews_manager
            .get_by_id(id)
            .ok_or_else(|| anyhow!("WebView with ID: '{:?}' not found", id))?;

        Ok(JsEvaluator::new(webview))
    }

    pub fn js_evaluator_by_id_mut<'a>(&'a mut self, id: WebViewId) -> Result<JsEvaluator<'a>> {
        let webview = self
            .webviews_manager
            .get_by_id_mut(id)
            .ok_or_else(|| anyhow!("WebView with ID: '{:?}' not found", id))?;

        Ok(JsEvaluator::new(webview))
    }

    pub fn js_evaluator_by_label_mut<'a>(&'a mut self, label: &str) -> Result<JsEvaluator<'a>> {
        let webview = self
            .webviews_manager
            .get_by_label_mut(label)
            .ok_or_else(|| anyhow!("WebView with label '{label}' not found"))?;

        Ok(JsEvaluator::new(webview))
    } */
}

#[cfg(target_os = "macos")]
pub fn inner_size(window: &TaoWindow, webviews: &[WebView], has_children: bool) -> Result<PhysicalSize<u32>> {
    use wry::WebViewExtMacOS;
    if !has_children {
        if let Some(webview) = webviews.first() {
            let _main_thread =
                MainThreadMarker::new().expect("native view measurement must run on the macOS main thread");
            let native_webview = webview.as_wry().webview();
            // SAFETY:
            // Wry returns its WKWebView subclass.
            // WKWebView is an NSView subclass on macOS.
            // Access occurs after verification on the main thread.
            let view = unsafe { Retained::cast_unchecked::<NSView>(native_webview) };
            let frame = view.frame();
            return Ok(LogicalSize::<f64>::new(frame.size.width, frame.size.height).to_physical(window.scale_factor()));
        }
    }
    let size = window.inner_size();
    // Explicit conversion avoids a dependency on whether engine_schema
    // and Tao re-export identical types.
    Ok(PhysicalSize::new(size.width, size.height))
}

#[cfg(not(target_os = "macos"))]
pub fn inner_size(window: &TaoWindow, _webviews: &[WebView], _has_children: bool) -> Result<PhysicalSize<u32>> {
    let size = window.inner_size();
    Ok(PhysicalSize::new(size.width, size.height))
}
