use {
    anyhow::{Result, ensure},
    serde::{Deserialize, Deserializer, Serialize, de::Error as DeError},
    serde_repr::{Deserialize_repr, Serialize_repr},
    std::{
        path::{Component, Display, Path, PathBuf},
        str::FromStr,
    },
};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct SafePathBuf(PathBuf);

impl SafePathBuf {
    /// Creates a validated path without parent-directory traversal.
    pub fn new(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();

        ensure!(!path.as_os_str().is_empty(), "Path must not be empty");

        ensure!(
            !path.components().any(|component| { matches!(component, Component::ParentDir) }),
            "Parent-directory traversal ('..') is not allowed"
        );

        Ok(Self(path))
    }

    /// Returns the underlying path.
    pub fn as_path(&self) -> &Path { self.0.as_path() }

    /// Returns a display formatter for the path.
    pub fn display(&self) -> Display<'_> { self.0.display() }

    /// Consumes the wrapper and returns the PathBuf.
    pub fn into_inner(self) -> PathBuf { self.0 }
}

impl AsRef<Path> for SafePathBuf {
    fn as_ref(&self) -> &Path { self.as_path() }
}

impl FromStr for SafePathBuf {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self> { Self::new(value) }
}

impl TryFrom<PathBuf> for SafePathBuf {
    type Error = anyhow::Error;

    fn try_from(path: PathBuf) -> Result<Self> { Self::new(path) }
}

impl From<SafePathBuf> for PathBuf {
    fn from(path: SafePathBuf) -> Self { path.into_inner() }
}

impl<'de> Deserialize<'de> for SafePathBuf {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let path = PathBuf::deserialize(deserializer)?;

        Self::new(path).map_err(D::Error::custom)
    }
}

/// A base directory for a path.
///
/// The base directory is the optional root of a file system operation.
/// If informed by the API call, all paths will be relative to the path of the given directory.
///
/// For more information, check the [`dirs` documentation](https://docs.rs/dirs/).
#[derive(Serialize_repr, Deserialize_repr, Clone, Copy, Debug)]
#[repr(u16)]
#[non_exhaustive]
pub enum BaseDirectory {
    /// The Audio directory.
    /// Resolves to [`crate::path::PathResolver::audio_dir`].
    Audio = 1,
    /// The Cache directory.
    /// Resolves to [`crate::path::PathResolver::cache_dir`].
    Cache = 2,
    /// The Config directory.
    /// Resolves to [`crate::path::PathResolver::config_dir`].
    Config = 3,
    /// The Data directory.
    /// Resolves to [`crate::path::PathResolver::data_dir`].
    Data = 4,
    /// The LocalData directory.
    /// Resolves to [`crate::path::PathResolver::local_data_dir`].
    LocalData = 5,
    /// The Document directory.
    /// Resolves to [`crate::path::PathResolver::document_dir`].
    Document = 6,
    /// The Download directory.
    /// Resolves to [`crate::path::PathResolver::download_dir`].
    Download = 7,
    /// The Picture directory.
    /// Resolves to [`crate::path::PathResolver::picture_dir`].
    Picture = 8,
    /// The Public directory.
    /// Resolves to [`crate::path::PathResolver::public_dir`].
    Public = 9,
    /// The Video directory.
    /// Resolves to [`crate::path::PathResolver::video_dir`].
    Video = 10,
    /// The Resource directory.
    /// Resolves to [`crate::path::PathResolver::resource_dir`].
    Resource = 11,
    /// A temporary directory.
    /// Resolves to [`std::env::temp_dir`].
    Temp = 12,
    /// The default app config directory.
    /// Resolves to [`crate::path::PathResolver::app_config_dir`],
    /// which can be overridden with the `app > appDirectoriesOverride` config.
    AppConfig = 13,
    /// The default app data directory.
    /// Resolves to [`crate::path::PathResolver::app_data_dir`],
    /// which can be overridden with the `app > appDirectoriesOverride` config.
    AppData = 14,
    /// The default app local data directory.
    /// Resolves to [`crate::path::PathResolver::app_local_data_dir`],
    /// which can be overridden with the `app > appDirectoriesOverride` config.
    AppLocalData = 15,
    /// The default app cache directory.
    /// Resolves to [`crate::path::PathResolver::app_cache_dir`],
    /// which can be overridden with the `app > appDirectoriesOverride` config.
    AppCache = 16,
    /// The default app log directory.
    /// Resolves to [`crate::path::PathResolver::app_log_dir`],
    /// which can be overridden with the `app > appDirectoriesOverride` config.
    AppLog = 17,
    /// The Desktop directory.
    /// Resolves to [`crate::path::PathResolver::desktop_dir`].
    #[cfg(not(target_os = "android"))]
    Desktop = 18,
    /// The Executable directory.
    /// Resolves to [`crate::path::PathResolver::executable_dir`].
    #[cfg(not(target_os = "android"))]
    Executable = 19,
    /// The Font directory.
    /// Resolves to [`crate::path::PathResolver::font_dir`].
    #[cfg(not(target_os = "android"))]
    Font = 20,
    /// The Home directory.
    /// Resolves to [`crate::path::PathResolver::home_dir`].
    Home = 21,
    /// The Runtime directory.
    /// Resolves to [`crate::path::PathResolver::runtime_dir`].
    #[cfg(not(target_os = "android"))]
    Runtime = 22,
    /// The Template directory.
    /// Resolves to [`crate::path::PathResolver::template_dir`].
    #[cfg(not(target_os = "android"))]
    Template = 23,
}

impl BaseDirectory {
    /// Gets the variable that represents this [`BaseDirectory`] for string paths.
    pub fn variable(self) -> &'static str {
        match self {
            Self::Audio => "$AUDIO",
            Self::Cache => "$CACHE",
            Self::Config => "$CONFIG",
            Self::Data => "$DATA",
            Self::LocalData => "$LOCALDATA",
            Self::Document => "$DOCUMENT",
            Self::Download => "$DOWNLOAD",
            Self::Picture => "$PICTURE",
            Self::Public => "$PUBLIC",
            Self::Video => "$VIDEO",
            Self::Resource => "$RESOURCE",
            Self::Temp => "$TEMP",
            Self::AppConfig => "$APPCONFIG",
            Self::AppData => "$APPDATA",
            Self::AppLocalData => "$APPLOCALDATA",
            Self::AppCache => "$APPCACHE",
            Self::AppLog => "$APPLOG",
            Self::Home => "$HOME",

            #[cfg(not(target_os = "android"))]
            Self::Desktop => "$DESKTOP",
            #[cfg(not(target_os = "android"))]
            Self::Executable => "$EXE",
            #[cfg(not(target_os = "android"))]
            Self::Font => "$FONT",
            #[cfg(not(target_os = "android"))]
            Self::Runtime => "$RUNTIME",
            #[cfg(not(target_os = "android"))]
            Self::Template => "$TEMPLATE",
        }
    }

    /// Gets the [`BaseDirectory`] associated with the given variable, or [`None`] if the variable doesn't match any.
    pub fn from_variable(variable: &str) -> Option<Self> {
        let res = match variable {
            "$AUDIO" => Self::Audio,
            "$CACHE" => Self::Cache,
            "$CONFIG" => Self::Config,
            "$DATA" => Self::Data,
            "$LOCALDATA" => Self::LocalData,
            "$DOCUMENT" => Self::Document,
            "$DOWNLOAD" => Self::Download,

            "$PICTURE" => Self::Picture,
            "$PUBLIC" => Self::Public,
            "$VIDEO" => Self::Video,
            "$RESOURCE" => Self::Resource,
            "$TEMP" => Self::Temp,
            "$APPCONFIG" => Self::AppConfig,
            "$APPDATA" => Self::AppData,
            "$APPLOCALDATA" => Self::AppLocalData,
            "$APPCACHE" => Self::AppCache,
            "$APPLOG" => Self::AppLog,
            "$HOME" => Self::Home,

            #[cfg(not(target_os = "android"))]
            "$DESKTOP" => Self::Desktop,
            #[cfg(not(target_os = "android"))]
            "$EXE" => Self::Executable,
            #[cfg(not(target_os = "android"))]
            "$FONT" => Self::Font,
            #[cfg(not(target_os = "android"))]
            "$RUNTIME" => Self::Runtime,
            #[cfg(not(target_os = "android"))]
            "$TEMPLATE" => Self::Template,

            _ => return None,
        };
        Some(res)
    }
}

/// Validates, resolves and creates a WebView data directory.
pub fn prepare_webview_data_directory(base_dir: &Path, window_label: &str, data_directory: &Path) -> Result<PathBuf> {
    // Validate window label.
    let mut components = Path::new(window_label).components();

    ensure!(
        matches!(components.next(), Some(Component::Normal(_))) && components.next().is_none(),
        "Invalid window label: expected a single directory component"
    );

    // Validate configured relative directory.
    let safe_path = SafePathBuf::new(data_directory.to_path_buf())?;

    let relative_path: &Path = safe_path.as_ref();

    ensure!(
        relative_path.is_relative()
            && relative_path
                .components()
                .all(|component| { matches!(component, Component::Normal(_) | Component::CurDir) }),
        "WebView data directory must be a safe relative path"
    );

    // Resolve directory within the application data root.
    let resolved_path = base_dir.join(window_label).join(relative_path);

    Ok(resolved_path)
}
