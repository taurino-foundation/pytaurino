"""Spawn-based child process lifecycle, suitable for GUI main-thread event loops."""

from __future__ import annotations

import multiprocessing as mp
import time
import traceback
from collections.abc import Callable
from multiprocessing.connection import Connection
from multiprocessing.context import BaseContext
from typing import Any, Callable, Literal, Protocol

from taurino.core import runtime
from taurino.error import RuntimeStartError, RuntimeStateError


def _gui_process_entry(
    config: bytes,
    shutdown_event: Any,
    status: Connection,
) -> None:
    ready_sent = False

    def notify_ready() -> None:
        nonlocal ready_sent

        if not ready_sent:
            status.send(("ready", None))
            ready_sent = True

    try:
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
    """Internal GUI child controller. The native Rust binding runs on the child's main thread.

    The target must be an importable top-level function (multiprocessing spawn).
    It must invoke notify_ready ONLY after native setup / IPC connectivity is ready.
    """

    def __init__(
        self,
        config: bytes,
        *,
        name: str = "taurino-gui",
        start_timeout: float = 15.0,
        stop_timeout: float = 5.0,
        method: Literal["spawn", "fork"] = "spawn",
    ):
        if start_timeout <= 0 or stop_timeout < 0:
            raise ValueError("Invalid runtime timeout")
        self._config, self._name = config, name
        self._start_timeout, self._stop_timeout = start_timeout, stop_timeout
        self._context: BaseContext = mp.get_context(method)
        self._process: mp.Process | None = None
        self._shutdown_event = self._context.Event()
        self._status_connection: Connection | None = None

    @property
    def pid(self) -> int | None:
        return self._process.pid if self._process else None

    @property
    def exitcode(self) -> int | None:
        return self._process.exitcode if self._process else None

    @property
    def is_running(self) -> bool:
        return self._process is not None and self._process.is_alive()

    @property
    def is_stopping(self) -> bool:
        return self.is_running and self._shutdown_event.is_set()

    def start(self) -> None:
        if self.is_running:
            raise RuntimeStateError("GUI runtime is already running")
        self.close()
        self._shutdown_event = self._context.Event()
        parent, child = self._context.Pipe(duplex=False)
        process = self._context.Process(
            name=self._name,
            target=_gui_process_entry,
            args=(self._config, self._shutdown_event, child),
            daemon=False,
        )
        self._process = process
        self._status_connection = parent
        try:
            process.start()
        except BaseException:
            self._process = None
            self._status_connection = None
            parent.close()
            process.close()
            raise
        finally:
            child.close()
        try:
            self._wait_until_ready()
        except BaseException:
            self.stop()
            raise

    def _wait_until_ready(self) -> None:
        assert self._status_connection is not None
        conn = self._status_connection
        deadline = time.monotonic() + self._start_timeout
        while True:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise RuntimeStartError("GUI startup timed out")
            if conn.poll(min(remaining, 0.1)):
                try:
                    status, message = conn.recv()
                except EOFError as exc:
                    raise RuntimeStartError(
                        "GUI process disconnected during startup"
                    ) from exc
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
        if self.is_running:
            self._shutdown_event.set()

    def stop(self, timeout: float | None = None) -> None:
        process = self._process
        if process is None:
            return
        if process.is_alive():
            self.request_stop()
            process.join(self._stop_timeout if timeout is None else timeout)
            if process.is_alive():
                process.terminate()
                process.join(2.0)
            if process.is_alive():
                process.kill()
                process.join()

    def join(self, timeout: float | None = None) -> int | None:
        if self._process is None:
            return None
        self._process.join(timeout)
        return self._process.exitcode

    def close(self) -> None:
        self.stop()
        if self._status_connection is not None:
            self._status_connection.close()
            self._status_connection = None
        if self._process is not None:
            self._process.close()
            self._process = None
