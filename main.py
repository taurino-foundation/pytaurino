import asyncio
import sys
from typing import Any

from taurino import App, AppHandle, Builder, ConfigBuilder

builder = Builder()


@builder.setup()
def setup(app: App) -> None:
    config = ConfigBuilder.default(
        product_name="Taurino",
        version="0.1.0",
        webview_url="about:blank",
    ).build()

    app.configure(config)
    app.state.set({"initialized": True})


@builder.command("greet")
async def greet(name: str, handle: AppHandle) -> str:
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