use {
    crate::{
        Color, PhysicalRect, Theme,
        webview::WebViewConfig,
        window::window_effects::{WindowEffect, WindowEffectState},
    },
    anyhow::Result,
    dpi::{LogicalPosition, PhysicalPosition, PhysicalSize, PixelUnit},
    serde::{Deserialize, Deserializer, Serialize, Serializer},
    serde_with::skip_serializing_none,
    std::{borrow::Cow, fmt::Display},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Icon<'a> {
    /// RGBA bytes of the icon.
    pub rgba: Cow<'a, [u8]>,
    /// Icon width.
    pub width: u32,
    /// Icon height.
    pub height: u32,
}

#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, Ord, PartialOrd, serde::Serialize, serde::Deserialize)]
pub struct WindowId(u32);

impl From<u32> for WindowId {
    fn from(value: u32) -> Self { Self(value) }
}

impl WindowId {
    pub fn get(self) -> u32 { self.0 }
}

/// Monitor descriptor.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Monitor {
    /// A human-readable name of the monitor.
    /// `None` if the monitor doesn't exist anymore.
    pub name: Option<String>,
    /// The monitor's resolution.
    pub size: PhysicalSize<u32>,
    /// The top-left corner position of the monitor relative to the larger full screen area.
    pub position: PhysicalPosition<i32>,
    /// The monitor's work_area.
    pub work_area: PhysicalRect<i32, u32>,
    /// Returns the scale factor that can be used to map logical pixels to physical pixels, and vice versa.
    pub scale_factor: f64,
}

impl Monitor {
    /// Returns a human-readable name of the monitor.
    /// Returns None if the monitor doesn't exist anymore.
    pub fn name(&self) -> Option<&String> { self.name.as_ref() }

    /// Returns the monitor's resolution.
    pub fn size(&self) -> &PhysicalSize<u32> { &self.size }

    /// Returns the top-left corner position of the monitor relative to the larger full screen area.
    pub fn position(&self) -> &PhysicalPosition<i32> { &self.position }

    /// Returns the monitor's work_area.
    pub fn work_area(&self) -> &PhysicalRect<i32, u32> { &self.work_area }

    /// Returns the scale factor that can be used to map logical pixels to physical pixels, and vice versa.
    pub fn scale_factor(&self) -> f64 { self.scale_factor }
}

/// Describes the appearance of the mouse cursor.
#[non_exhaustive]
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, Hash, Serialize)]
pub enum CursorIcon {
    /// The platform-dependent default cursor.
    #[default]
    Default,
    /// A simple crosshair.
    Crosshair,
    /// A hand (often used to indicate links in web browsers).
    Hand,
    /// Self explanatory.
    Arrow,
    /// Indicates something is to be moved.
    Move,
    /// Indicates text that may be selected or edited.
    Text,
    /// Program busy indicator.
    Wait,
    /// Help indicator (often rendered as a "?")
    Help,
    /// Progress indicator. Shows that processing is being done. But in contrast
    /// with "Wait" the user may still interact with the program. Often rendered
    /// as a spinning beach ball, or an arrow with a watch or hourglass.
    Progress,

    /// Cursor showing that something cannot be done.
    NotAllowed,
    ContextMenu,
    Cell,
    VerticalText,
    Alias,
    Copy,
    NoDrop,
    /// Indicates something can be grabbed.
    Grab,
    /// Indicates something is grabbed.
    Grabbing,
    AllScroll,
    ZoomIn,
    ZoomOut,

    /// Indicate that some edge is to be moved. For example, the 'SeResize' cursor
    /// is used when the movement starts from the south-east corner of the box.
    EResize,
    NResize,
    NeResize,
    NwResize,
    SResize,
    SeResize,
    SwResize,
    WResize,
    EwResize,
    NsResize,
    NeswResize,
    NwseResize,
    ColResize,
    RowResize,
}

impl<'de> Deserialize<'de> for CursorIcon {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(match s.to_lowercase().as_str() {
            "default" => CursorIcon::Default,
            "crosshair" => CursorIcon::Crosshair,
            "hand" => CursorIcon::Hand,
            "arrow" => CursorIcon::Arrow,
            "move" => CursorIcon::Move,
            "text" => CursorIcon::Text,
            "wait" => CursorIcon::Wait,
            "help" => CursorIcon::Help,
            "progress" => CursorIcon::Progress,
            "notallowed" => CursorIcon::NotAllowed,
            "contextmenu" => CursorIcon::ContextMenu,
            "cell" => CursorIcon::Cell,
            "verticaltext" => CursorIcon::VerticalText,
            "alias" => CursorIcon::Alias,
            "copy" => CursorIcon::Copy,
            "nodrop" => CursorIcon::NoDrop,
            "grab" => CursorIcon::Grab,
            "grabbing" => CursorIcon::Grabbing,
            "allscroll" => CursorIcon::AllScroll,
            "zoomin" => CursorIcon::ZoomIn,
            "zoomout" => CursorIcon::ZoomOut,
            "eresize" => CursorIcon::EResize,
            "nresize" => CursorIcon::NResize,
            "neresize" => CursorIcon::NeResize,
            "nwresize" => CursorIcon::NwResize,
            "sresize" => CursorIcon::SResize,
            "seresize" => CursorIcon::SeResize,
            "swresize" => CursorIcon::SwResize,
            "wresize" => CursorIcon::WResize,
            "ewresize" => CursorIcon::EwResize,
            "nsresize" => CursorIcon::NsResize,
            "neswresize" => CursorIcon::NeswResize,
            "nwseresize" => CursorIcon::NwseResize,
            "colresize" => CursorIcon::ColResize,
            "rowresize" => CursorIcon::RowResize,
            _ => CursorIcon::Default,
        })
    }
}

/// Window size constraints
#[derive(Clone, Copy, PartialEq, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowSizeConstraints {
    /// The minimum width a window can be, If this is `None`, the window will have no minimum width.
    ///
    /// The default is `None`.
    pub min_width: Option<PixelUnit>,
    /// The minimum height a window can be, If this is `None`, the window will have no minimum height.
    ///
    /// The default is `None`.
    pub min_height: Option<PixelUnit>,
    /// The maximum width a window can be, If this is `None`, the window will have no maximum width.
    ///
    /// The default is `None`.
    pub max_width: Option<PixelUnit>,
    /// The maximum height a window can be, If this is `None`, the window will have no maximum height.
    ///
    /// The default is `None`.
    pub max_height: Option<PixelUnit>,
}

/// Progress bar status.
#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ProgressBarStatus {
    /// Hide progress bar.
    None,
    /// Normal state.
    Normal,
    /// Indeterminate state. **Treated as Normal on Linux and macOS**
    Indeterminate,
    /// Paused state. **Treated as Normal on Linux**
    Paused,
    /// Error state. **Treated as Normal on Linux**
    Error,
}

/// Progress Bar State
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressBarState {
    /// The progress bar status.
    pub status: Option<ProgressBarStatus>,
    /// The progress bar progress. This can be a value ranging from `0` to `100`
    pub progress: Option<u64>,
    /// The `.desktop` filename with the Unity desktop window manager, for example `myapp.desktop` **Linux Only**
    pub desktop_filename: Option<String>,
}

/// Type of user attention requested on a window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum UserAttentionType {
    /// ## Platform-specific
    /// - **macOS:** Bounces the dock icon until the application is in focus.
    /// - **Windows:** Flashes both the window and the taskbar button until the application is in focus.
    Critical,
    /// ## Platform-specific
    /// - **macOS:** Bounces the dock icon once.
    /// - **Windows:** Flashes the taskbar button until the application is in focus.
    Informational,
}

/// Defines which device events (raw input from mice, keyboards and other HID devices that is not
/// bound to a specific window) the event loop should deliver to the application.
///
/// Listening to device events can be expensive, so the runtime filters them out by default
/// while the application has no focused window. See [`crate::Runtime::set_device_event_filter`].
///
/// ## Platform-specific
///
/// - **Linux / macOS / iOS / Android**: Unsupported, device events are always filtered out.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum DeviceEventFilter {
    /// Always filter out device events.
    Always,
    /// Filter out device events while the window is not focused.
    #[default]
    Unfocused,
    /// Report all device events regardless of window focus.
    Never,
}

/// Defines the orientation that a window resize will be performed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum ResizeDirection {
    East,
    North,
    NorthEast,
    NorthWest,
    South,
    SouthEast,
    SouthWest,
    West,
}

/// How the window title bar should be displayed on macOS.
#[derive(Debug, Clone, PartialEq, Eq, Copy, Default)]
#[non_exhaustive]
pub enum TitleBarStyle {
    /// A normal title bar.
    #[default]
    Visible,
    /// Makes the title bar transparent, so the window background color is shown instead.
    ///
    /// Useful if you don't need to have actual HTML under the title bar. This lets you avoid the caveats of using `TitleBarStyle::Overlay`. Will be more useful when Tauri lets you set a custom window background color.
    Transparent,
    /// Shows the title bar as a transparent overlay over the window's content.
    ///
    /// Keep in mind:
    /// - The height of the title bar is different on different OS versions, which can lead to window the controls and title not being where you don't expect.
    /// - You need to define a custom drag region to make your window draggable, however due to a limitation you can't drag the window when it's not in focus <https://github.com/tauri-apps/tauri/issues/4316>.
    /// - The color of the window title depends on the system theme.
    Overlay,
}

impl Serialize for TitleBarStyle {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.to_string().as_ref())
    }
}

impl<'de> Deserialize<'de> for TitleBarStyle {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(match s.to_lowercase().as_str() {
            "transparent" => Self::Transparent,
            "overlay" => Self::Overlay,
            _ => Self::Visible,
        })
    }
}

impl Display for TitleBarStyle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", match self {
            Self::Visible => "Visible",
            Self::Transparent => "Transparent",
            Self::Overlay => "Overlay",
        })
    }
}
/// Application's activation policy. Corresponds to NSApplicationActivationPolicy.
#[cfg(target_os = "macos")]
#[cfg_attr(docsrs, doc(cfg(target_os = "macos")))]
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum ActivationPolicy {
    /// Corresponds to NSApplicationActivationPolicyRegular.
    Regular,
    /// Corresponds to NSApplicationActivationPolicyAccessory.
    Accessory,
    /// Corresponds to NSApplicationActivationPolicyProhibited.
    Prohibited,
}

/// Enable prevent overflow with a margin
/// so that the window's size + this margin won't overflow the workarea
#[derive(Debug, PartialEq, Clone, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PreventOverflowMargin {
    /// Horizontal margin in physical pixels
    pub width: u32,
    /// Vertical margin in physical pixels
    pub height: u32,
}

/// Prevent overflow with a margin
#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PreventOverflowConfig {
    /// Enable prevent overflow or not
    Enable(bool),
    /// Enable prevent overflow with a margin
    /// so that the window's size + this margin won't overflow the workarea
    Margin(PreventOverflowMargin),
}

#[skip_serializing_none]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WindowConfig {
    /// The window identifier. It must be alphanumeric.
    #[serde(default = "default_window_label")]
    pub label: String,
    /// Whether Taurino should create this window at app startup or not.
    ///
    /// When this is set to `false` you must manually grab the config object via `app.config().app.windows`
    ///
    #[serde(default = "default_true")]
    pub create: bool,

    /// The horizontal position of the window's top left corner in logical pixels
    pub x: Option<f64>,
    /// The vertical position of the window's top left corner in logical pixels
    pub y: Option<f64>,

    pub webviews: Vec<WebViewConfig>,
    /// Whether or not the window starts centered or not.
    #[serde(default)]
    pub center: bool,
    /// The window width in logical pixels.
    #[serde(default = "default_width")]
    pub width: f64,
    /// The window height in logical pixels.
    #[serde(default = "default_height")]
    pub height: f64,
    /// The min window width in logical pixels.
    #[serde(alias = "min-width")]
    pub min_width: Option<f64>,
    /// The min window height in logical pixels.
    #[serde(alias = "min-height")]
    pub min_height: Option<f64>,
    /// The max window width in logical pixels.
    #[serde(alias = "max-width")]
    pub max_width: Option<f64>,
    /// The max window height in logical pixels.
    #[serde(alias = "max-height")]
    pub max_height: Option<f64>,

    /// Whether or not to prevent the window from overflowing the workarea
    ///
    /// ## Platform-specific
    ///
    /// - **iOS / Android:** Unsupported.
    #[serde(alias = "prevent-overflow")]
    pub prevent_overflow: Option<PreventOverflowConfig>,
    /// Whether the window is resizable or not. When resizable is set to false, native window's maximize button is automatically disabled.
    #[serde(default = "default_true")]
    pub resizable: bool,
    /// Whether the window's native maximize button is enabled or not.
    /// If resizable is set to false, this setting is ignored.
    ///
    /// ## Platform-specific
    ///
    /// - **macOS:** Disables the "zoom" button in the window titlebar, which is also used to enter fullscreen mode.
    /// - **Linux / iOS / Android:** Unsupported.
    #[serde(default = "default_true")]
    pub maximizable: bool,

    /// Whether the window's native minimize button is enabled or not.
    ///
    /// ## Platform-specific
    ///
    /// - **Linux / iOS / Android:** Unsupported.
    #[serde(default = "default_true")]
    pub minimizable: bool,
    /// Whether the window's native close button is enabled or not.
    ///
    /// ## Platform-specific
    ///
    /// - **Linux:** "GTK+ will do its best to convince the window manager not to show a close button.
    ///   Depending on the system, this function may not have any effect when called on a window that is already visible"
    /// - **iOS / Android:** Unsupported.
    #[serde(default = "default_true")]
    pub closable: bool,
    /// The window title.
    #[serde(default = "default_title")]
    pub title: String,
    /// Whether the window starts as fullscreen or not.
    #[serde(default)]
    pub fullscreen: bool,
    /// Whether the window will be initially focused or not.
    #[serde(default = "default_true")]
    pub focus: bool,
    /// Whether the window will be focusable or not.
    #[serde(default = "default_true")]
    pub focusable: bool,
    /// Whether the window is transparent or not.
    ///
    /// ## Platform-specific
    ///
    /// - **Windows**: Using `noRedirectionBitmap` can help avoid a white flash when creating a transparent window.
    #[serde(default)]
    pub transparent: bool,
    /// Whether the window is maximized or not.
    #[serde(default)]
    pub maximized: bool,
    /// Whether the window is visible or not.
    #[serde(default = "default_true")]
    pub visible: bool,
    /// Whether the window should have borders and bars.
    #[serde(default = "default_true")]
    pub decorations: bool,
    /// Whether the window should always be below other windows.
    #[serde(default, alias = "always-on-bottom")]
    pub always_on_bottom: bool,
    /// Whether the window should always be on top of other windows.
    #[serde(default, alias = "always-on-top")]
    pub always_on_top: bool,
    /// Whether the window should be visible on all workspaces or virtual desktops.
    ///
    /// ## Platform-specific
    ///
    /// - **Windows / iOS / Android:** Unsupported.
    #[serde(default, alias = "visible-on-all-workspaces")]
    pub visible_on_all_workspaces: bool,
    /// Prevents the window contents from being captured by other apps.
    #[serde(default, alias = "content-protected")]
    pub content_protected: bool,
    /// If `true`, hides the window icon from the taskbar on Windows and Linux.
    #[serde(default, alias = "skip-taskbar")]
    pub skip_taskbar: bool,
    /// The name of the window class created on Windows to create the window. **Windows only**.
    pub window_classname: Option<String>,
    /// This sets `WS_EX_NOREDIRECTIONBITMAP`.
    ///
    /// This can avoid the white flash that may appear before the webview content is rendered
    /// when using a transparent window. **Windows only**.
    #[serde(default, alias = "no-redirection-bitmap")]
    pub no_redirection_bitmap: bool,
    /// The initial window theme. Defaults to the system theme. Only implemented on Windows and macOS 10.14+.
    pub theme: Option<Theme>,
    /// The style of the macOS title bar.
    #[serde(default, alias = "title-bar-style")]
    pub title_bar_style: TitleBarStyle,
    /// The position of the window controls on macOS.
    ///
    /// Requires titleBarStyle: Overlay and decorations: true.
    #[serde(default, alias = "traffic-light-position")]
    pub traffic_light_position: Option<LogicalPosition<f64>>,
    /// If `true`, sets the window title to be hidden on macOS.
    #[serde(default, alias = "hidden-title")]
    pub hidden_title: bool,
    /// Whether clicking an inactive window also clicks through to the webview on macOS.
    #[serde(default, alias = "accept-first-mouse")]
    pub accept_first_mouse: bool,
    /// Defines the window [tabbing identifier] for macOS.
    ///
    /// Windows with matching tabbing identifiers will be grouped together.
    /// If the tabbing identifier is not set, automatic tabbing will be disabled.
    ///
    /// [tabbing identifier]: <https://developer.apple.com/documentation/appkit/nswindow/1644704-tabbingidentifier>
    #[serde(default, alias = "tabbing-identifier")]
    pub tabbing_identifier: Option<String>,

    /// Whether or not the window has shadow.
    ///
    /// ## Platform-specific
    ///
    /// - **Windows:**
    ///   - `false` has no effect on decorated window, shadow are always ON.
    ///   - `true` will make undecorated window have a 1px white border,
    /// and on Windows 11, it will have a rounded corners.
    /// - **Linux:** Unsupported.
    #[serde(default = "default_true")]
    pub shadow: bool,
    /// Window effects.
    ///
    /// Requires the window to be transparent.
    ///
    /// ## Platform-specific:
    ///
    /// - **Windows**: If using decorations or shadows, you may want to try this workaround <https://github.com/tauri-apps/tao/issues/72#issuecomment-975607891>
    /// - **Linux**: Unsupported
    #[serde(default, alias = "window-effects")]
    pub window_effects: Option<WindowEffectsConfig>,

    /// Sets the window associated with this label to be the parent of the window to be created.
    ///
    /// ## Platform-specific
    ///
    /// - **Windows**: This sets the passed parent as an owner window to the window to be created.
    ///   From [MSDN owned windows docs](https://docs.microsoft.com/en-us/windows/win32/winmsg/window-features#owned-windows):
    ///     - An owned window is always above its owner in the z-order.
    ///     - The system automatically destroys an owned window when its owner is destroyed.
    ///     - An owned window is hidden when its owner is minimized.
    /// - **Linux**: This makes the new window transient for parent, see <https://docs.gtk.org/gtk3/method.Window.set_transient_for.html>
    /// - **macOS**: This adds the window as a child of parent, see <https://developer.apple.com/documentation/appkit/nswindow/1419152-addchildwindow?language=objc>
    pub parent: Option<String>,

    /// Set the window and webview background color.
    ///
    /// ## Platform-specific:
    ///
    /// - **Windows**: alpha channel is ignored for the window layer.
    /// - **Windows**: On Windows 7, alpha channel is ignored for the webview layer.
    /// - **Windows**: On Windows 8 and newer, if alpha channel is not `0`, it will be ignored for the webview layer.
    #[serde(alias = "background-color")]
    pub background_color: Option<Color>,

    #[serde(default = "default_true")]
    pub enable_drag_drop: bool,
}

fn default_window_label() -> String { "main".to_string() }

fn default_width() -> f64 { 800. }

fn default_height() -> f64 { 600. }

fn default_title() -> String { "Taurino App".to_string() }

pub(crate) fn default_true() -> bool { true }

/// The window effects configuration object
#[skip_serializing_none]
#[derive(Debug, PartialEq, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WindowEffectsConfig {
    /// List of Window effects to apply to the Window.
    ///
    /// Generally, conflicting effects will apply the first one and ignore the rest but
    /// on macOS you can specify one Liquid Glass style and one Visual Effect material at the same time
    /// to make Tauri fallback to the latter on macOS 15 and below.
    pub effects: Vec<WindowEffect>,
    /// Window effect state **macOS Only**. Ignored for Liquid Glass Effects.
    pub state: Option<WindowEffectState>,
    /// Window effect corner radius **macOS Only**
    pub radius: Option<f64>,
    /// Window effect color.
    ///
    /// ## Platform-specific
    ///
    /// - **Windows**: Affects [`WindowEffect::Blur`] and [`WindowEffect::Acrylic`] only
    /// on Windows 10 v1903+. Doesn't have any effect on Windows 7 or Windows 11.
    /// - **macOS**: Only affects Liquid Glass effects.
    pub color: Option<Color>,
    /// Enables interactive glass behavior, which adds a visual response to user interactions.
    ///
    /// **macOS 27.0+**. Only affects Liquid Glass effects.
    #[serde(default)]
    pub interactive: bool,
}

#[allow(deprecated)]
mod window_effects {
    use super::*;

    #[derive(Debug, PartialEq, Eq, Clone, Copy, Deserialize, Serialize)]
    #[serde(rename_all = "camelCase")]
    #[non_exhaustive]
    /// Platform-specific window effects
    pub enum WindowEffect {
        /// A default material appropriate for the view's effectiveAppearance. **macOS 10.14-**
        #[deprecated(note = "You should instead choose an appropriate semantic material.")]
        AppearanceBased,
        /// **macOS 10.14-**
        #[deprecated(note = "Use a semantic material instead.")]
        Light,
        /// **macOS 10.14-**
        #[deprecated(note = "Use a semantic material instead.")]
        Dark,
        /// **macOS 10.14-**
        #[deprecated(note = "Use a semantic material instead.")]
        MediumLight,
        /// **macOS 10.14-**
        #[deprecated(note = "Use a semantic material instead.")]
        UltraDark,
        /// **macOS 10.10+**
        Titlebar,
        /// **macOS 10.10+**
        Selection,
        /// **macOS 10.11+**
        Menu,
        /// **macOS 10.11+**
        Popover,
        /// **macOS 10.11+**
        Sidebar,
        /// **macOS 10.14+**
        HeaderView,
        /// **macOS 10.14+**
        Sheet,
        /// **macOS 10.14+**
        WindowBackground,
        /// **macOS 10.14+**
        HudWindow,
        /// **macOS 10.14+**
        FullScreenUI,
        /// **macOS 10.14+**
        Tooltip,
        /// **macOS 10.14+**
        ContentBackground,
        /// **macOS 10.14+**
        UnderWindowBackground,
        /// **macOS 10.14+**
        UnderPageBackground,
        /// **macOS 26.0+**
        LiquidGlassRegular,
        /// **macOS 26.0+**
        LiquidGlassClear,
        /// Mica effect that matches the system dark preference **Windows 11 Only**
        Mica,
        /// Mica effect with dark mode but only if dark mode is enabled on the system **Windows 11 Only**
        MicaDark,
        /// Mica effect with light mode **Windows 11 Only**
        MicaLight,
        /// Tabbed effect that matches the system dark preference **Windows 11 Only**
        Tabbed,
        /// Tabbed effect with dark mode but only if dark mode is enabled on the system **Windows 11 Only**
        TabbedDark,
        /// Tabbed effect with light mode **Windows 11 Only**
        TabbedLight,
        /// **Windows 7/10/11(22H1) Only**
        ///
        /// ## Notes
        ///
        /// This effect has bad performance when resizing/dragging the window on Windows 11 build 22621.
        Blur,
        /// **Windows 10/11 Only**
        ///
        /// ## Notes
        ///
        /// This effect has bad performance when resizing/dragging the window on Windows 10 v1903+ and Windows 11 build 22000.
        Acrylic,
    }

    /// Window effect state **macOS only**
    ///
    /// <https://developer.apple.com/documentation/appkit/nsvisualeffectview/state>
    #[derive(Debug, PartialEq, Eq, Clone, Copy, Deserialize, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub enum WindowEffectState {
        /// Make window effect state follow the window's active state
        FollowsWindowActiveState,
        /// Make window effect state always active
        Active,
        /// Make window effect state always inactive
        Inactive,
    }
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            label: default_window_label(),
            create: true,

            x: None,
            y: None,

            webviews: Vec::new(),
            enable_drag_drop: false,
            center: false,
            width: default_width(),
            height: default_height(),

            min_width: None,
            min_height: None,
            max_width: None,
            max_height: None,

            prevent_overflow: None,

            resizable: true,
            maximizable: true,
            minimizable: true,
            closable: true,

            title: default_title(),

            fullscreen: false,
            focus: true,
            focusable: true,
            transparent: false,
            maximized: false,
            visible: true,
            decorations: true,

            always_on_bottom: false,
            always_on_top: false,
            visible_on_all_workspaces: false,
            content_protected: false,
            skip_taskbar: false,

            window_classname: None,
            no_redirection_bitmap: false,

            theme: None,
            title_bar_style: TitleBarStyle::default(),
            traffic_light_position: None,
            hidden_title: false,
            accept_first_mouse: false,
            tabbing_identifier: None,

            shadow: true,
            window_effects: None,

            parent: None,
            background_color: None,
        }
    }
}
