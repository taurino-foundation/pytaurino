import asyncio
from typing import Any
from uuid import uuid4

from taurino import (
    App,
    AppHandle,
    Builder,
    IpcClientConfig,
    WebViewBuilder,
    WindowBuilder,
)

builder = Builder()


def setup_app(app: App) -> None:
    (
        app.configure()
        .product_name("Taurino")
        .version("0.1.0")
        .app_id(uuid4())
        .connection(IpcClientConfig())
        .window(
            WindowBuilder("main")
            .title("Taurino Application")
            .size(800, 600)
            .center(True)
            .resizable(True)
            .visible(True)
            .webview(WebViewBuilder("root").url("about:blank").auto_resize(True).devtools(True))
        )
    )

    app.state.set({"initialized": True})


@builder.command("greet")
async def greet(name: str, handle: AppHandle) -> str:
    state = handle.state.get(dict)["initialized"]

    await handle.emit(
        "status",
        {"message": f"Hallo JavaScript! with the state {state}"},
        webview_label="root",
    )

    await handle.emit(
        "status",
        {"percent": 50},
        webview_label="root",
    )

    await handle.emit(
        "ready",
        {"success": True},
        webview_label="root",
    )

    return f"Hello, {name}! with app state:{state}"


# Tao EventLoop
async def on_loop_event(
    handle: AppHandle,
    event: dict[str, Any],
) -> None:
    print("Loop event:", event)


# Tao WindowEvent
async def on_window_event(
    handle: AppHandle,
    event: dict[str, Any],
) -> None:
    match event.get("name"):
        case "resized":
            print("Window resized:", event)

        case "focused":
            print("Window focused:", event)

        case "moved":
            print("Window moved:", event)

        case "close_requested":
            print("Window close requested:", event)


# Wry WebView Events
async def on_webview_event(
    handle: AppHandle,
    event: dict[str, Any],
) -> None:
    print("WebView event:", event)


async def main() -> None:
    app = (
        builder.setup(setup_app)
        .on_loop_event(on_loop_event)
        .on_window_event(on_window_event)
        .on_webview_event(on_webview_event)
        .build()
    )

    await app.run()


if __name__ == "__main__":
    try:
        asyncio.run(main())
    except KeyboardInterrupt:
        pass
