use {
    anyhow::{Context, Result, ensure},
    serde::{Deserialize, Serialize},
    serde_bytes::ByteBuf,
    std::path::PathBuf,
};

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

    #[serde(default = "default_max_clients")]
    pub max_clients: u16,

    pub security: IpcSecurity,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server_public_key: Option<ByteBuf>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_private_key_path: Option<PathBuf>,
}

const fn default_max_clients() -> u16 { 1 }

impl IpcClientConfig {
    pub fn validate(&self) -> Result<()> {
        ensure!(!self.endpoint.trim().is_empty(), "IPC endpoint must not be blank");
        for (name, value) in [
            ("connectTimeout", self.connect_timeout),
            ("operationTimeout", self.operation_timeout),
            ("handshakeTimeout", self.handshake_timeout),
        ] {
            ensure!(value.is_finite() && value > 0.0, "{name} must be a finite positive number");
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
                let public_key = self.server_public_key.as_ref().context("Missing serverPublicKey")?;
                ensure!(public_key.len() == 32, "serverPublicKey must contain 32 bytes");
                let path = self.client_private_key_path.as_ref().context("Missing clientPrivateKeyPath")?;
                ensure!(!path.as_os_str().is_empty(), "clientPrivateKeyPath must not be empty");
            }
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
