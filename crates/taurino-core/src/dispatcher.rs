use {
    crate::{EngineLoopProxy, EventLoopMessage, webview::WebView},
    anyhow::{Context, Result},
    rmpv::Value as RmpvValue,
    serde::Deserialize,
    serde_json::{Value as JsonValue, json},
    std::collections::HashMap,
    taurino_config::{
        NativeMessage,
        protocol::{InvokeRequest, InvokeResponse},
    },
    wry::{InitializationScript, WebViewBuilder},
};

pub type RpcResponse = (u16, i32, String, JsonValue);

/// Decode IPC response in the async client, then deliver InvokeResponse
/// to the GUI event loop. Call this only from the Tao GUI thread with
/// the original request and the originating WebView (not an arbitrary WebView).
pub fn resolve_invoke(
    webview: &WebView,
    request: &InvokeRequest,
    response: std::result::Result<InvokeResponse, String>,
) -> Result<()> {
    // IPC/transport failures must reject the JS Promise, not leave it pending.
    let (id, data) = match response {
        Ok(response) if response.ok => (request.callback, response.data),
        Ok(response) => (request.error, response.data),
        Err(message) => (
            request.error,
            serde_json::json!({
                "code": "IPC_ERROR",
                "message": message,
                "details": null
            }),
        ),
    };

    let json = serde_json::to_string(&data).context("Failed to encode JS invoke callback data")?;

    let script = format!("window.__TAURINO_INTERNALS__.runCallback({id}, {json});");

    webview.evaluate_script(&script).context("Failed to evaluate JS invoke callback")?;
    Ok(())
}

// Example in your async IPC client (not on the GUI event loop):
// let response: InvokeResponse = rmp_serde::from_slice(&received_msgpack)?;
// proxy.send_event(EventLoopMessage::JsInvokeCompleted { webview_label, request, response })?;
//
// Example in Tao's Event::UserEvent handler (GUI thread):
// let webview = manager.get_webview(&webview_label)?;
// resolve_invoke(webview, &request, Ok(response))?;

/// Handle a Python -> Rust RPC request.
///
/// Request:
/// [u16 request_id, string method, array arguments]
///
/// Response:
/// [u16 request_id, i32 code, string message, data]
pub fn dispatch(payload: &[u8]) -> Result<Vec<u8>> {
    let (request_id, method, arguments): (u16, String, Vec<RmpvValue>) =
        rmp_serde::from_slice(payload).context("Invalid IPC request")?;

    match method.as_str() {
        "engine.ping" => {
            if !arguments.is_empty() {
                return response_error(request_id, 400, "engine.ping does not accept arguments");
            }
            response_ok(request_id, "pong")
        }

        _ => response_error(request_id, 404, &format!("Unknown engine method: {method}")),
    }
}

pub fn response_ok(request_id: u16, data: &str) -> Result<Vec<u8>> {
    Ok(rmp_serde::to_vec(&(request_id, 0_i32, "ok", data))?)
}

pub fn response_error(request_id: u16, code: i32, message: &str) -> Result<Vec<u8>> {
    Ok(rmp_serde::to_vec(&(request_id, code, message, Option::<String>::None))?)
}

pub fn install_ipc_bridge(
    builder: WebViewBuilder<'_>,
    window_label: String,
    webview_label: String,
    proxy: EngineLoopProxy,
) -> WebViewBuilder<'_> {
    builder.with_ipc_handler(move |request| {
        if request.body().len() > 1024 * 1024 {
            eprintln!("IPC message too large");
            return;
        }

        let message: InvokeRequest = match serde_json::from_str(request.body()) {
            Ok(message) => message,
            Err(error) => {
                eprintln!("Invalid IPC message: {error}");
                return;
            }
        };

        let event = NativeMessage::JsInvoke {
            window_label: window_label.clone(),
            webview_label: webview_label.clone(),
            request: message,
        };

        if let Err(error) = proxy.send_native_event(event) {
            eprintln!("Failed to forward JS invoke: {error}");
        }
    })
}
pub fn all_initialization_scripts(
    window_label: &str,
    webview_label: &str,
    use_https_scheme: bool,
    scripts_capacity: Option<usize>,
) -> Result<Vec<InitializationScript>> {
    fn main_frame_script(script: impl Into<String>) -> InitializationScript {
        InitializationScript { script: script.into(), for_main_frame_only: true }
    }

    let mut scripts = Vec::with_capacity(scripts_capacity.unwrap_or(3));

    // ----------------------------------------------------------
    // 1. Initialize global namespace
    // ----------------------------------------------------------
    /*
    webview.evaluate_script(
        "window.__TAURINO_INTERNALS__.runCallback(0, 30)"
    )?;
    */
    scripts.push(main_frame_script(
        r#"
        (() => {
            "use strict";

            const global = window;

            if (!Object.hasOwn(global, "__TAURINO_INTERNALS__")) {
                Object.defineProperty(global, "__TAURINO_INTERNALS__", {
                    value: Object.create(null),
                    writable: false,
                    configurable: false,
                    enumerable: false
                });
            }

            const internals = global.__TAURINO_INTERNALS__;

            if (!internals || typeof internals !== "object") {
                throw new TypeError("Invalid Taurino internals");
            }

            if (!Object.hasOwn(internals, "plugins")) {
                Object.defineProperty(internals, "plugins", {
                    value: Object.create(null),
                    writable: false,
                    configurable: false,
                    enumerable: true
                });
            }

            if (!Object.hasOwn(global, "isTaurino")) {
                Object.defineProperty(global, "isTaurino", {
                    value: true,
                    writable: false,
                    configurable: false,
                    enumerable: true
                });
            }
        })();
        "#,
    ));

    // ----------------------------------------------------------
    // 2. Initialize immutable metadata
    // ----------------------------------------------------------

    let os = std::env::consts::OS;
    let family = std::env::consts::FAMILY;
    let arch = std::env::consts::ARCH;
    let pid = std::process::id();

    let metadata = serde_json::json!({
        "currentWindow": {
            "label": window_label,
        },
        "currentWebview": {
            "label": webview_label,
        },
        "process": {
            "pid": pid,
        },
        "platform": {
            "os": os,
            "family": family,
            "arch": arch,
            "target": format!("{arch}-{os}"),
        },
    });

    let metadata_json = serde_json::to_string(&metadata)?;

    let metadata_script = r#"
        (() => {
            "use strict";

            const internals = window.__TAURINO_INTERNALS__;
            const metadata = __TAURINO_METADATA_JSON__;

            const deepFreeze = (value) => {
                if (value && typeof value === "object") {
                    for (const key of Object.keys(value)) {
                        deepFreeze(value[key]);
                    }
                    Object.freeze(value);
                }
                return value;
            };

            Object.defineProperty(internals, "metadata", {
                value: deepFreeze(metadata),
                writable: false,
                configurable: false,
                enumerable: true
            });
        })();
    "#
    .replace("__TAURINO_METADATA_JSON__", &metadata_json);

    scripts.push(main_frame_script(metadata_script));

    // ----------------------------------------------------------
    // 3. Initialize core APIs and IPC
    // ----------------------------------------------------------

    let scheme = if use_https_scheme { "https" } else { "http" };

    let core_script = r#"
        (() => {
            "use strict";

            const internals = window.__TAURINO_INTERNALS__;

            const osName = __TAURINO_OS_JSON__;
            const protocolScheme = __TAURINO_SCHEME_JSON__;

            // --------------------------------------------------
            // Callback registry
            // --------------------------------------------------

            const callbacks = new Map();
            let nextCallbackId = 0;

            function uid() {
                let id;

                do {
                    id = nextCallbackId;
                    nextCallbackId =
                        (nextCallbackId + 1) >>> 0;
                } while (callbacks.has(id));

                return id;
            }

            function unregisterCallback(id) {
                return callbacks.delete(id);
            }

            function registerCallback(callback, once = false) {
                if (typeof callback !== "function") {
                    throw new TypeError(
                        "Callback must be a function"
                    );
                }

                const id = uid();

                callbacks.set(id, (data) => {
                    if (once) {
                        unregisterCallback(id);
                    }

                    return callback(data);
                });

                return id;
            }

            function runCallback(id, data) {
                const callback = callbacks.get(id);

                if (callback) {
                    return callback(data);
                }

                console.warn(
                    `[TAURINO] Callback ${id} was not found`
                );
            }

            // --------------------------------------------------
            // File URL conversion
            // --------------------------------------------------

            function convertFileSrc(filePath, protocol = "asset") {
                if (typeof filePath !== "string") {
                    throw new TypeError(
                        "filePath must be a string"
                    );
                }

                const path = encodeURIComponent(filePath);

                if (osName === "windows" || osName === "android") {
                    return `${protocolScheme}://${protocol}.localhost/${path}`;
                }

                return `${protocol}://localhost/${path}`;
            }

            // --------------------------------------------------
            // IPC queue
            // --------------------------------------------------

            const ipcQueue = [];
            let isWaitingForIpc = false;

            function hasIpc() {
                return (
                    typeof internals.ipc === "function" &&
                    typeof window.ipc?.postMessage === "function"
                );
            }
            function flushIpcQueue() {
                if (!hasIpc()) {
                    return false;
                }

                isWaitingForIpc = false;

                while (ipcQueue.length > 0) {
                    const action = ipcQueue.shift();

                    try {
                        action();
                    } catch (error) {
                        console.error(
                            "[TAURINO] IPC action failed:",
                            error
                        );
                    }
                }

                return true;
            }

            function waitForIpc() {
                if (flushIpcQueue()) {
                    return;
                }

                setTimeout(waitForIpc, 50);
            }

            function enqueueIpc(action) {
                if (hasIpc() && ipcQueue.length === 0) {
                    action();
                    return;
                }

                ipcQueue.push(action);

                if (hasIpc()) {
                    flushIpcQueue();
                    return;
                }

                if (!isWaitingForIpc) {
                    isWaitingForIpc = true;
                    setTimeout(waitForIpc, 50);
                }
            }

            // --------------------------------------------------
            // Invoke API
            // --------------------------------------------------

            function listen(event, handler) {
                if (typeof event !== "string" || !event) {
                    throw new TypeError("Invalid event name");
                }

                if (typeof handler !== "function") {
                    throw new TypeError("Handler must be a function");
                }

                const eventType = `taurino:${event}`;

                const listener = (nativeEvent) => {
                    handler({
                        event,
                        payload: nativeEvent.detail
                    });
                };

                window.addEventListener(eventType, listener);

                // Unsubscribe function
                return () => {
                    window.removeEventListener(eventType, listener);
                };
            }

            function once(event, handler) {
                if (typeof event !== "string" || !event) {
                    throw new TypeError("Invalid event name");
                }

                if (typeof handler !== "function") {
                    throw new TypeError("Handler must be a function");
                }

                const eventType = `taurino:${event}`;

                const listener = (nativeEvent) => {
                    handler({
                        event,
                        payload: nativeEvent.detail
                    });
                };

                window.addEventListener(eventType, listener, {
                    once: true
                });

                return () => {
                    window.removeEventListener(eventType, listener);
                };
            }

            function invoke(cmd, payload = {}, options) {
                return new Promise((resolve, reject) => {
                    let successId;
                    let errorId;

                    successId = registerCallback((data) => {
                        unregisterCallback(errorId);
                        resolve(data);
                    }, true);

                    errorId = registerCallback((error) => {
                        unregisterCallback(successId);
                        reject(error);
                    }, true);

                    const action = () => {
                        try {
                            internals.ipc({
                                cmd,
                                callback: successId,
                                error: errorId,
                                payload,
                                options
                            });
                        } catch (error) {
                            unregisterCallback(successId);
                            unregisterCallback(errorId);
                            reject(error);
                        }
                    };

                    enqueueIpc(action);
                });
            }

            // --------------------------------------------------
            // Register internal APIs
            // --------------------------------------------------

            const defineApi = (name, value) => {
                Object.defineProperty(internals, name, {
                    value,
                    writable: false,
                    configurable: false,
                    enumerable: true
                });
            };

            defineApi("convertFileSrc", convertFileSrc);
            defineApi("transformCallback", registerCallback);
            defineApi("unregisterCallback", unregisterCallback);
            defineApi("runCallback", runCallback);
            defineApi("listen", listen);
            defineApi("once", once);
            defineApi("invoke", invoke);

            defineApi("ipc", (message) => {
                if (typeof window.ipc?.postMessage !== "function") {
                    throw new Error("Native IPC is not available");
                }

                window.ipc.postMessage(JSON.stringify(message));
            });

        })();
    "#
    .replace("__TAURINO_OS_JSON__", &serde_json::to_string(os)?)
    .replace("__TAURINO_SCHEME_JSON__", &serde_json::to_string(scheme)?);

    scripts.push(main_frame_script(core_script));

    Ok(scripts)
}

pub fn emit_to_webview(webview: &WebView, event: &str, payload: &JsonValue) -> Result<()> {
    let event_name = format!("taurino:{event}");

    let name_json = serde_json::to_string(&event_name)?;
    let payload_json = serde_json::to_string(payload)?;

    let script = format!(
        "window.dispatchEvent(new CustomEvent(
            {name_json},
            {{ detail: {payload_json} }}
        ));"
    );

    webview.evaluate_script(&script).context("Failed to dispatch WebView event")?;

    Ok(())
}

#[derive(Debug, Deserialize)]
pub struct WebviewNotification {
    webview_label: String,
    event: String,
    payload: JsonValue,
}

pub fn try_handle_notification(payload: &[u8], webview: &WebView) -> bool {
    let decoded = rmp_serde::from_slice::<(String, WebviewNotification)>(payload);

    let Ok((method, notification)) = decoded else {
        return false;
    };

    if method != "webview.emit" {
        return false;
    }

    if notification.webview_label != webview.label() {
        eprintln!("Unknown WebView: {}", notification.webview_label);
        return true;
    }

    if let Err(error) = emit_to_webview(webview, &notification.event, &notification.payload) {
        eprintln!("Failed to emit event: {error:#}");
    }

    true
}

pub struct PendingInvoke {
    pub window_label: String,
    pub webview_label: String,
    pub request: InvokeRequest,
}

pub fn allocate_request_id(pending: &HashMap<u16, PendingInvoke>, next_id: &mut u16) -> Option<u16> {
    for _ in 0..=u16::MAX {
        let id = *next_id;
        *next_id = next_id.wrapping_add(1);

        if !pending.contains_key(&id) {
            return Some(id);
        }
    }
    None
}

pub fn reply_to_js(webview: &WebView, request: &InvokeRequest, response: std::result::Result<InvokeResponse, String>) {
    if let Err(error) = resolve_invoke(webview, request, response) {
        eprintln!("Taurino JS callback failed: {error:#}");
    }
}
