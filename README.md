# **Quellcode direkt in `taurino/*.py`**

Siehe [`START_HIER.md`](START_HIER.md) fuer den Dateiindex und eine deutsche Schnellstart-Anleitung.

# Taurino — Python architecture starter

A modular Python 3.11+ project based on the provided Taurino architecture and source snippets.

**Status:** The Python architecture, validation, typed state, process supervision, RPC
multiplexing, command dispatch, and Taunicorn IPC server implementation are provided.
The **compiled Rust / PyO3 / Tao / Wry GUI engine is not included** in the uploaded
materials. The default `native_gui_target` therefore requires a separately built
`taurino.engine.runtime` extension with the contract documented below. A Python
mock engine is provided for testing the protocol, not as a GUI replacement.

## Project layout

```text
Taurino/
  pyproject.toml
  README.md
  docs/
    ARCHITECTURE.md
    RUST_BRIDGE.md
    USER_SOURCE.md               # unchanged original source paste
  taurino/
    __init__.py                  # public API
    config.py                    # Pydantic v2 configuration
    builder.py                   # public Builder and decorators
    app.py                       # lifecycle and IPC dispatch
    app_handle.py                # controlled public API
    state.py                     # typed, thread-safe State
    error.py
    _internal/
      command.py                 # CommandRegistry / DI
      connection.py              # IpcServer / IpcPeer using Taunicorn
      multiplexer.py             # async RPC and MessagePack
      runtime.py                 # multiprocessing spawn process supervisor
      native.py                  # required PyO3 adapter signature
  examples/
    demo_engine.py               # *mock*, no native GUI
    quickstart.py
  tests/
    test_core.py
    test_runtime.py
    test_integration.py
```

## Installation

From the extracted `Taurino-Quellcode` directory:

```powershell
py -3.11 -m venv .venv
.venv\Scripts\Activate.ps1
python -m pip install -e ".[dev]"
python -m pytest -q
python examples/quickstart.py
```

Linux/macOS: `python3 -m venv .venv && source .venv/bin/activate`.

`taunicorn` provides a native Rust-backed local IPC transport. The Python environment
used to create this archive did **not** have `taunicorn`, and access to the package
index was unavailable. The included unit tests were executed; the Taunicorn
integration test is conditional on installation and has not been run here.

## Minimal application

```python
import asyncio
from uuid import uuid4
from taurino import Builder, Config, IpcConnectionConfig, App, AppHandle

builder = Builder(Config(
    productname="My App",
    version="0.1.0",
    app_id=uuid4(),
    connection=IpcConnectionConfig(endpoint=f"taurino-{uuid4().hex}"),
))

@builder.setup()
def setup(app: App) -> None:
    # create State and optionally call app.configure(new_config)
    app.state.set({"initialized": True})

@builder.command("greet")
async def greet(name: str, handle: AppHandle) -> str:
    return f"Hello, {name}!"

@builder.on_event()  # alternatively @builder.run()
async def event_loop_hook(handle: AppHandle, event: dict) -> None:
    print(event)

async def main() -> None:
    app = builder.build()  # default requires taurino.engine.runtime (compiled PyO3)
    async with app:
        await app.handle.call("engine.ping", result_type=str)

if __name__ == "__main__":
    asyncio.run(main())
```

For a **runnable protocol example without Rust**, see `examples/quickstart.py`.

## Architectural boundaries

- **Public:** `Builder`, `App`, `AppHandle`, `Config` and config models, `State`.
- **Internal:** `Runtime`, `IpcServer`, `IpcPeer`, `CommandRegistry`, `Multiplexer`.
- `AppHandle.call(...)` provides controlled access to the currently active RPC
  session; no `Multiplexer` object is directly exposed.
- A `CommandRegistry` handler may declare a typed `AppHandle` parameter. It is
  injected internally.
- **Explicit dependency injection:** for a service use
  `Annotated[MyService, Depends()]` and register it internally using
  `registry.container.register_singleton(...)` or `register_factory(...)`. The
  original idea of auto-injecting *all* user-defined classes is intentionally not
  used, because Pydantic models and other data types can be mistaken for services.
- The `IpcServer` is transport-only: accepts clients and sends/receives bytes.
  `App` handles message classification and dispatch. `CommandRegistry` and
  `Multiplexer` have no direct dependency on each other.
- Synchronous command handlers and lifecycle hooks are run in a worker thread to
  avoid blocking the Python IPC `asyncio` event loop. Long-running handlers
  should remain asynchronous or use explicit worker/process pools.

## Lifecycle

1. `Builder` collects config, commands, `setup`, and event-loop hooks.
2. `Builder.build()` creates `App` / `AppHandle` / `State` / registry.
3. `App.start()` invokes the `setup` hook. A replacement `Config` can be applied
   **only here**, via `app.configure(new_config)`.
4. `IpcServer.start()` binds the local IPC endpoint. The accept loop starts.
5. `Runtime.start()` spawns a Python child running the Rust/PyO3 GUI entry point on
   the **child main thread**. Pydantic app config is serialized to MessagePack.
6. The Rust engine connects to the Python server as an IPC client.
7. Once the Rust engine confirms real readiness via `notify_ready()`, startup
   completes. The engine owns the Tao event loop and Wry webviews.
8. `App.stop()` stops the GUI child and closes server, peers and pending RPCs.

**Windows:** Always start an app under `if __name__ == "__main__":` because
`multiprocessing` uses the `spawn` start method.

## Data flow and protocol

The transport uses Taunicorn local IPC (not TCP). In unauthenticated mode it uses
**4-byte big-endian unsigned payload length** followed by MessagePack bytes; in
authenticated mode Taunicorn `SecureConnection` provides message boundaries,
authentication and encryption without the extra 4-byte application prefix.

| Direction | MessagePack array | Purpose |
|---|---|---|
| Python → Rust | `[u16_id, method, [args...]]` | async `Multiplexer` request |
| Rust → Python | `[u16_id, i32_code, message, data]` | response to pending Python request |
| Rust → Python | `[u16_id, "taurino.command", [name, result_id, error_id, data]]` | dispatch registered Python command |
| Rust → Python | `[u16_id, "taurino.event", [event]]` | invoke Python event-loop hook |
| Python → Rust | `[u16_id, i32_code, message, data]` | result of a Rust-originated request |

`data` for `taurino.command` is the existing JS callback contract:
`{"result_id": N, "result": ...}` or
`{"error_id": N, "error": "...", "error_obj": {...}}`.
Rust remains responsible for delivering that value to the originating WebView;
the Python framework does **not** evaluate JavaScript itself.

## Security considerations

- `security="none"` is for **trusted local** development. Local transport alone
  is not peer authentication. For production, choose `IpcSecurity.AUTHENTICATED`
  with separate, pinned Ed25519 identities and appropriate OS endpoint ACLs.
- The **server private key is never included** in the Rust engine's MessagePack
  bootstrap config. The client receives a server public key and a *path* to its
  own private identity key, which must be accessible to the child process.
- Request sizes and concurrency are bounded; RPC IDs are `u16` and are never
  reused within one session. Reconnect before 65,536 outgoing requests.
- This is a development scaffold, **not a security audit of the Rust engine**.
  Restrict exposed browser APIs; validate commands and enforce WebView origin,
  scopes/capabilities and permissions on the trusted Rust side.

## Compatibility notes

- Python config uses Pydantic v2 with `camelCase` aliases for snake_case fields,
  such as `appId`, `schemaVersion`, `webviews`, and `minWidth`. Preserve the same
  field names and version in the Rust `serde` models.
- The pasted source used `@builder.run` to register the event hook; this project
  retains it as an alias for `@builder.on_event`. To launch the app, use
  `await app.start()` or `await app.run_forever()`.
- The pasted code was included verbatim in `docs/USER_SOURCE.md` as an unmodified
  design reference. The executable modules are an integrated implementation,
  not a promise of byte-for-byte compatibility with those independent snippets.
