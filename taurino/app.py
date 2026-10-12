"""Application lifecycle, startup ordering and boundary between transport and services."""

from __future__ import annotations

import asyncio
import inspect
import logging
from dataclasses import dataclass
from typing import Any, Literal

from taurino._internal.command import CommandRegistry
from taurino._internal.connection import IpcPeer, IpcServer
from taurino._internal.multiplexer import (
    ApiRequest,
    ApiResponse,
    Multiplexer,
    pack,
    unpack,
)
from taurino._internal.runtime import Runtime
from taurino.app_handle import AppHandle
from taurino.config import Config
from taurino.error import AppLifecycleError, ProtocolError
from taurino.state import State

log = logging.getLogger(__name__)

COMMAND_METHOD = "taurino.command"
EVENT_METHOD = "taurino.event"


@dataclass(eq=False)
class _Session:
    peer: IpcPeer
    multiplexer: Multiplexer


async def _invoke_hook(hook: Any, *args: Any) -> Any:
    if inspect.iscoroutinefunction(hook):
        return await hook(*args)
    value = await asyncio.to_thread(hook, *args)
    if inspect.isawaitable(value):
        return await value
    return value


class App:
    """Owns State, CommandRegistry, IpcServer, Multiplexer(s), and Runtime.

    Startup: user setup -> bind server -> accept task -> spawn Rust -> IPC connected.
    IpcServer never processes or routes a message; App does this here.
    """

    def __init__(
        self,
        *,
        config: Config,
        commands: dict[str, Any],
        setup_hook: Any,
        event_hook: Any,
    ) -> None:
        self.config = config
        self.state = State()
        self.handle = AppHandle(self)
        self._registry = CommandRegistry()
        self._registry.bind(self.handle)
        for name, fn in commands.items():
            self._registry.register(name, fn)
        self._setup_hook = setup_hook
        self._event_hook = event_hook
        self._server: IpcServer | None = None
        self._runtime: Runtime | None = None
        self._accept_task: asyncio.Task[None] | None = None
        self._peer_tasks: set[asyncio.Task[Any]] = set()
        self._primary_session: _Session | None = None
        self._sessions: set[_Session] = set()
        self._connected = asyncio.Event()
        self._started = False
        self._starting = False
        self._stopped = False
        self._ready_to_run = asyncio.Event()

    @property
    def is_running(self) -> bool:
        return self._started and not self._stopped

    @property
    def engine_pid(self) -> int | None:
        return self._runtime.pid if self._runtime else None

    def configure(self, config: Config) -> None:
        """Replace global config only during Builder.setup, before IPC is bound."""
        if not self._starting or self._server is not None:
            raise AppLifecycleError("App.configure is only allowed in the setup hook")
        self.config = config

    async def start(self, start_method: Literal["spawn", "fork"] = "spawn") -> None:
        if self._started or self._starting or self._stopped:
            raise AppLifecycleError("Application has already started or stopped")
        self._starting = True
        try:
            if self._setup_hook is not None:
                await _invoke_hook(self._setup_hook, self)
            self._server = IpcServer(self.config.connection)
            await self._server.start()  # MUST finish before the Rust child process starts.
            self._accept_task = asyncio.create_task(
                self._accept_forever(), name="taurino-accept"
            )
            self._runtime = Runtime(
                self.config.to_engine_bytes(),
                start_timeout=self.config.connection.connect_timeout + 10.0,
                method=start_method,
            )
            # multiprocessing spawn + readiness handshake must never block the asyncio server loop.
            await asyncio.to_thread(self._runtime.start)
            await self.handle.wait_connected(
                timeout=self.config.connection.connect_timeout
            )
            self._started = True
            self._ready_to_run.set()
        except BaseException:
            await self.stop()
            raise
        finally:
            self._starting = False

    async def run_forever(self) -> int | None:
        """Block asynchronously until the child exits; always shut down the Python side."""
        await self.start()
        try:
            assert self._runtime is not None
            return await asyncio.to_thread(self._runtime.join)
        finally:
            await self.stop()

    async def _accept_forever(self) -> None:
        assert self._server is not None
        while not self._stopped:
            try:
                peer = await self._server.accept()
            except asyncio.CancelledError:
                break
            except Exception:
                if self._stopped:
                    break
                log.exception("IPC accept failed")
                await asyncio.sleep(0.1)
                continue
            task = asyncio.create_task(self._serve_peer(peer), name="taurino-peer")
            self._peer_tasks.add(task)
            task.add_done_callback(self._peer_tasks.discard)

    async def _serve_peer(self, peer: IpcPeer) -> None:
        assert self._server is not None
        session = _Session(
            peer,
            Multiplexer(
                peer.send,
                timeout=self.config.connection.operation_timeout,
                max_message_size=self.config.connection.max_message_size,
            ),
        )
        self._sessions.add(session)
        if self._primary_session is None:
            self._primary_session = session
            self._connected.set()
        # Backpressure: tasks are created only after acquiring a bounded slot.
        capacity = asyncio.Semaphore(64)
        messages: set[asyncio.Task[None]] = set()
        try:
            while not self._stopped:
                raw = await peer.receive()
                value = unpack(raw, max_size=self.config.connection.max_message_size)
                if isinstance(value, list) and len(value) == 4:
                    session.multiplexer.resolve(ApiResponse.from_array(value))
                    continue
                request = ApiRequest.from_array(value)
                await capacity.acquire()
                task = asyncio.create_task(
                    self._dispatch_request(session, request, capacity),
                    name=f"taurino-req-{request.request_id}",
                )
                messages.add(task)
                task.add_done_callback(messages.discard)
        except asyncio.CancelledError:
            pass
        except (EOFError, ConnectionError, TimeoutError):
            log.info("IPC session disconnected")
        except (ProtocolError, ValueError):
            log.warning("Rejected invalid IPC message", exc_info=True)
        except Exception:
            log.exception("IPC session failed")
        finally:
            for task in messages:
                task.cancel()
            if messages:
                await asyncio.gather(*messages, return_exceptions=True)
            session.multiplexer.close()
            await self._server.disconnect(peer)
            self._sessions.discard(session)
            if self._primary_session is session:
                self._primary_session = next(iter(self._sessions), None)
                if self._primary_session is None:
                    self._connected.clear()

    async def _dispatch_request(
        self, session: _Session, request: ApiRequest, capacity: asyncio.Semaphore
    ) -> None:
        try:
            if request.method == COMMAND_METHOD:
                args = request.args.value
                if (
                    len(args) != 4
                    or not isinstance(args[0], str)
                    or not isinstance(args[1], int)
                    or not isinstance(args[2], int)
                    or not isinstance(args[3], dict)
                ):
                    response = request.err(
                        400,
                        "Command arguments must be [name, result_id, error_id, data]",
                    )
                else:
                    name, result_id, error_id, data = args
                    result = await self._registry.dispatch(
                        name, result_id=result_id, error_id=error_id, data=data
                    )
                    response = request.ok(result)
            elif request.method == EVENT_METHOD:
                if len(request.args.value) != 1:
                    response = request.err(
                        400, "Event arguments must contain exactly one event"
                    )
                else:
                    if self._event_hook is not None:
                        await _invoke_hook(
                            self._event_hook, self.handle, request.args.single()
                        )
                    response = request.ok(None)
            else:
                response = request.err(404, "Unknown internal IPC method")
            await session.peer.send(
                pack(
                    response.to_array(),
                    max_size=self.config.connection.max_message_size,
                )
            )
        except asyncio.CancelledError:
            raise
        except Exception:
            log.exception("Request dispatch failed")
            try:
                await session.peer.send(
                    pack(request.err(500, "Internal dispatch error").to_array())
                )
            except Exception:
                log.exception("Failed to send dispatch error")
        finally:
            capacity.release()

    async def stop(self) -> None:
        if self._stopped:
            return
        self._stopped = True
        self._ready_to_run.set()
        self._connected.clear()
        # Stop Rust before closing the local IPC server to enable graceful client exit.
        if self._runtime is not None:
            await asyncio.to_thread(self._runtime.close)
            self._runtime = None
        if self._accept_task is not None:
            self._accept_task.cancel()
            await asyncio.gather(self._accept_task, return_exceptions=True)
            self._accept_task = None
        if self._server is not None:
            await self._server.stop()
            self._server = None
        current = asyncio.current_task()
        to_cancel = [task for task in self._peer_tasks if task is not current]
        for task in to_cancel:
            task.cancel()
        if to_cancel:
            await asyncio.gather(*to_cancel, return_exceptions=True)
        self._peer_tasks.clear()
        self._primary_session = None
        self._sessions.clear()
        self._registry.close()
        self._started = False

    async def __aenter__(self) -> "App":
        await self.start()
        return self

    async def __aexit__(self, *_: Any) -> None:
        await self.stop()
