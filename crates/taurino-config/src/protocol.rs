use {
    serde::{Deserialize, Serialize},
    serde_json::Value,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvokeRequest {
    pub cmd: String,
    pub callback: u32,
    pub error: u32,

    #[serde(default)]
    pub payload: Value,

    #[serde(default)]
    pub options: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InvokeResponse {
    pub ok: bool,
    pub data: Value,
}

#[derive(Debug, Deserialize)]
pub struct NotificationTarget {
    pub webview_label: String,

    #[serde(default)]
    pub window_label: Option<String>,
}
