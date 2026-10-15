"""Application lifecycle, startup ordering and boundary between transport and services."""

from __future__ import annotations

import asyncio
import inspect
import logging
from dataclasses import dataclass
from typing import Any, Literal

from pydantic import ValidationError

from taurino._internal.command import CommandEnvelope, CommandRegistry, InvokeRequest
from taurino._internal.connection import IpcPeer, IpcServer
from taurino._internal.multiplexer import (
    ApiResponse,
    Multiplexer,
    pack,
    unpack,
)
from taurino.app_handle import AppHandle
from taurino.config import AppBuilder
from taurino.core.runtime import Runtime
from taurino.error import AppLifecycleError, ProtocolError
from taurino.proto import AppConfig
from taurino.state import State

log = logging.getLogger(__name__)

COMMAND_METHOD = "commands.invoke"
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
        commands: dict[str, Any],
        setup_hook: Any = None,
        loop_event_hook: Any = None,
        window_event_hook: Any = None,
        webview_event_hook: Any = None,
        systemtray_event_hook: Any = None,
        shortcut_event_hook: Any = None,
    ) -> None:
        self._config_builder = AppBuilder()
        self.config: AppConfig | None = None
        self.state = State()
        self.handle = AppHandle(self)
        self._registry = CommandRegistry()
        self._registry.bind(self.handle)
        for name, fn in commands.items():
            self._registry.register(name, fn)
        self._setup_hook = setup_hook
        self._native_event_hooks: dict[str, Any] = {
            "loop": loop_event_hook,
            "window": window_event_hook,
            "webview": webview_event_hook,
            "tray": systemtray_event_hook,
            "shortcut": shortcut_event_hook,
        }

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

    def _require_config(self) -> AppConfig:
        config = self.config

        if config is None:
            raise AppLifecycleError("Application configuration is not initialized")

        return config

    def configure(self) -> AppBuilder:
        """Replace global config only during Builder.setup, before IPC is bound."""
        if not self._starting or self._server is not None:
            raise AppLifecycleError("App.configure is only allowed in the setup hook")
        return self._config_builder

    async def __start(
        self,
        start_method: Literal["spawn", "fork"] = "spawn",
    ) -> None:
        if self._started or self._starting or self._stopped:
            raise AppLifecycleError("Application has already started or stopped")

        self._starting = True

        try:
            # 1. Execute user setup hook
            if self._setup_hook is not None:
                await _invoke_hook(self._setup_hook, self)

            # 2. Build and validate configuration
            config: AppConfig = self._config_builder.build()
            self.config = config

            # 3. Start Python IPC server
            server = IpcServer(config.connection)
            self._server = server

            await server.start()

            # 4. Start IPC accept loop
            self._accept_task = asyncio.create_task(
                self._accept_forever(),
                name="taurino-accept",
            )

            # 5. Create Rust runtime
            runtime = Runtime(
                config.to_engine_bytes(),
                start_timeout=config.connection.connect_timeout + 10.0,
                method=start_method,
            )
            self._runtime = runtime

            # 6. Start Rust process without blocking asyncio
            await asyncio.to_thread(runtime.start)

            # 7. Wait for Rust IPC connection
            await self.handle.wait_connected(timeout=config.connection.connect_timeout)  # type: ignore

            self._started = True
            self._ready_to_run.set()

        except BaseException:
            await self.stop()
            raise

        finally:
            self._starting = False

    async def run(self, start_method: Literal["spawn", "fork"] = "spawn") -> int | None:
        """Block asynchronously until the child exits; always shut down the Python side."""
        await self.__start(start_method)
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
                timeout=self._require_config().connection.operation_timeout,
                max_message_size=self._require_config().connection.max_message_size,
            ),
        )

        self._sessions.add(session)

        if self._primary_session is None:
            self._primary_session = session
            self._connected.set()

        capacity = asyncio.Semaphore(64)
        messages: set[asyncio.Task[None]] = set()

        try:
            while not self._stopped:
                raw = await peer.receive()

                value = unpack(
                    raw,
                    max_size=self._require_config().connection.max_message_size,
                )

                if not isinstance(value, (list, tuple)):
                    log.warning("Invalid IPC envelope type")
                    continue
                # --------------------------------------------------
                # Native Event: Rust -> Python
                # ["taurino.event", {"type": "...", "payload": {...}}]
                # --------------------------------------------------
                if len(value) == 2:
                    method, event = value

                    if method != EVENT_METHOD:
                        log.warning("Unknown IPC notification: %r", method)
                        continue

                    if not isinstance(event, dict):
                        log.warning("Invalid native event payload")
                        continue

                    event_type = event.get("type")
                    event_payload = event.get("payload")

                    if (
                        not isinstance(event_type, str)
                        or event_type not in self._native_event_hooks
                        or not isinstance(event_payload, dict)
                    ):
                        log.warning("Invalid native event: %r", event)
                        continue

                    # Kein registrierter Hook -> keine weitere Verarbeitung.
                    if self._native_event_hooks[event_type] is None:
                        continue

                    # Anzahl paralleler Event-Tasks begrenzen.
                    if len(messages) >= 512:
                        log.warning("Native event queue is full")
                        continue

                    task = asyncio.create_task(
                        self._dispatch_native_event(event),
                        name=f"taurino-event-{event_type}",
                    )

                    messages.add(task)
                    task.add_done_callback(messages.discard)

                    continue
                # RPC response: Rust -> Python Multiplexer
                if len(value) == 4:
                    response = ApiResponse.from_array(value)
                    session.multiplexer.resolve(response)
                    continue

                # RPC request: Rust -> Python App
                if len(value) != 3:
                    log.warning("Invalid IPC envelope length")
                    continue

                request_id, method, arguments = value

                if type(request_id) is not int or not 0 <= request_id <= 0xFFFF:
                    log.warning("Invalid RPC request ID")
                    continue

                if not isinstance(method, str):
                    await self._send_rpc_response(session, request_id, 400, "Invalid RPC method")
                    continue

                if method != COMMAND_METHOD:
                    await self._send_rpc_response(
                        session,
                        request_id,
                        404,
                        f"Unknown RPC method: {method}",
                    )
                    continue

                if not isinstance(arguments, (list, tuple)) or len(arguments) != 1:
                    await self._send_rpc_response(
                        session,
                        request_id,
                        400,
                        "commands.invoke expects exactly one argument",
                    )
                    continue

                try:
                    envelope = CommandEnvelope.model_validate(arguments[0])

                    if envelope.cmd != envelope.request.cmd:
                        await self._send_rpc_response(
                            session,
                            request_id,
                            400,
                            "Command name mismatch",
                        )
                        continue

                    invoke = envelope.request

                except ValidationError:
                    log.warning(
                        "Rejected malformed command request",
                        exc_info=True,
                    )

                    await self._send_rpc_response(
                        session,
                        request_id,
                        400,
                        "Invalid command request",
                    )
                    continue

                await capacity.acquire()

                try:
                    task = asyncio.create_task(
                        self._dispatch_command(
                            session,
                            request_id,
                            invoke,
                            capacity,
                        ),
                        name=f"taurino-req-{invoke.cmd}",
                    )
                except BaseException:
                    capacity.release()
                    raise

                messages.add(task)
                task.add_done_callback(messages.discard)

        except asyncio.CancelledError:
            pass

        except (EOFError, ConnectionError, TimeoutError):
            log.info("IPC session disconnected")

        except (ProtocolError, ValueError):
            log.warning("Rejected invalid IPC protocol message", exc_info=True)

        except Exception:
            log.exception("IPC session failed")

        finally:
            for task in messages:
                task.cancel()

            if messages:
                await asyncio.gather(
                    *messages,
                    return_exceptions=True,
                )

            session.multiplexer.close()

            await self._server.disconnect(peer)

            self._sessions.discard(session)

            if self._primary_session is session:
                self._primary_session = next(
                    iter(self._sessions),
                    None,
                )

                if self._primary_session is None:
                    self._connected.clear()

    async def _dispatch_native_event(
        self,
        event: dict,
    ) -> None:
        """Dispatch a native event to its registered category hook."""

        hook = self._native_event_hooks.get(event["type"])

        if hook is None:
            return

        try:
            await _invoke_hook(
                hook,
                self.handle,
                event["payload"],
            )
        except Exception:
            log.exception(
                "Native event hook failed: %s",
                event["type"],
            )

    async def _send_rpc_response(
        self,
        session: _Session,
        request_id: int,
        status: int,
        message: str,
        data: Any = None,
    ) -> None:
        payload = pack(
            [request_id, status, message, data],
            max_size=self._require_config().connection.max_message_size,
        )
        await session.peer.send(payload)

    async def _dispatch_command(
        self,
        session: _Session,
        request_id: int,
        request: InvokeRequest,
        capacity: asyncio.Semaphore,
    ) -> None:
        try:
            try:
                result = await self._registry.dispatch(request)

                await self._send_rpc_response(
                    session,
                    request_id,
                    0,
                    "ok",
                    result.model_dump(mode="json"),
                )

            except asyncio.CancelledError:
                raise

            except Exception:
                log.exception(
                    "Command dispatch failed: %s",
                    request.cmd,
                )

                try:
                    await self._send_rpc_response(
                        session,
                        request_id,
                        500,
                        "Internal command dispatch error",
                    )
                except Exception:
                    log.exception("Failed to send RPC error response")

        finally:
            capacity.release()

    async def emit(
        self,
        event: str,
        payload: Any = None,
        *,
        webview_label: str = "webview-main",
    ) -> None:
        if not isinstance(event, str) or not event:
            raise ValueError("Invalid event name")

        if not isinstance(webview_label, str) or not webview_label:
            raise ValueError("Invalid webview label")

        session = self._primary_session

        if session is None or session.peer.closed:
            raise ConnectionError("No active IPC session")

        message = [
            "webview.emit",
            {
                "webview_label": webview_label,
                "event": event,
                "payload": payload,
            },
        ]

        data = pack(
            message,
            max_size=self._require_config().connection.max_message_size,
        )

        await session.peer.send(data)

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

    async def __aenter__(self) -> App:
        await self.__start()
        return self

    async def __aexit__(self, *_: Any) -> None:
        await self.stop()
