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


@builder.setup()
def setup(app: App) -> None:
    (
        app.configure()
        .product_name("Taurino")
        .version("0.1.0")
        .app_id(uuid4())
        .connection(IpcClientConfig())
        .window(
            WindowBuilder("main")
            .title("Taurino Application")
            .size(1200, 800)
            .center(True)
            .resizable(True)
            .visible(True)
            .webview(WebViewBuilder("root").url("about:blank").auto_resize(True).devtools(True))
        )
    )

    app.state.set({"initialized": True})


@builder.command("greet")
async def greet(name: str, handle: AppHandle) -> str:
    await handle.emit(
        "status",
        {"message": "Hallo JavaScript!"},
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

    return f"Hello, {name}!"


@builder.on_event()
async def event_loop(
    handle: AppHandle,
    event: dict[str, Any],
) -> None:
    match event.get("name"):
        case "window":
            print(event.get("payload"))


async def main() -> None:
    app = builder.build()
    await app.run()


if __name__ == "__main__":
    try:
        asyncio.run(main())
    except KeyboardInterrupt:
        pass
