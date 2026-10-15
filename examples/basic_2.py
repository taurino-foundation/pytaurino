import asyncio
import multiprocessing as mp
from dataclasses import dataclass
from multiprocessing.managers import DictProxy, SyncManager
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


@dataclass(slots=True)
class SharedResources:
    manager: SyncManager | None = None
    data: DictProxy | None = None

    def start(self) -> None:
        if self.manager is not None:
            raise RuntimeError("Multiprocessing Manager already started")

        manager = mp.get_context("spawn").Manager()

        try:
            data = manager.dict({"initialized": True, "counter": 0})
        except BaseException:
            manager.shutdown()
            raise

        self.manager = manager
        self.data = data

    def shutdown(self) -> None:
        manager = self.manager

        if manager is None:
            return

        try:
            manager.shutdown()
        finally:
            self.manager = None
            self.data = None


builder = Builder()
resources = SharedResources()


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

    # Create the multiprocessing manager before starting Rust.
    resources.start()

    # Make the shared resources available to commands.
    app.state.set(resources)


@builder.command("greet")
async def greet(name: str, handle: AppHandle) -> str:
    state = handle.state.get(SharedResources)

    if state.data is None:
        raise RuntimeError("Shared state is not initialized")

    initialized = await asyncio.to_thread(state.data.get, "initialized")

    await handle.emit(
        "status",
        {"message": f"Initialized: {initialized}"},
        webview_label="root",
    )

    return f"Hello, {name}! Initialized: {initialized}"


async def on_window_event(
    handle: AppHandle,
    event: dict[str, Any],
) -> None:
    print("Window event:", event)


async def main() -> None:
    app = builder.setup(setup_app).on_window_event(on_window_event).build()

    try:
        await app.run()
    finally:
        await asyncio.to_thread(resources.shutdown)


if __name__ == "__main__":
    mp.freeze_support()
    asyncio.run(main())
