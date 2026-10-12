use std::{collections::HashSet, path::PathBuf};

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use serde_bytes::ByteBuf;
use uuid::Uuid;

// ============================================================
// Enums
// ============================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WindowPosition {
    Center,
    Default,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MenuItemType {
    Item,
    Separator,
    Submenu,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IpcSecurity {
    None,
    Authenticated,
}

// ============================================================
// IPC Client Bootstrap
// ============================================================

/// Configuration passed from the Python IPC server
/// to the Rust engine.
///
/// Contains only client-side connection information.
///
/// The Python server's private identity key is never
/// included in this structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct IpcClientConfig {
    pub endpoint: String,

    pub connect_timeout: f64,
    pub operation_timeout: f64,
    pub handshake_timeout: f64,

    pub max_message_size: u64,

    pub security: IpcSecurity,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server_public_key: Option<ByteBuf>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_private_key_path: Option<PathBuf>,
}

impl IpcClientConfig {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            !self.endpoint.trim().is_empty(),
            "IPC endpoint must not be blank"
        );

        for (name, value) in [
            ("connectTimeout", self.connect_timeout),
            ("operationTimeout", self.operation_timeout),
            ("handshakeTimeout", self.handshake_timeout),
        ] {
            ensure!(
                value.is_finite() && value > 0.0,
                "{name} must be a finite positive number"
            );
        }

        ensure!(
            (1024..=64 * 1024 * 1024).contains(&self.max_message_size),
            "maxMessageSize is outside the allowed range"
        );

        match self.security {
            IpcSecurity::None => {
                ensure!(
                    self.server_public_key.is_none() && self.client_private_key_path.is_none(),
                    "Unauthenticated IPC must not contain identity keys"
                );
            }

            IpcSecurity::Authenticated => {
                let public_key = self
                    .server_public_key
                    .as_ref()
                    .context("Missing serverPublicKey")?;

                ensure!(
                    public_key.len() == 32,
                    "serverPublicKey must contain 32 bytes"
                );

                let path = self
                    .client_private_key_path
                    .as_ref()
                    .context("Missing clientPrivateKeyPath")?;

                ensure!(
                    !path.as_os_str().is_empty(),
                    "clientPrivateKeyPath must not be empty"
                );
            }
        }

        Ok(())
    }
}

// ============================================================
// WebView
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WebViewConfig {
    pub url: String,
    pub label: String,

    pub x: i64,
    pub y: i64,

    pub width: Option<u32>,
    pub height: Option<u32>,

    pub visible: bool,
    pub devtools: bool,
}

impl WebViewConfig {
    pub fn validate(&self) -> Result<()> {
        ensure!(!self.label.is_empty(), "WebView label must not be empty");

        if let Some(width) = self.width {
            ensure!(width > 0, "WebView width must be positive");
        }

        if let Some(height) = self.height {
            ensure!(height > 0, "WebView height must be positive");
        }

        Ok(())
    }
}

// ============================================================
// Window
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WindowConfig {
    pub label: String,
    pub center: bool,
    pub title: String,

    pub width: u32,
    pub height: u32,

    pub min_width: Option<u32>,
    pub min_height: Option<u32>,

    pub resizable: bool,
    pub decorations: bool,
    pub transparent: bool,
    pub visible: bool,

    pub fullscreen: bool,
    pub maximized: bool,

    #[serde(default)]
    pub webviews: Vec<WebViewConfig>,
}

impl WindowConfig {
    pub fn validate(&self) -> Result<()> {
        ensure!(!self.label.is_empty(), "Window label must not be empty");

        ensure!(
            self.width > 0 && self.height > 0,
            "Window dimensions must be positive"
        );

        if let Some(width) = self.min_width {
            ensure!(width > 0, "minWidth must be positive");
        }

        if let Some(height) = self.min_height {
            ensure!(height > 0, "minHeight must be positive");
        }

        for webview in &self.webviews {
            webview.validate()?;
        }

        Ok(())
    }
}

// ============================================================
// Shortcuts
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ShortcutConfig {
    pub accelerator: String,
    pub action: String,
    pub enabled: bool,
}

// ============================================================
// Menus
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MenuItemConfig {
    pub id: String,
    pub text: String,
    pub kind: MenuItemType,
    pub enabled: bool,
    pub accelerator: Option<String>,

    #[serde(default)]
    pub children: Vec<MenuItemConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MenuConfig {
    #[serde(default)]
    pub items: Vec<MenuItemConfig>,
}

// ============================================================
// System Tray
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TrayIconConfig {
    pub id: String,
    pub icon: Option<String>,
    pub tooltip: Option<String>,
    pub visible: bool,
    pub menu: Option<MenuConfig>,
}

// ============================================================
// Application Config
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Config {
    pub productname: String,
    pub version: String,

    #[serde(with = "uuid_as_string")]
    pub app_id: Uuid,

    pub connection: IpcClientConfig,

    #[serde(default)]
    pub windows: Vec<WindowConfig>,

    #[serde(default)]
    pub shortcuts: Vec<ShortcutConfig>,

    #[serde(default)]
    pub trays: Vec<TrayIconConfig>,

    pub app_menu: Option<MenuConfig>,

    pub schema_version: u32,
}

impl Config {
    /// Deserialize a Python MessagePack engine bootstrap.
    ///
    /// Expected input:
    /// Python Config.to_engine_bytes()
    pub fn from_engine_bytes(bytes: &[u8]) -> Result<Self> {
        const MAX_BOOTSTRAP_SIZE: usize = 8 * 1024 * 1024;

        ensure!(
            bytes.len() <= MAX_BOOTSTRAP_SIZE,
            "Engine configuration exceeds maximum bootstrap size"
        );

        let config: Self =
            rmp_serde::from_slice(bytes).context("Failed to deserialize engine configuration")?;

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
        ensure!(
            self.schema_version == 1,
            "Unsupported schema version: {}",
            self.schema_version
        );

        ensure!(
            !self.productname.trim().is_empty(),
            "productname must not be blank"
        );

        ensure!(!self.version.trim().is_empty(), "version must not be blank");

        self.connection.validate()?;

        let mut window_labels = HashSet::new();
        let mut webview_labels = HashSet::new();

        for window in &self.windows {
            window.validate()?;

            ensure!(
                window_labels.insert(window.label.as_str()),
                "Duplicate window label: {}",
                window.label
            );

            for webview in &window.webviews {
                ensure!(
                    webview_labels.insert(webview.label.as_str()),
                    "Duplicate WebView label: {}",
                    webview.label
                );
            }
        }

        Ok(())
    }
}

// ============================================================
// UUID String Serialization
// ============================================================

/// Python's Pydantic model_dump(mode="json") encodes
/// UUID values as strings.
///
/// Explicitly use the same representation in MessagePack.
mod uuid_as_string {
    use serde::{Deserialize, Deserializer, Serializer};

    use uuid::Uuid;

    pub fn serialize<S>(value: &Uuid, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&value.to_string())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Uuid, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;

        Uuid::parse_str(&value).map_err(serde::de::Error::custom)
    }
}
