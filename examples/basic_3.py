"""Taurino application with the original multiprocessing measurement worker."""

from __future__ import annotations

import asyncio
import multiprocessing as mp
from typing import Any, Literal
from uuid import uuid4

from examples.machine import S_DATA, SM, classMultiPros, imp_data
from taurino import (
    App,
    AppHandle,
    Builder,
    IpcClientConfig,
    WebViewBuilder,
    WindowBuilder,
)

builder = Builder()

# Process resources are owned by the Python application.
manager: Any = None
shared_list: Any = None
p1: Any = None
stop_event: Any = None


def setup_app(app: App) -> None:
    global manager, shared_list, p1, stop_event

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

    # Original multiprocessing initialization.
    context = mp.get_context("spawn")
    manager = context.Manager()

    try:
        imp_data2 = imp_data()
        data = imp_data2.server_data()

        shared_list = manager.list(data.tolist())

        MultPros = classMultiPros()
        stop_event = context.Event()

        p1 = context.Process(
            target=MultPros.th,
            args=(shared_list, stop_event),
            name="measurement-worker",
        )

        p1.start()

        # Make the shared list available to Taurino commands.
        app.state.set(
            {
                "initialized": True,
                "shared_list": shared_list,
            }
        )

    except BaseException:
        stop_worker()
        raise


def stop_worker() -> None:
    global manager, shared_list, p1, stop_event

    try:
        if stop_event is not None:
            stop_event.set()

        if p1 is not None:
            p1.join(timeout=5)

            if p1.is_alive():
                p1.terminate()
                p1.join(timeout=2)

            if p1.is_alive():
                p1.kill()
                p1.join(timeout=2)

            if not p1.is_alive():
                p1.close()

        if manager is not None:
            manager.shutdown()

    finally:
        manager = None
        shared_list = None
        p1 = None
        stop_event = None


@builder.command("get_measurement")
async def get_measurement(handle: AppHandle) -> dict[str, Any]:
    data = handle.state.get(dict)["shared_list"]

    def read() -> dict[str, Any]:
        values = list(data)

        result = {field.name: values[field.value] for field in S_DATA}

        result["STATE_NAME"] = SM(int(values[S_DATA.STATE_MACHINE.value])).name

        return result

    return await asyncio.to_thread(read)


@builder.command("start_measurement")
async def start_measurement(handle: AppHandle) -> bool:
    data = handle.state.get(dict)["shared_list"]

    def start() -> bool:
        if int(data[S_DATA.STATE_MACHINE.value]) != SM.READY.value:
            return False

        data[S_DATA.STATE_MACHINE.value] = SM.RUNNING.value
        return True

    return await asyncio.to_thread(start)


@builder.command("set_machine_state")
async def set_machine_state(
    action: Literal["pause", "resume", "stop", "cancel"],
    handle: AppHandle,
) -> bool:
    data = handle.state.get(dict)["shared_list"]

    def change() -> bool:
        current = SM(int(data[S_DATA.STATE_MACHINE.value]))

        transitions = {
            "pause": (SM.MEASURING, SM.PAUSE),
            "resume": (SM.PAUSE, SM.MEASURING),
            "stop": (SM.MEASURING, SM.STOP),
            "cancel": (SM.MEASURING, SM.CANCEL),
        }

        expected, target = transitions[action]

        if current != expected:
            return False

        data[S_DATA.STATE_MACHINE.value] = target.value
        return True

    return await asyncio.to_thread(change)


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
        await asyncio.to_thread(stop_worker)


if __name__ == "__main__":
    mp.freeze_support()

    try:
        asyncio.run(main())
    except KeyboardInterrupt:
        pass
