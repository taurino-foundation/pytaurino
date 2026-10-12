"""Spawn-based GUI child lifecycle for Tao, Wry, and PyO3.

The Rust/PyO3 runtime runs on the child process's main thread.
The parent controls startup readiness and graceful shutdown.
"""

from __future__ import annotations

import multiprocessing as mp
import time
import traceback
from collections.abc import Callable
from multiprocessing.context import BaseContext
from multiprocessing.process import BaseProcess
from typing import Any, Literal, Protocol, cast

from taurino.core import runtime
from taurino.error import RuntimeStartError, RuntimeStateError


class StatusConnection(Protocol):
    """Minimal multiprocessing status pipe interface.

    Compatible with Windows PipeConnection and POSIX Connection.
    """

    def send(self, obj: Any) -> None: ...

    def recv(self) -> Any: ...

    def poll(self, timeout: float = 0.0) -> bool: ...

    def close(self) -> None: ...


ProcessFactory = Callable[..., BaseProcess]


def _gui_process_entry(
    config: bytes,
    shutdown_event: Any,
    status: StatusConnection,
) -> None:
    """Child entry point, importable with multiprocessing spawn."""
    ready_sent = False

    def notify_ready() -> None:
        nonlocal ready_sent

        if not ready_sent:
            status.send(("ready", None))
            ready_sent = True

    try:
        # Runs on the child process's main thread.
        runtime(config, shutdown_event, notify_ready)

    except KeyboardInterrupt:
        print("Keyboard interrupt detected. Exiting...")
        shutdown_event.set()

    except BaseException:
        if not ready_sent:
            try:
                status.send(("error", traceback.format_exc()))
            except (OSError, BrokenPipeError, EOFError):
                pass
        raise

    finally:
        status.close()


class Runtime:
    """Internal GUI child process controller.

    The native Rust binding runs on the child's main thread.

    Startup:
        1. Spawn child process.
        2. Initialize native GUI runtime.
        3. Receive readiness notification.

    Shutdown:
        1. Signal shutdown_event.
        2. Wait for graceful exit.
        3. Terminate if necessary.
        4. Kill as final fallback.
    """

    def __init__(
        self,
        config: bytes,
        *,
        name: str = "taurino-gui",
        start_timeout: float = 15.0,
        stop_timeout: float = 5.0,
        method: Literal["spawn", "fork"] = "spawn",
    ) -> None:
        if not isinstance(config, bytes):
            raise TypeError("GUI configuration must be bytes")

        if start_timeout <= 0 or stop_timeout < 0:
            raise ValueError("Invalid runtime timeout")

        self._config = config
        self._name = name
        self._start_timeout = start_timeout
        self._stop_timeout = stop_timeout

        self._context: BaseContext = mp.get_context(method)
        self._process: BaseProcess | None = None
        self._shutdown_event: Any = self._context.Event()
        self._status_connection: StatusConnection | None = None

    @property
    def pid(self) -> int | None:
        return self._process.pid if self._process is not None else None

    @property
    def exitcode(self) -> int | None:
        return self._process.exitcode if self._process is not None else None

    @property
    def is_running(self) -> bool:
        return self._process is not None and self._process.is_alive()

    @property
    def is_stopping(self) -> bool:
        return self.is_running and self._shutdown_event.is_set()

    def start(self) -> None:
        """Start the native GUI process and wait until it is ready."""

        if self.is_running:
            raise RuntimeStateError("GUI runtime is already running")

        # Clean up resources from any previous execution.
        self.close()

        self._shutdown_event = self._context.Event()

        # Parent receives; child sends.
        parent, child = self._context.Pipe(duplex=False)

        # Resolve the context-specific process factory.
        # Some typing stubs do not declare BaseContext.Process.
        process_factory = cast(
            ProcessFactory,
            getattr(self._context, "Process"),  # noqa: B009
        )

        try:
            process = process_factory(
                name=self._name,
                target=_gui_process_entry,
                args=(
                    self._config,
                    self._shutdown_event,
                    child,
                ),
                daemon=False,
            )
        except BaseException:
            parent.close()
            child.close()
            raise

        self._process = process
        self._status_connection = cast(
            StatusConnection,
            parent,
        )

        try:
            try:
                process.start()
            finally:
                # The parent must release its copy of the sender.
                child.close()

            self._wait_until_ready()

        except BaseException:
            self.close()
            raise

    def _wait_until_ready(self) -> None:
        """Wait until the child reports successful GUI initialization."""

        connection = self._status_connection

        if connection is None:
            raise RuntimeStateError("GUI status pipe is not available")

        deadline = time.monotonic() + self._start_timeout

        while True:
            remaining = deadline - time.monotonic()

            if remaining <= 0:
                raise RuntimeStartError("GUI startup timed out")

            if connection.poll(min(remaining, 0.1)):
                try:
                    status, message = connection.recv()
                except EOFError as exc:
                    raise RuntimeStartError("GUI process disconnected during startup") from exc

                if status == "ready":
                    if not self.is_running:
                        raise RuntimeStartError("GUI exited during startup")
                    return

                if status == "error":
                    raise RuntimeStartError(f"GUI startup failed:\n{message}")

                raise RuntimeStartError(f"Unknown GUI status: {status}")

            if not self.is_running:
                raise RuntimeStartError(f"GUI exited with code {self.exitcode}")

    def request_stop(self) -> None:
        """Request a graceful shutdown without blocking."""

        if self.is_running:
            self._shutdown_event.set()

    def stop(self, timeout: float | None = None) -> None:
        """Stop the child, escalating only when necessary."""

        if timeout is not None and timeout < 0:
            raise ValueError("Stop timeout must not be negative")

        process = self._process

        if process is None or not process.is_alive():
            return

        # Graceful shutdown.
        self.request_stop()

        process.join(self._stop_timeout if timeout is None else timeout)

        # Forced termination.
        if process.is_alive():
            process.terminate()
            process.join(2.0)

        # Last-resort shutdown.
        if process.is_alive():
            process.kill()
            process.join()

    def join(self, timeout: float | None = None) -> int | None:
        """Wait for the GUI process without requesting shutdown."""

        process = self._process

        if process is None:
            return None

        process.join(timeout)
        return process.exitcode

    def close(self) -> None:
        """Release the process and pipe resources."""

        self.stop()

        connection = self._status_connection
        self._status_connection = None

        if connection is not None:
            connection.close()

        process = self._process
        self._process = None

        if process is not None:
            process.close()
