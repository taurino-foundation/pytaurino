"""Restricted public view of a running application."""

from __future__ import annotations

import asyncio
from typing import TYPE_CHECKING, Any, TypeVar

from taurino.error import AppLifecycleError

if TYPE_CHECKING:
    from taurino.app import App
    from taurino.proto import AppConfig
    from taurino.state import State


T = TypeVar("T")


class AppHandle:
    """Restricted public interface to the application runtime."""

    def __init__(self, app: App) -> None:
        self._app = app

    @property
    def config(self) -> AppConfig:
        """Return the validated application configuration."""
        return self._app._require_config()

    @property
    def state(self) -> State:
        """Return the application state manager."""
        return self._app.state

    @property
    def is_connected(self) -> bool:
        """Whether an active Rust IPC session exists."""
        session = self._app._primary_session
        return session is not None and not session.peer.closed

    @property
    def is_running(self) -> bool:
        """Whether the application is running."""
        return self._app.is_running

    @property
    def engine_pid(self) -> int | None:
        """Return the Rust engine process ID."""
        return self._app.engine_pid

    async def wait_connected(
        self,
        *,
        timeout: float | None = None,
    ) -> None:
        """Wait until the Rust engine establishes an IPC connection."""
        if self._app._stopped:
            raise AppLifecycleError("Application is stopped")

        if self.is_connected:
            return

        async with asyncio.timeout(timeout):
            await self._app._connected.wait()

        if not self.is_connected:
            raise ConnectionError("Rust engine disconnected")

    async def call(
        self,
        method: str,
        *args: Any,
        result_type: type[T],
        timeout: float | None = None,
    ) -> T:
        """Call the Rust engine through the internal Multiplexer."""
        session = self._app._primary_session

        if session is None or session.peer.closed:
            raise ConnectionError("Rust engine not connected")

        return await session.multiplexer.call(
            method,
            *args,
            result_type=result_type,
            timeout=timeout,
        )

    async def emit(
        self,
        event: str,
        payload: Any = None,
        *,
        webview_label: str = "root",
    ) -> None:
        """Emit an event to a WebView."""
        await self._app.emit(
            event=event,
            payload=payload,
            webview_label=webview_label,
        )

    async def stop(self) -> None:
        """Request graceful application shutdown."""
        await self._app.stop()
