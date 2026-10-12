"""Raw local IPC server/peer; intentionally has no message routing/business handlers."""

from __future__ import annotations

import asyncio
import os
from typing import TYPE_CHECKING, Self

from taurino.config import IpcConnectionConfig, IpcSecurity

if TYPE_CHECKING:
    from taunicorn import Connection, Server
    from taunicorn.security import SecureConnection


async def read_exact(connection: Connection, size: int, timeout: float | None) -> bytes:
    chunks = bytearray()
    async with asyncio.timeout(timeout):
        while len(chunks) < size:
            part = await connection.receive(size - len(chunks))
            if not part:
                raise EOFError("IPC peer disconnected")
            chunks.extend(part)
    return bytes(chunks)


class IpcPeer:
    def __init__(
        self,
        connection: Connection,
        *,
        secure: SecureConnection | None,
        config: IpcConnectionConfig,
    ):
        self.connection = connection
        self.secure = secure
        self.config = config
        self._send_lock = asyncio.Lock()
        self._receive_lock = asyncio.Lock()
        self._closed = False

    @property
    def closed(self) -> bool:
        return self._closed

    async def send(self, data: bytes) -> None:
        if self._closed:
            raise ConnectionError("IPC peer closed")
        if not isinstance(data, bytes):
            raise TypeError("Expected bytes")
        if not 0 < len(data) <= self.config.max_message_size:
            raise ValueError("Invalid message size")
        async with self._send_lock:
            if self._closed:
                raise ConnectionError("IPC peer closed")
            try:
                async with asyncio.timeout(self.config.operation_timeout):
                    if self.secure is not None:
                        await self.secure.send(data)
                    else:
                        await self.connection.send(len(data).to_bytes(4, "big") + data)
            except BaseException:
                await self.close()
                raise

    async def receive(self) -> bytes:
        if self._closed:
            raise ConnectionError("IPC peer closed")

        async with self._receive_lock:
            if self._closed:
                raise ConnectionError("IPC peer closed")

            try:
                if self.secure is not None:
                    # No application-level idle timeout.
                    # SecureConnection manages its own framing.
                    data = await self.secure.receive()

                else:
                    # Wait indefinitely for the first byte.
                    # An idle connection is not an error.
                    first_byte = await read_exact(
                        self.connection,
                        1,
                        None,
                    )

                    # Once a frame starts, enforce the timeout.
                    async with asyncio.timeout(self.config.operation_timeout):
                        remaining_header = await read_exact(
                            self.connection,
                            3,
                            None,
                        )

                        header = first_byte + remaining_header
                        length = int.from_bytes(header, "big")

                        if not 0 < length <= self.config.max_message_size:
                            raise ValueError("Invalid IPC frame")

                        data = await read_exact(
                            self.connection,
                            length,
                            None,
                        )

                if not 0 < len(data) <= self.config.max_message_size:
                    raise ValueError("Invalid IPC message")

                return data

            except asyncio.CancelledError:
                # Cancellation is controlled by application lifecycle.
                raise

            except BaseException:
                await self.close()
                raise

    async def close(self) -> None:
        if self._closed:
            return
        self._closed = True
        if self.secure is not None:
            await self.secure.close()
        else:
            await self.connection.close()


class IpcServer:
    """Lifecycle, client admission, and peer ownership only (no router)."""

    def __init__(self, config: IpcConnectionConfig):
        self.config = config
        self._server: Server | None = None
        self._clients: set[IpcPeer] = set()
        self._pending = 0
        self._closed = False
        self._admission_lock = asyncio.Lock()

    @property
    def clients(self) -> tuple[IpcPeer, ...]:
        return tuple(peer for peer in self._clients if not peer.closed)

    @property
    def client_count(self) -> int:
        return len(self.clients)

    @property
    def running(self) -> bool:
        return self._server is not None and not self._closed

    async def start(self) -> None:
        from taunicorn import Server

        if self._closed or self._server is not None:
            raise RuntimeError("Server unavailable")

        endpoint = self.config.endpoint
        permissions = self.config.permissions

        if permissions is None:
            self._server = await Server.start(endpoint)
            return

        if os.name == "nt":
            # Windows: SDDL security descriptor
            if not isinstance(permissions, str):
                raise TypeError("Windows IPC permissions must be an SDDL string")

            self._server = await Server.bind(
                endpoint,
                mode=permissions,
            )

        else:
            # Unix: POSIX file permissions
            if type(permissions) is not int:
                raise TypeError("Unix IPC permissions must be an integer")

            self._server = await Server.bind(
                endpoint,
                mode=permissions,  # pyright: ignore[reportArgumentType]
            )

    async def accept(self) -> IpcPeer:
        server = self._server
        if server is None or self._closed:
            raise RuntimeError("Server is not running")
        connection = await server.accept()
        reserved = False
        try:
            async with self._admission_lock:
                self._clients = {peer for peer in self._clients if not peer.closed}
                if self._closed:
                    raise ConnectionError("Server stopped")
                if len(self._clients) + self._pending >= self.config.max_clients:
                    raise ConnectionError("Maximum clients reached")
                self._pending += 1
                reserved = True

            secure = None
            if self.config.security == IpcSecurity.AUTHENTICATED:
                from taunicorn.security import SecureConnection

                key_path = self.config.server_private_key_path
                assert key_path is not None
                assert self.config.client_public_key is not None
                private_key = key_path.read_bytes()
                if len(private_key) != 32:
                    raise ValueError("Invalid server private key")
                async with asyncio.timeout(self.config.handshake_timeout):
                    secure = await SecureConnection.server(
                        connection,
                        identity_private_key=private_key,
                        client_identity_public_key=self.config.client_public_key,
                    )
            peer = IpcPeer(connection, secure=secure, config=self.config)
            async with self._admission_lock:
                if self._closed:
                    raise ConnectionError("Server stopped")
                self._clients.add(peer)
            return peer
        except BaseException:
            await connection.close()
            raise
        finally:
            if reserved:
                async with self._admission_lock:
                    self._pending -= 1

    async def disconnect(self, peer: IpcPeer) -> None:
        async with self._admission_lock:
            self._clients.discard(peer)
        await peer.close()

    async def stop(self) -> None:
        async with self._admission_lock:
            if self._closed:
                return
            self._closed = True
            server, self._server = self._server, None
            clients = tuple(self._clients)
            self._clients.clear()
        if server is not None:
            await server.stop()
        await asyncio.gather(*(peer.close() for peer in clients), return_exceptions=True)

    async def __aenter__(self) -> Self:
        await self.start()
        return self

    async def __aexit__(self, *_: object) -> None:
        await self.stop()
