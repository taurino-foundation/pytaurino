pub const HTML: &str = r#"<!DOCTYPE html>
<html lang="de">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Taurino - Command Playground</title>
    <style>
        * {
            box-sizing: border-box;
        }
        body {
            margin: 0;
            min-height: 100vh;
            background: #0e1117;
            color: #e6edf3;
            font-family: system-ui, sans-serif;
            padding: 32px;
        }
        .container {
            max-width: 760px;
            margin: 0 auto;
        }
        h1 {
            font-size: 28px;
            margin-bottom: 6px;
        }
        .subtitle {
            color: #8b949e;
            margin-bottom: 28px;
        }
        .card {
            background: #161b22;
            border: 1px solid #30363d;
            border-radius: 12px;
            padding: 22px;
            margin-bottom: 18px;
        }
        .status {
            display: flex;
            align-items: center;
            gap: 10px;
            font-size: 14px;
        }
        .dot {
            width: 10px;
            height: 10px;
            background: #f0883e;
            border-radius: 50%;
        }
        .dot.connected {
            background: #3fb950;
        }
        label {
            display: block;
            margin-bottom: 8px;
            color: #8b949e;
            font-size: 13px;
        }
        input, textarea {
            width: 100%;
            background: #0d1117;
            border: 1px solid #30363d;
            border-radius: 7px;
            padding: 12px;
            color: #e6edf3;
            font-family: monospace;
            font-size: 14px;
            outline: none;
        }
        input:focus, textarea:focus {
            border-color: #58a6ff;
        }
        textarea {
            min-height: 130px;
            resize: vertical;
        }
        .field {
            margin-bottom: 18px;
        }
        button {
            background: #238636;
            border: none;
            border-radius: 7px;
            padding: 11px 20px;
            color: white;
            font-size: 14px;
            font-weight: 600;
            cursor: pointer;
        }
        button:hover {
            background: #2ea043;
        }
        button:disabled {
            opacity: 0.5;
            cursor: not-allowed;
        }
        .secondary {
            background: #30363d;
        }
        .secondary:hover {
            background: #484f58;
        }
        .buttons {
            display: flex;
            gap: 10px;
            flex-wrap: wrap;
        }
        pre {
            background: #0d1117;
            border: 1px solid #30363d;
            padding: 16px;
            border-radius: 8px;
            overflow: auto;
            min-height: 90px;
            white-space: pre-wrap;
            overflow-wrap: anywhere;
            font-size: 13px;
            line-height: 1.6;
        }
        .success {
            color: #3fb950;
        }
        .error {
            color: #f85149;
        }
        .info {
            color: #58a6ff;
        }
        .hint {
            margin: 10px 0 16px;
            color: #8b949e;
            font-size: 13px;
            line-height: 1.5;
        }
        .event-count {
            color: #8b949e;
            font-size: 13px;
            margin-top: 14px;
        }
        .code-example {
            min-height: 0;
            margin: 12px 0 0;
            color: #c9d1d9;
        }
        .event-log {
            min-height: 130px;
            max-height: 280px;
            overflow-y: auto;
        }
    </style>
</head>
<body>
    <main class="container">
        <h1>Taurino</h1>
        <p class="subtitle">
            JavaScript ↔ Rust ↔ Python Command Playground
        </p>
        <section class="card">
            <div class="status">
                <span class="dot" id="status-dot"></span>
                <span id="status-text">Checking Taurino API...</span>
            </div>
        </section>
        <section class="card">
            <h3>Invoke Command</h3>
            <div class="field">
                <label for="command">Command</label>
                <input
                    id="command"
                    value="greet"
                    spellcheck="false"
                >
            </div>
            <div class="field">
                <label for="payload">Payload (JSON)</label>
                <textarea id="payload" spellcheck="false">{
    "name": "Taurino"
}</textarea>
            </div>
            <div class="buttons">
                <button id="invoke-btn">
                    Invoke
                </button>
                <button id="greet-btn" class="secondary">
                    Test Greet
                </button>
                <button id="clear-btn" class="secondary">
                    Clear
                </button>
            </div>
        </section>
        <section class="card">
            <h3>Response</h3>
            <pre id="response">No response yet.</pre>
        </section>
        <section class="card">
            <h3>Python -> JavaScript Events</h3>
            <p class="hint">
                Beim Laden werden <code>listen("status")</code> und
                <code>once("ready")</code> automatisch registriert.
                Python kann danach Events an diese WebView senden.
            </p>
            <div class="buttons">
                <button id="listen-start-btn">Listen status</button>
                <button id="listen-stop-btn" class="secondary">Unlisten status</button>
                <button id="once-start-btn" class="secondary">Arm once ready</button>
                <button id="events-clear-btn" class="secondary">Clear events</button>
            </div>
            <p class="event-count" id="listener-state">Listeners not initialized</p>
            <p class="event-count" id="event-count">Events received: 0</p>
            <pre class="event-log" id="event-log">No events received yet.</pre>
            <label>Python: nach dem Registrieren der JavaScript-Listener ausfuehren</label>
            <pre class="code-example">await app.emit("status", {"message": "Hallo von Python"})
await app.emit("status", {"percent": 50})
await app.emit("ready", {"success": True})
await app.emit("ready", {"success": True})  # once ignores this</pre>
        </section>
        <section class="card">
            <h3>Runtime Metadata</h3>
            <pre id="metadata">Loading...</pre>
        </section>
    </main>
    <script>
        (() => {
            "use strict";
            const $ = (id) => document.getElementById(id);
            const internals =
                window.__TAURINO_INTERNALS__;
            const invokeButton = $("invoke-btn");
            const responseElement = $("response");
            const eventLogElement = $("event-log");
            const eventHistory = [];
            let eventCount = 0;
            let unlistenStatus = null;
            let unlistenReady = null;

            function addEventLog(kind, value) {
                const entry = {
                    time: new Date().toLocaleTimeString(),
                    kind,
                    value
                };
                eventHistory.push(entry);
                if (eventHistory.length > 50) eventHistory.shift();
                eventLogElement.textContent = eventHistory
                    .map((item) =>
                        `[${item.time}] ${item.kind}: ${format(item.value)}`
                    )
                    .join("\n\n");
                eventLogElement.scrollTop = eventLogElement.scrollHeight;
                if (kind === "status" || kind === "ready") {
                    eventCount++;
                    $("event-count").textContent = `Events received: ${eventCount}`;
                }
            }

            function updateListenerControls() {
                const available =
                    typeof internals?.listen === "function" &&
                    typeof internals?.once === "function";
                $("listen-start-btn").disabled = !available || !!unlistenStatus;
                $("listen-stop-btn").disabled = !available || !unlistenStatus;
                $("once-start-btn").disabled = !available || !!unlistenReady;
                $("listener-state").textContent = available
                    ? `status: ${unlistenStatus ? "listening" : "inactive"} | ` +
                      `ready: ${unlistenReady ? "armed (once)" : "inactive"}`
                    : "Taurino Event API unavailable";
            }

            function startStatusListener() {
                if (unlistenStatus || typeof internals?.listen !== "function") return;
                unlistenStatus = internals.listen("status", ({ payload }) => {
                    addEventLog("status", payload);
                    console.log("[Taurino] status event:", payload);
                });
                updateListenerControls();
            }

            function stopStatusListener() {
                if (unlistenStatus) {
                    unlistenStatus();
                    unlistenStatus = null;
                }
                updateListenerControls();
            }

            function armReadyOnce() {
                if (unlistenReady || typeof internals?.once !== "function") return;
                unlistenReady = internals.once("ready", ({ payload }) => {
                    // The native once listener is removed before this handler returns.
                    unlistenReady = null;
                    addEventLog("ready", payload);
                    console.log("[Taurino] ready event (once):", payload);
                    updateListenerControls();
                });
                updateListenerControls();
            }
            function format(value) {
                if (typeof value === "string") {
                    return value;
                }
                return JSON.stringify(value, null, 2);
            }
            function showResponse(type, value) {
                responseElement.className = type;
                responseElement.textContent = format(value);
            }
            function updateStatus() {
                const available =
                    typeof internals?.invoke === "function" &&
                    typeof window.ipc?.postMessage === "function";
                $("status-dot").classList.toggle(
                    "connected",
                    available
                );
                $("status-text").textContent = available
                    ? "Taurino API available"
                    : "Taurino API unavailable";
                invokeButton.disabled = !available;
                $("greet-btn").disabled = !available;
            }
            async function executeCommand(command, payload = {}) {
                if (typeof internals?.invoke !== "function") {
                    showResponse("error", {
                        message: "Taurino invoke API unavailable"
                    });
                    return;
                }
                invokeButton.disabled = true;
                showResponse("info", {
                    status: "pending",
                    command,
                    payload
                });
                const start = performance.now();
                try {
                    const result = await internals.invoke(
                        command,
                        payload
                    );
                    showResponse("success", {
                        status: "success",
                        command,
                        durationMs: Number(
                            (performance.now() - start).toFixed(2)
                        ),
                        data: result
                    });
                    console.log(
                        "[Taurino] Command result:",
                        result
                    );
                } catch (error) {
                    showResponse("error", {
                        status: "error",
                        command,
                        durationMs: Number(
                            (performance.now() - start).toFixed(2)
                        ),
                        error
                    });
                    console.error(
                        "[Taurino] Command failed:",
                        error
                    );
                } finally {
                    updateStatus();
                }
            }
            invokeButton.addEventListener("click", () => {
                const command = $("command").value.trim();
                if (!command) {
                    showResponse("error", {
                        message: "Command name is required"
                    });
                    return;
                }
                let payload;
                try {
                    payload = JSON.parse($("payload").value);
                    if (
                        payload === null ||
                        typeof payload !== "object" ||
                        Array.isArray(payload)
                    ) {
                        throw new Error(
                            "Payload must be a JSON object"
                        );
                    }
                } catch (error) {
                    showResponse("error", {
                        message: "Invalid JSON payload",
                        details: String(error)
                    });
                    return;
                }
                executeCommand(command, payload);
            });
            $("greet-btn").addEventListener("click", () => {
                executeCommand("greet", {
                    name: "Taurino"
                });
            });
            $("clear-btn").addEventListener("click", () => {
                showResponse("", "No response yet.");
            });
            $("listen-start-btn").addEventListener("click", startStatusListener);
            $("listen-stop-btn").addEventListener("click", stopStatusListener);
            $("once-start-btn").addEventListener("click", armReadyOnce);
            $("events-clear-btn").addEventListener("click", () => {
                eventHistory.length = 0;
                eventCount = 0;
                eventLogElement.textContent = "No events received yet.";
                $("event-count").textContent = "Events received: 0";
            });

            // Listen immediately. Python must send only after the page has loaded.
            startStatusListener();
            armReadyOnce();

            // Metadata provided by Rust initialization scripts.
            $("metadata").textContent = format(
                internals?.metadata ?? {
                    available: false
                }
            );
            updateStatus();
        })();
    </script>
</body>
</html>
"#;
