use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::HashSet, sync::Arc};
use url::Url;
use wry::WebViewBuilder;

use crate::client::IpcClient;
/// JavaScript registered at document creation (before application scripts).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct InitializationScript {
    pub script: String,
    pub for_main_frame_only: bool,
}
/// Creates the Taurino JavaScript bridge for one webview.
///
/// IMPORTANT: The Wry WebViewBuilder must also install `with_ipc_handler`.
/// The native handler must independently authorize the actual webview/document
/// origin and command. Metadata and `documentId` supplied by JS are not trusted.
/// Native responses must be routed to the originating webview on the GUI thread:
///   window.__TAURINO_INTERNALS__.__completeIpc({
///     documentId: request.documentId,
///     callback: request.callback /* or request.error */,
///     data: result /* or error */
///   });
pub fn all_initialization_scripts(
    window_label: &str,
    webview_label: &str,
    use_https_scheme: bool,
    scripts_capacity: Option<usize>,
) -> Result<Vec<InitializationScript>> {
    fn main_frame_script(script: impl Into<String>) -> InitializationScript {
        InitializationScript {
            script: script.into(),
            for_main_frame_only: true,
        }
    }
    let mut scripts = Vec::with_capacity(scripts_capacity.unwrap_or(3).max(3));
    let metadata = serde_json::json!({
        "currentWindow": {"label": window_label},
        "currentWebview": {"label": webview_label, "windowLabel": window_label},
        "process": {"pid": std::process::id()},
        "platform": {
            "os": std::env::consts::OS,
            "family": std::env::consts::FAMILY,
            "arch": std::env::consts::ARCH,
            "target": format!("{}-{}", std::env::consts::ARCH, std::env::consts::OS)
        }
    });
    // 1. Namespace + immutable metadata. JS-visible metadata is informational only.
    let bootstrap = r#"
(() => {
    'use strict';
    const root = window;
    if (Object.hasOwn(root, '__TAURINO_INTERNALS__')) {
        throw new Error('Taurino internals already initialized');
    }
    const internals = Object.create(null);
    Object.defineProperty(root, '__TAURINO_INTERNALS__', {
        value: internals, writable: false, enumerable: false, configurable: false
    });
    Object.defineProperty(root, 'isTaurino', {
        value: true, writable: false, enumerable: true, configurable: false
    });
    const freeze = (value) => {
        if (value && typeof value === 'object' && !Object.isFrozen(value)) {
            for (const key of Object.keys(value)) freeze(value[key]);
            Object.freeze(value);
        }
        return value;
    };
    Object.defineProperty(internals, 'metadata', {
        value: freeze(__TAURINO_METADATA_JSON__),
        writable: false, enumerable: true, configurable: false
    });
    Object.defineProperty(internals, 'plugins', {
        value: Object.create(null),
        writable: false, enumerable: true, configurable: false
    });
})();
"#
    .replace(
        "__TAURINO_METADATA_JSON__",
        &serde_json::to_string(&metadata)?,
    );
    scripts.push(main_frame_script(bootstrap));
    // 2. Core APIs + Wry bridge. Keeping callback state in one closure avoids
    // external code changing callback IDs or resolving another request by ID.
    let scheme = if use_https_scheme { "https" } else { "http" };
    let core = r#"
(() => {
    'use strict';
    const internals = window.__TAURINO_INTERNALS__;
    const osName = __TAURINO_OS_JSON__;
    const protocolScheme = __TAURINO_SCHEME_JSON__;
    const documentId = (() => {
        const bytes = new Uint8Array(16);
        window.crypto.getRandomValues(bytes);
        return Array.from(bytes, (b) => b.toString(16).padStart(2, '0')).join('');
    })();
    const callbacks = new Map();
    const pending = new Map();
    const pendingPairs = new Map();
    const MAX_PENDING = 128;
    const MAX_MESSAGE_CHARS = 1024 * 1024;
    const REQUEST_TIMEOUT_MS = 30000;
    let closed = false;
    let nextCallbackId = 0;
    const define = (name, value, enumerable = true) => {
        Object.defineProperty(internals, name, {
            value, writable: false, configurable: false, enumerable
        });
    };
    function uid() {
        if (callbacks.size >= 0xffffffff) throw new Error('Callback registry exhausted');
        let id;
        do {
            id = nextCallbackId;
            nextCallbackId = (nextCallbackId + 1) >>> 0;
        } while (callbacks.has(id));
        return id;
    }
    function unregisterCallback(id) {
        return callbacks.delete(id);
    }
    function registerCallback(callback, once = false) {
        if (typeof callback !== 'function') throw new TypeError('Callback must be a function');
        const id = uid();
        callbacks.set(id, (value) => {
            if (once) callbacks.delete(id);
            return callback(value);
        });
        return id;
    }
    function runCallback(id, data) {
        const callback = callbacks.get(id);
        if (!callback) return false;
        callback(data);
        return true;
    }
    function convertFileSrc(filePath, protocol = 'asset') {
        if (typeof filePath !== 'string') throw new TypeError('filePath must be a string');
        if (typeof protocol !== 'string' || !/^[a-z][a-z0-9+.-]*$/.test(protocol)) {
            throw new TypeError('Invalid asset protocol');
        }
        const path = encodeURIComponent(filePath);
        return osName === 'windows' || osName === 'android'
            ? `${protocolScheme}://${protocol}.localhost/${path}`
            : `${protocol}://localhost/${path}`;
    }
    function send(message) {
        if (closed) throw new Error('Taurino bridge is closed');
        const native = window.ipc;
        if (!native || typeof native.postMessage !== 'function') {
            throw new Error('Wry IPC handler is not installed');
        }
        const serialized = JSON.stringify({ ...message, documentId });
        if (typeof serialized !== 'string' || serialized.length > MAX_MESSAGE_CHARS) {
            throw new RangeError('IPC message is invalid or too large');
        }
        native.postMessage(serialized);
    }
    function invoke(cmd, payload = {}, options) {
        if (typeof cmd !== 'string' || !cmd) {
            return Promise.reject(new TypeError('Invalid IPC command'));
        }
        if (closed) return Promise.reject(new Error('Taurino bridge is closed'));
        if (pending.size >= MAX_PENDING) {
            return Promise.reject(new Error('Too many outstanding IPC requests'));
        }
        return new Promise((resolve, reject) => {
            let successId;
            let errorId;
            let timer;
            let settled = false;
            const cleanup = () => {
                clearTimeout(timer);
                unregisterCallback(successId);
                unregisterCallback(errorId);
                pending.delete(successId);
                pendingPairs.delete(successId);
            };
            const finish = (ok, result) => {
                if (settled) return;
                settled = true;
                cleanup();
                if (ok) resolve(result);
                else reject(result instanceof Error ? result : new Error(String(result)));
            };
            successId = registerCallback((value) => finish(true, value), true);
            errorId = registerCallback((value) => finish(false, value), true);
            pending.set(successId, (reason) => finish(false, reason));
            pendingPairs.set(successId, errorId);
            timer = setTimeout(() => finish(false, new Error(`IPC request timed out: ${cmd}`)), REQUEST_TIMEOUT_MS);
            try {
                send({ kind: 'request', cmd, callback: successId, error: errorId, payload, options });
            } catch (error) {
                finish(false, error);
            }
        });
    }
    function notify(event, payload = null) {
        if (typeof event !== 'string' || !event) throw new TypeError('Invalid event');
        send({ kind: 'notification', cmd: event, payload });
        // This only means the message was submitted to the native bridge.
    }
    function completeIpc(response) {
        if (closed || !response || typeof response !== 'object') return;
        if (response.documentId !== documentId) return;
        if (!Number.isSafeInteger(response.callback) || response.callback < 0) return;
        // Only invoke success/error callbacks belonging to an outstanding request.
        // The completion can arrive after timeout; then it is safely ignored.
        const valid = pending.has(response.callback) ||
            Array.from(pending.keys()).some((id) => {
                // Check the explicit success/error callback pair.
                return pendingPairs.get(id) === response.callback;
            });
        if (valid) runCallback(response.callback, response.data);
    }
    // Explicitly track error IDs; do not infer ownership by callback arithmetic.
    define('convertFileSrc', convertFileSrc);
    define('transformCallback', registerCallback);
    define('unregisterCallback', unregisterCallback);
    define('runCallback', runCallback);
    define('invoke', invoke);
    define('notify', notify);
    define('__completeIpc', completeIpc, false);
    define('__documentId', documentId, false);
    window.addEventListener('pagehide', () => {
        closed = true;
        for (const rejectPending of Array.from(pending.values())) {
            rejectPending(new Error('WebView document unloaded'));
        }
        pending.clear();
        pendingPairs.clear();
        callbacks.clear();
    }, { once: true });
})();
"#
    .replace("__TAURINO_OS_JSON__", &serde_json::to_string(std::env::consts::OS)?)
    .replace("__TAURINO_SCHEME_JSON__", &serde_json::to_string(scheme)?);
    scripts.push(main_frame_script(core));
    Ok(scripts)
}
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BridgeScope {
    pub window_label: String,
    pub webview_label: String,
    /// Exact permitted document origin, e.g. "http\://taurino.localhost".
    /// Use a value computed from the resolved WebViewUrl, NOT from JS metadata.
    pub allowed_origin: String,
    /// Allowlisted Python event names callable by JS.
    pub allowed_commands: HashSet<String>,
}
/// Validated input from the Wry IPC callback. JS fields are not authority.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct JsMessage {
    kind: String,
    cmd: String,
    #[serde(default)]
    payload: Value,
    #[serde(default)]
    callback: Option<u32>,
    #[serde(default)]
    error: Option<u32>,
    #[serde(rename = "documentId")]
    document_id: String,
    #[serde(default)]
    options: Option<Value>,
}

#[derive(Debug, Serialize)]
pub struct BridgeRequest {
    pub window_label: String,
    pub webview_label: String,
    pub document_id: String,
    pub event: String,
    pub payload: Value,
    pub callbacks: Option<(u32, u32)>,
}

#[derive(Debug, Clone, Serialize)]
pub struct JsCompletion {
    #[serde(rename = "documentId")]
    pub document_id: String,
    pub callback: u32,
    pub data: Value,
}
#[derive(Debug)]
pub enum BridgeEvent {
    Complete {
        window_label: String,
        webview_label: String,
        completion: JsCompletion,
    },
}
/// Install on a builder *after* app scripts; caller applies Wry build on GUI thread.
/// `EventLoopProxy<YourEvent>` can be supplied by mapping BridgeEvent externally:
/// use the Emitter to enqueue the validated request for IPCRuntime.
/// Install the synchronous Wry entry point. Actual IPC RPC is processed by
/// IPCRuntime on Tokio; this callback only validates and enqueues.
pub fn install_ipc_bridge<'a>(
    builder: WebViewBuilder<'a>,
    scope: BridgeScope,
    emitter: IpcClient,
) -> WebViewBuilder<'a> {
    let scope = Arc::new(scope);
    builder.with_ipc_handler(move |request| {
        if !is_allowed_origin(&request.uri().to_string(), &scope.allowed_origin) {
            eprintln!("Taurino: IPC rejected from origin {}", request.uri());
            return;
        }
        if request.body().len() > 1024 * 1024 {
            eprintln!("Taurino: oversized JS IPC request");
            return;
        }
        let message: JsMessage = match serde_json::from_str(request.body()) {
            Ok(value) => value,
            Err(error) => {
                eprintln!("Taurino: malformed JS IPC request: {error}");
                return;
            }
        };
        if message.cmd.is_empty()
            || message.cmd.len() > 128
            || !scope.allowed_commands.contains(&message.cmd)
            || message.document_id.len() != 32
            || !message.document_id.bytes().all(|b| b.is_ascii_hexdigit())
        {
            eprintln!("Taurino: unauthorized/invalid JS IPC request");
            return;
        }
        let callbacks = match message.kind.as_str() {
            "request" => match (message.callback, message.error) {
                (Some(ok), Some(err)) if ok != err => Some((ok, err)),
                _ => return,
            },
            "notification" if message.callback.is_none() && message.error.is_none() => None,
            _ => return,
        };
        let _ = message.options; // advisory only; never a permission source
        let outgoing = BridgeRequest {
            window_label: scope.window_label.clone(),
            webview_label: scope.webview_label.clone(),
            document_id: message.document_id,
            event: message.cmd,
            payload: message.payload,
            callbacks,
        };
        if let Err(error) = emitter.send(rmp_serde::to_vec(&outgoing).unwrap()) {
            eprintln!("Taurino: unable to enqueue JS IPC: {error:#}");
        }
    })
}

fn is_allowed_origin(raw_uri: &str, origin: &str) -> bool {
    let (Ok(url), Ok(expected)) = (Url::parse(raw_uri), Url::parse(origin)) else {
        return false;
    };
    // Explicit URL tuple comparison also works with custom `taurino:` schemes,
    // whose standard web-origin serialization might be opaque (`null`).
    url.scheme() == expected.scheme()
        && url.host_str() == expected.host_str()
        && url.port_or_known_default() == expected.port_or_known_default()
        && url.username().is_empty()
        && url.password().is_none()
        && expected.username().is_empty()
        && expected.password().is_none()
}
/// Call exclusively from your Tao/Wry GUI event loop, using the WebView identified
/// by `webview_label`; never call evaluate_script on the Tokio worker thread.
pub fn completion_script(completion: &JsCompletion) -> Result<String> {
    let json = serde_json::to_string(completion)?;
    // JSON string encoded inside a JS string: no user-originated source evaluation.
    let js_string = serde_json::to_string(&json)?;
    Ok(format!(
        "window.__TAURINO_INTERNALS__?.__completeIpc(JSON.parse({js_string}));"
    ))
}
