# Taurino Serde -> Pydantic v2

Pydantic models based on the supplied Rust source (`app`, `mix`, `webview`, `window`, `lib`, `protocol`, `paths`). This package does not alter the Rust source.

## Dependencies

```powershell
uv add pydantic msgpack
uv add --dev pytest
```

## Example

```python
from uuid import uuid4
from config import AppConfig, WindowConfig, WebViewConfig, IpcClientConfig

config = AppConfig(
    product_name="Taurino",
    version="0.1.0",
    app_id=uuid4(),
    schema_version=1,
    connection=IpcClientConfig(
        endpoint="127.0.0.1:8999",  # replace with actual IPC endpoint
        connect_timeout=5.0,
        operation_timeout=30.0,
        handshake_timeout=5.0,
        max_message_size=1024 * 1024,
        security="none",
    ),
    windows=[WindowConfig(label="main", webviews=[WebViewConfig(label="root", url="index.html")])],
)

payload = config.to_engine_bytes()
print(config.to_wire()["productName"])
```

Use `uv run python -m examples.basic` instead of global `python` to execute against the installed project environment.

## Included

- `AppConfig` including UUID, schema version, SemVer, window/webview label uniqueness and 8 MiB payload bound.
- `IpcClientConfig`, `IpcSecurity` with security and key-length checks. `serverPublicKey` is MessagePack binary (`serde_bytes::ByteBuf`).
- `ShortcutConfig`, `MenuItemType`, `MenuItemConfig`, `MenuConfig`, `TrayIconConfig`.
- `WindowConfig`, `WebViewConfig` and associated navigation, new-window, window-effects, theme, color and other enum configuration.
- `InvokeRequest`, `InvokeResponse`, `WebviewNotification` and `BaseDirectory` numeric enum.
- `snake_case` Python properties and Rust `camelCase` wire keys, including the supplied hyphenated Serde aliases.

## Important limitations and details

1. Tests cover **Python -> MessagePack -> Python**, not direct execution of the Rust deserializer. Validate interoperability in the real Taurino runtime before production use.
2. External `dpi::Position`, `dpi::Size`, `dpi::PixelUnit`, and logical/physical coordinates are partially represented as dictionaries or permissive values because the exact version-specific Serde representation is not in the supplied source. Further Rust-side validation is required for those fields.
3. Rust `impl Default` is not the same as Serde's **missing-field default**. The models now use Serde's missing-field behavior: `WebViewConfig.auto_resize=False`, `WebViewConfig.enable_clipboard_access=True`, `WindowConfig.enable_drag_drop=True`. Rust's explicit `Default` implementations use different values for these three fields. Set them explicitly when you want the Rust `Default` behavior.
4. Some Rust types such as `Config` (resolved engine paths), `WebContext` and event loop callbacks are runtime-only, not MessagePack bootstrap configuration and are intentionally not modeled as engine config input.
5. `AppConfig.schema_version` defaults to 1 for Python convenience, but its presence on the wire is mandatory for Rust. `WindowConfig.webviews` is also emitted even when empty.
6. For safe relative WebView data directories, validate platform-specific filesystem path semantics in Rust as well. Python `str` alone is not a substitute for native Windows path canonicalization.
7. `serverPublicKey` is omitted if `None`, and the authenticated variant checks for 32 bytes and a client-private-key path; the Python model does not load or disclose that private key.
8. `MenuItemConfig.accelerator` is optional and the recursive `children` list defaults to empty.
9. The older `FrontendDist` documentation is commented out in your Rust source and is **not** an active field in `AppConfig`; do not send `frontendDist` in the current bootstrap.

## Test

```powershell
uv run pytest -q test_config.py
```
