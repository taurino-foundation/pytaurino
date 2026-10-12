"""Restricted public view of a running application."""

from __future__ import annotations

import asyncio
from typing import TYPE_CHECKING, Any, TypeVar

from taurino.error import AppLifecycleError

if TYPE_CHECKING:
    from taurino.app import App
    from taurino.config import Config
    from taurino.state import State

T = TypeVar("T")


class AppHandle:
    def __init__(self, app: App) -> None:
        self._app = app

    @property
    def config(self) -> Config:
        return self._app.config.build()

    @property
    def state(self) -> State:
        return self._app.state

    @property
    def is_connected(self) -> bool:
        return self._app._primary_session is not None

    async def wait_connected(self, *, timeout: float | None = None) -> None:
        if self._app._stopped:
            raise AppLifecycleError("Application is stopped")
        if self.is_connected:
            return
        async with asyncio.timeout(timeout):
            await self._app._connected.wait()
        if not self.is_connected:
            raise ConnectionError("Engine disconnected")

    async def call(
        self,
        method: str,
        *args: Any,
        result_type: type[T],
        timeout: float | None = None,
    ) -> T:
        """Use the internal per-session Multiplexer without publishing it."""
        session = self._app._primary_session
        if session is None:
            raise ConnectionError("Rust engine not connected")
        return await session.multiplexer.call(
            method, *args, result_type=result_type, timeout=timeout
        )

    async def stop(self) -> None:
        await self._app.stop()

    async def emit(
        self, event: str, payload: Any = None, *, webview_label: str = "webview-main"
    ) -> None:
        await self._app.emit(event=event, payload=payload, webview_label=webview_label)

    # The registry and multiplexer are deliberately *not* public properties.
    # Internal framework methods can reach app._registry and app._primary_session.
