use {
    crate::{app::AppConfig, package::version_deserializer},
    dpi::{Position, Size},
    serde::{Deserialize, Deserializer, Serialize, Serializer},
    std::{
        fmt::{self, Display, Formatter},
        path::PathBuf,
        pin::Pin,
        str::FromStr,
    },
    url::Url,
};
use {
    crate::{
        /* app::AppConfig, */
        protocol::{InvokeRequest, InvokeResponse},
        starting_binary::current_exe,
    },
    anyhow::{Context, Result},
    directories::BaseDirs,
    std::{
        env,
        fmt::Debug,
        fs,
        ops::Deref,
        sync::{Arc, Mutex},
    },
    tao::{
        event::Event,
        event_loop::{
            ControlFlow, EventLoop, EventLoopBuilder, EventLoopClosed, EventLoopProxy, EventLoopWindowTarget,
        },
    },
};
pub mod package;
pub mod app;
pub mod mix;
pub mod paths;
pub mod utils;
pub mod starting_binary;
pub mod window;
pub mod webview;
pub mod dispatcher;
pub mod protocol;
pub type EngineLoop = EventLoop<EventLoopMessage>;
pub type EngineLoopBuilder = EventLoopBuilder<EventLoopMessage>;
pub type EngineLoopProxy = EventLoopProxy<EventLoopMessage>;
pub type EngineWindowTarget = EventLoopWindowTarget<EventLoopMessage>;
pub type EngineLoopClosed = EventLoopClosed<EventLoopMessage>;
pub type EngineEvent<'a> = Event<'a, EventLoopMessage>;

pub type EngineCallback = Pin<Box<dyn Fn(&EngineWindowTarget, &mut ControlFlow) -> Result<()> + Send>>;
pub struct EngineTask(EngineCallback);

impl Debug for EngineTask {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result { f.debug_struct("EngineTask").finish() }
}

impl EngineTask {
    pub fn new<F: Fn(&EngineWindowTarget, &mut ControlFlow) -> Result<()> + Send + 'static>(f: F) -> Self {
        Self(Box::pin(f))
    }
}

impl Deref for EngineTask {
    type Target = Pin<Box<dyn Fn(&EngineWindowTarget, &mut ControlFlow) -> Result<()> + Send>>;
    fn deref(&self) -> &Self::Target { &self.0 }
}

#[derive(Debug)]
pub enum IPCMessage {
    Send(Vec<u8>),
}

#[derive(Debug, Clone)]
pub enum IpcEvent {
    Message(Vec<u8>),
    Disconnected(String),
}
/// Internal messages delivered through the Tao user-event channel.
///
/// These messages provide a thread-safe mechanism for scheduling Taurino work on
/// the event-loop thread.
///
/// Code executing outside the GUI event loop should generally send one of these
/// messages instead of manipulating Tao state directly.
pub enum EventLoopMessage {
    /// Requests graceful termination of the entire application.
    ///
    /// The request is processed by [`MainThreadEventHandler`] on the event-loop
    /// thread.
    Shutdown,

    /// Executes a dynamically supplied Taurino event-loop operation.
    ///
    /// This variant allows Taurino subsystems to schedule arbitrary operations
    /// that require access to the Tao event-loop target and its
    /// [`ControlFlow`].
    EngineTask(EngineTask),
    /// Executes a dynamically supplied event-loop operation.
    ///
    /// This variant allows Taurino subsystems to schedule arbitrary operations
    /// that require to run on the Tao event-loop target thread
    Task(Box<dyn FnOnce() + Send>),

    EvalScript(String, String, String),
    JsInvoke {
        webview_label: String,
        request: InvokeRequest,
    },
    JsReply {
        webview_label: String,
        request: InvokeRequest,
        response: std::result::Result<InvokeResponse, String>,
    },
    IPC(IpcEvent),
}

// Create a filesystem-safe application name.
pub fn normalize_name(name: &str) -> String {
    let mut result = String::new();
    let mut previous_separator = false;
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            result.push(ch.to_ascii_lowercase());
            previous_separator = false;
        } else if !previous_separator && !result.is_empty() {
            result.push('-');
            previous_separator = true;
        }
    }
    let normalized = result.trim_end_matches('-');
    if normalized.is_empty() { "taurino".to_owned() } else { normalized.to_owned() }
}

pub type ArcMut<T> = Arc<Mutex<T>>;
pub fn arc_mut<T>(t: T) -> ArcMut<T> { Arc::new(Mutex::new(t)) }

/*

let data_directory: Option<PathBuf> =
    Some(PathBuf::from("profiles/default"));

let resolved = config.prepare_webview_data_directory(
    "main",
    data_directory,
)?;

*/

/// System theme.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Theme {
    /// Light theme.
    Light,
    /// Dark theme.
    Dark,
}

impl Serialize for Theme {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.to_string().as_ref())
    }
}

impl<'de> Deserialize<'de> for Theme {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(match s.to_lowercase().as_str() {
            "dark" => Self::Dark,
            _ => Self::Light,
        })
    }
}

impl Display for Theme {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", match self {
            Self::Light => "light",
            Self::Dark => "dark",
        })
    }
}
/// A rectangular region.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Rect {
    /// Rect position.
    pub position: dpi::Position,
    /// Rect size.
    pub size: dpi::Size,
}

impl Default for Rect {
    fn default() -> Self { Self { position: Position::Logical((0, 0).into()), size: Size::Logical((0, 0).into()) } }
}

/// A rectangular region in physical pixels.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct PhysicalRect<P: dpi::Pixel, S: dpi::Pixel> {
    /// Rect position.
    pub position: dpi::PhysicalPosition<P>,
    /// Rect size.
    pub size: dpi::PhysicalSize<S>,
}

impl<P: dpi::Pixel, S: dpi::Pixel> Default for PhysicalRect<P, S> {
    fn default() -> Self { Self { position: (0, 0).into(), size: (0, 0).into() } }
}

/// A rectangular region in logical pixels.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct LogicalRect<P: dpi::Pixel, S: dpi::Pixel> {
    /// Rect position.
    pub position: dpi::LogicalPosition<P>,
    /// Rect size.
    pub size: dpi::LogicalSize<S>,
}

impl<P: dpi::Pixel, S: dpi::Pixel> Default for LogicalRect<P, S> {
    fn default() -> Self { Self { position: (0, 0).into(), size: (0, 0).into() } }
}

// ============================================================================
// FrontendDist
// ============================================================================
//
// Enum describing how the frontend is delivered. Three variants:
//
//   Url(Url)          -> load the UI from a URL, no local files embedded
//   Directory(PathBuf)-> a directory of frontend files, embedded recursively,
//                        with `index.html` as the entry point
//   Files(Vec<PathBuf>)-> an explicit list of individual files to embed
//
// Serde attributes:
//
//   `untagged`
//     The config value is NOT tagged with the variant name. Serde tries each
//     variant in order and picks the first that matches. Examples:
//         "https://example.com"       -> Url
//         "./dist"                    -> Directory
//         ["./a.html", "./b.js"]      -> Files
//
//   `deny_unknown_fields`
//     Unknown keys cause an error. Mainly relevant for struct-like variants;
//     for these newtype variants it has limited impact.
//
// Derives:
//
//   Debug, PartialEq, Eq, Clone
//     Standard utility traits. `Eq` is valid here because `Url`, `PathBuf`
//     and `Vec<PathBuf>` all implement `Eq`.
//
//   Deserialize AND Serialize
//     Unlike the config structs above, this type is also serializable,
//     because it may be written back out (e.g. by tooling or for display).
//
//   #[non_exhaustive]
//     Outside this crate, any `match` on this enum must include a wildcard
//     arm. This lets us add new variants later without a breaking change.
//
/// Describes where the user interface is loaded from.
#[derive(Debug, PartialEq, Eq, Clone, Deserialize, Serialize)]
#[serde(untagged, deny_unknown_fields)]
#[non_exhaustive]
pub enum FrontendDist {
    /// URL of the user interface; no local files are embedded for it.
    Url(Url),

    /// Directory containing the frontend files to be served.
    Directory(PathBuf),

    /// Individual files that should be embedded into the application.
    Files(Vec<PathBuf>),
}

// ============================================================================
// impl Display for FrontendDist
// ============================================================================
//
// Provides a human-readable (and for `Files`, JSON) representation.
//
//   Url        -> prints the URL as-is.
//   Directory  -> prints the path via `Path::display()` (lossy, readable).
//   Files      -> serializes the path list to JSON and prints that.
//
// Notes / potential issues to review:
//
//   1. The `Files` arm uses `.unwrap()` on `serde_json::to_string(paths)`.
//      Serializing `Vec<PathBuf>` should not fail in practice, but
//      `unwrap()` inside a `Display` impl can panic during formatting.
//      Safer: fall back to `Debug` formatting or return `fmt::Error`.
//
//   2. `Display` for `Url` and `Directory` is human-oriented, while `Files`
//      produces JSON. That inconsistency may be intentional (for logging)
//      or may be worth reconsidering.
//
//   3. Because `FrontendDist` is both `Serialize` and `Display`, there are
//      two string representations. Make sure callers use the intended one.
//
impl Display for FrontendDist {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Url(address) => write!(formatter, "{address}"),
            Self::Directory(directory) => {
                write!(formatter, "{}", directory.display())
            }
            Self::Files(paths) => {
                write!(formatter, "{}", serde_json::to_string(paths).unwrap())
            }
        }
    }
}
/// A tuple struct of RGBA colors. Each value has minimum of 0 and maximum of 255.
#[derive(Debug, PartialEq, Eq, Serialize, Default, Clone, Copy)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Color(pub u8, pub u8, pub u8, pub u8);

impl From<Color> for (u8, u8, u8, u8) {
    fn from(value: Color) -> Self { (value.0, value.1, value.2, value.3) }
}

impl From<Color> for (u8, u8, u8) {
    fn from(value: Color) -> Self { (value.0, value.1, value.2) }
}

impl From<(u8, u8, u8, u8)> for Color {
    fn from(value: (u8, u8, u8, u8)) -> Self { Color(value.0, value.1, value.2, value.3) }
}

impl From<(u8, u8, u8)> for Color {
    fn from(value: (u8, u8, u8)) -> Self { Color(value.0, value.1, value.2, 255) }
}

impl From<Color> for [u8; 4] {
    fn from(value: Color) -> Self { [value.0, value.1, value.2, value.3] }
}

impl From<Color> for [u8; 3] {
    fn from(value: Color) -> Self { [value.0, value.1, value.2] }
}

impl From<[u8; 4]> for Color {
    fn from(value: [u8; 4]) -> Self { Color(value[0], value[1], value[2], value[3]) }
}

impl From<[u8; 3]> for Color {
    fn from(value: [u8; 3]) -> Self { Color(value[0], value[1], value[2], 255) }
}

impl FromStr for Color {
    type Err = String;
    fn from_str(mut color: &str) -> Result<Self, Self::Err> {
        color = color.trim().strip_prefix('#').unwrap_or(color);
        let color = match color.len() {
            3 => color.chars().flat_map(|c| std::iter::repeat_n(c, 2)).chain(std::iter::repeat_n('f', 2)).collect(),
            6 => format!("{color}FF"),
            8 => color.to_string(),
            _ => {
                return Err(
                    "Invalid hex color length, must be either 3, 6 or 8, for example: #fff, #ffffff, or #ffffffff"
                        .into(),
                );
            }
        };

        let r = u8::from_str_radix(&color[0..2], 16).map_err(|e| e.to_string())?;
        let g = u8::from_str_radix(&color[2..4], 16).map_err(|e| e.to_string())?;
        let b = u8::from_str_radix(&color[4..6], 16).map_err(|e| e.to_string())?;
        let a = u8::from_str_radix(&color[6..8], 16).map_err(|e| e.to_string())?;

        Ok(Color(r, g, b, a))
    }
}

fn default_alpha() -> u8 { 255 }

#[derive(Deserialize)]
#[serde(untagged)]
enum InnerColor {
    /// Color hex string, for example: #fff, #ffffff, or #ffffffff.
    String(String),
    /// Array of RGB colors. Each value has minimum of 0 and maximum of 255.
    Rgb((u8, u8, u8)),
    /// Array of RGBA colors. Each value has minimum of 0 and maximum of 255.
    Rgba((u8, u8, u8, u8)),
    /// Object of red, green, blue, alpha color values. Each value has minimum of 0 and maximum of 255.
    RgbaObject {
        red: u8,
        green: u8,
        blue: u8,
        #[serde(default = "default_alpha")]
        alpha: u8,
    },
}

impl<'de> Deserialize<'de> for Color {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let color = InnerColor::deserialize(deserializer)?;
        let color = match color {
            InnerColor::String(string) => string.parse().map_err(serde::de::Error::custom)?,
            InnerColor::Rgb(rgb) => Color(rgb.0, rgb.1, rgb.2, 255),
            InnerColor::Rgba(rgb) => rgb.into(),
            InnerColor::RgbaObject { red, green, blue, alpha } => Color(red, green, blue, alpha),
        };

        Ok(color)
    }
}

#[cfg(windows)]
#[derive(Debug, Serialize, Deserialize)]
pub enum FocusState {
    WindowFocused,
    WebviewFocused { webview_label: String },
    Blured { last_focused_webview_label: Option<String> },
}

#[cfg(windows)]
impl Default for FocusState {
    fn default() -> Self { Self::Blured { last_focused_webview_label: None } }
}

pub struct Config {
    pub product_name: String,
    pub uuid: String,
    pub id_name: String,
    pub data_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub temp_dir: PathBuf,
    pub current_exe: PathBuf,
}
impl Config {
    /// Decode MessagePack and initialize application paths.
    pub fn new(config: Vec<u8>) -> Result<Self> {
        let app = AppConfig::from_engine_bytes(&config)?;
        Self::from_app_config(app)
    }
    /// Create a resolved runtime configuration.
    pub fn from_app_config(app: AppConfig) -> Result<Self> {
        app.validate()?;
        let uuid = app.app_id;
        let product_name = app.product_name.unwrap_or_else(|| format!("taurino-{}", uuid));
        let normalized_name = normalize_name(&product_name);
        // UUID validation guarantees ASCII hexadecimal characters.
        let uuid_string = uuid.to_string();
        let short_uuid = &uuid_string[..8];
        let id_name = format!("{normalized_name}_{short_uuid}");
        let base_dirs = BaseDirs::new().context("Failed to resolve user base directories")?;
        // Persistent application-local data.
        let data_dir = base_dirs.data_local_dir().join(&id_name);
        // Re-creatable cache files.
        let cache_dir = base_dirs.cache_dir().join(&id_name);
        // Temporary runtime files.
        let temp_dir = env::temp_dir().join(&id_name);
        // Application executable can be unavailable in some environments.
        let current_exe = current_exe()?;
        for (label, path) in [("data", &data_dir), ("cache", &cache_dir), ("temporary", &temp_dir)] {
            fs::create_dir_all(path)
                .with_context(|| format!("Failed to create {label} directory: {}", path.display()))?;
        }
        Ok(Self { product_name, uuid: uuid_string, id_name, data_dir, cache_dir, temp_dir, current_exe })
    }
    /// Persistent WebView user data directory.
    pub fn webview_data_dir(&self) -> PathBuf { self.data_dir.join("webview") }
    /// Directory for extracted frontend resources.
    pub fn resources_dir(&self) -> PathBuf { self.data_dir.join("resources") }
    /// Directory for runtime logs.
    pub fn logs_dir(&self) -> PathBuf { self.data_dir.join("logs") }

    /// Resolves and prepares a window-specific WebView directory.
    pub fn prepare_webview_data_directory(
        &self,
        window_label: &str,
        data_directory: Option<PathBuf>,
    ) -> Result<Option<PathBuf>> {
        let Some(data_directory) = data_directory else {
            return Ok(None);
        };

        let resolved =
            crate::paths::prepare_webview_data_directory(&self.webview_data_dir(), window_label, &data_directory)?;

        Ok(Some(resolved))
    }
}
