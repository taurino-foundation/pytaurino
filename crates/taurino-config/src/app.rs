use {
    crate::{
        mix::{IpcClientConfig, MenuConfig, ShortcutConfig, TrayIconConfig},
        utils::uuid_as_string,
        version_deserializer,
        window::WindowConfig,
    },
    anyhow::{Context, Result, ensure},
    serde::{Deserialize, Serialize},
    std::collections::HashSet,
    url::Url,
    uuid::Uuid,
};

// ============================================================
// Application Config
// ============================================================
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AppConfig {
    /// Visible name of the application.
    ///
    /// This name is used for display in the system and can
    /// also influence package names, install paths, and metadata.
    #[serde(alias = "product-name")]
    pub product_name: Option<String>,
    #[serde(deserialize_with = "version_deserializer", default)]
    pub version: Option<String>,
    #[serde(with = "uuid_as_string")]
    pub app_id: Uuid,
    /// Configuration of the IPC connection.
    pub connection: IpcClientConfig,
    /// Settings of the application windows.
    pub windows: Vec<WindowConfig>,

    /// Address of the user interface during development.
    ///
    /// Usually this URL points to a development server
    /// that serves the frontend files and delivers changes directly.
    #[serde(alias = "dev-url")]
    pub dev_url: Option<Url>,
    #[serde(default)]
    pub shortcuts: Vec<ShortcutConfig>,
    #[serde(default)]
    pub trays: Vec<TrayIconConfig>,
    #[serde(default)]
    pub app_menu: Option<MenuConfig>,
    pub schema_version: u32,
}
impl AppConfig {
    /// Deserialize a Python MessagePack engine bootstrap.
    ///
    /// Expected input:
    /// Python Config.to_engine_bytes()
    pub fn from_engine_bytes(bytes: &[u8]) -> Result<Self> {
        const MAX_BOOTSTRAP_SIZE: usize = 8 * 1024 * 1024;
        ensure!(!bytes.is_empty(), "Engine configuration must not be empty");
        ensure!(bytes.len() <= MAX_BOOTSTRAP_SIZE, "Engine configuration exceeds maximum bootstrap size");
        let config: Self = rmp_serde::from_slice(bytes).context("Failed to deserialize engine configuration")?;
        config.validate().context("Invalid engine configuration")?;
        Ok(config)
    }
    /// Serialize using named MessagePack map fields.
    pub fn to_engine_bytes(&self) -> Result<Vec<u8>> {
        self.validate()?;
        rmp_serde::to_vec_named(self).context("Failed to serialize engine configuration")
    }
    /// Validate application configuration.
    pub fn validate(&self) -> Result<()> {
        ensure!(self.schema_version == 1, "Unsupported schema version: {}", self.schema_version);
        self.connection.validate()?;
        let mut window_labels = HashSet::new();
        let mut webview_labels = HashSet::new();
        for window in &self.windows {
            ensure!(window_labels.insert(window.label.as_str()), "Duplicate window label: {}", window.label);
            for webview in &window.webviews {
                ensure!(webview_labels.insert(webview.label.as_str()), "Duplicate WebView label: {}", webview.label);
            }
        }
        Ok(())
    }
}

/*

/// Re-exports of the [`serde`] crate for consumers of this engine.
///
// ============================================================================
// Imports
// ============================================================================
//
// std::fmt::{self, Display, Formatter}
//   - `Display` and `Formatter` are needed to implement the Display trait
//     for `FrontendDist`.
//   - `self` imports the `fmt` module itself so we can refer to `fmt::Result`.
//
// std::path::PathBuf
//   - Owned, heap-allocated filesystem path.
//   - Used for the `Directory` and `Files` variants of `FrontendDist`.
//
// serde::{Deserialize, Serialize}
//   - Derive macros to read from / write to config formats (JSON, TOML, ...).
//
// url::Url
//   - Parsed URL type, used by `dev_url` and `FrontendDist::Url`.
//
// crate::window::WindowConfig
//   - Per-window configuration, defined elsewhere in this crate.
//

// ============================================================================
// ConnecionConfig
// ============================================================================
//
// Small struct holding the IPC endpoint path.
//
// - Only `Deserialize` is derived: this is meant to be read from a config
//   file, not written back out.
// - NOTE: the name is misspelled (`ConnecionConfig` instead of
//   `ConnectionConfig`). This is a real identifier and must either be
//   renamed consistently everywhere or kept as-is.
//
/// Settings for inter-process communication.
#[derive(Debug, Clone, Deserialize)]
pub struct ConnecionConfig {
    /// Path of the IPC endpoint.
    pub path: String,
}

// ============================================================================
// AppConfig
// ============================================================================
//
// Top-level application configuration. All fields are read from the config
// file via serde.
//
// Field-by-field notes:
//
// product_name:
//   - Optional.
//   - `#[serde(alias = "product-name")]` allows either `product_name` or
//     `product-name` as the key in the config file.
//   - Human-readable name used for system display. May also influence
//     package names, install paths, and metadata.
//
// version:
//   - Optional.
//   - Uses a custom deserializer `crate::package::version_deserializer`.
//   - Accepts either a SemVer string OR a path to a `package.json`, from
//     which the `version` field is read.
//   - `default` means the field may be missing entirely.
//
// connection:
//   - Required (not Option, no default).
//   - Holds IPC connection settings (see ConnecionConfig above).
//
// windows:
//   - Required list of window definitions.
//   - Each entry is a WindowConfig describing one application window.
//
// dev_url:
//   - Optional.
//   - `#[serde(alias = "dev-url")]` allows `dev_url` or `dev-url`.
//   - URL of the frontend dev server during development.
//
// frontend_dist:
//   - Optional.
//   - `#[serde(alias = "frontend-dist")]` allows `frontend_dist` or
//     `frontend-dist`.
//   - Describes where the frontend content comes from (URL, directory, or
//     explicit file list). See FrontendDist below.
//   - Relative directory paths are resolved relative to the config file.
//
// identifier:
//   - Required.
//   - Reverse-DNS application identifier, e.g. `com.example.desktop`.
//   - Used to associate app-specific system data and paths.
//   - Intended characters: ASCII letters, digits, hyphens, and dots.
//   - NOTE: the doc says "intended", but the type does NOT enforce this.
//     Validation must happen elsewhere.
//
/// General configuration of the application.
#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    /// Visible name of the application.
    ///
    /// This name is used for display in the system and can
    /// also influence package names, install paths, and metadata.
    #[serde(alias = "product-name")]
    pub product_name: Option<String>,

    /// Version of the application.
    ///
    /// Supports a version number in SemVer format or a path to
    /// a `package.json` whose `version` entry should be read.
    /// Processing is handled by the project-specific version deserializer.
    #[serde(deserialize_with = "version_deserializer", default)]
    pub version: Option<String>,

    /// Configuration of the IPC connection.
    pub connection: ConnecionConfig,

    /// Settings of the application windows.
    pub windows: Vec<WindowConfig>,

    /// Address of the user interface during development.
    ///
    /// Usually this URL points to a development server
    /// that serves the frontend files and delivers changes directly.
    #[serde(alias = "dev-url")]
    pub dev_url: Option<Url>,

    /// Source of the frontend content.
    ///
    /// Possible values are a URL, a local directory, or individual file paths.
    /// A relative directory path is resolved starting from the configuration
    /// file.
    ///
    /// For a directory, its contents are embedded recursively;
    /// `index.html` serves as the entry point. A file list allows
    /// targeted selection of the resources to embed.
    ///
    /// If a URL is specified, the application loads its content from that
    /// address instead of embedding local frontend files for it.
    #[serde(alias = "frontend-dist")]
    pub frontend_dist: Option<FrontendDist>,

    /// Unique application identifier in reverse domain notation,
    /// for example `com.example.desktop`.
    ///
    /// It is used to associate application-specific system data and paths.
    /// Intended are ASCII letters, digits, hyphens, and dots.
    pub identifier: String,
}



*/
