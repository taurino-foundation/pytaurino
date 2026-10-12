"""Per-connection async RPC; independent of CommandRegistry."""

from __future__ import annotations

import asyncio
from collections.abc import Awaitable, Callable
from dataclasses import asdict, dataclass, field, is_dataclass
from typing import Any, TypeVar, cast

import msgpack
from pydantic import TypeAdapter, ValidationError

from taurino.error import (
    MultiplexerClosedError,
    ProtocolError,
    RequestIdsExhausted,
)

T = TypeVar("T")
MAX_MESSAGE_SIZE = 1024 * 1024
MAX_REQUEST_ID = 65535


def pack(value: Any, *, max_size: int = MAX_MESSAGE_SIZE) -> bytes:
    def serialize(x: Any) -> Any:
        if is_dataclass(x) and not isinstance(x, type):
            return asdict(x)

        raise TypeError(f"Unsupported MessagePack value: {type(x).__name__}")

    result = msgpack.packb(
        value,
        use_bin_type=True,
        default=serialize,
    )

    if not isinstance(result, bytes):
        raise ProtocolError("MessagePack serialization failed")

    if len(result) > max_size:
        raise ProtocolError("Message too large")

    return result


def unpack(data: bytes, *, max_size: int = MAX_MESSAGE_SIZE) -> Any:
    if len(data) > max_size:
        raise ProtocolError("Message too large")
    try:
        return msgpack.unpackb(
            data,
            raw=False,
            strict_map_key=True,
            max_str_len=max_size,
            max_bin_len=max_size,
            max_array_len=max_size,
            max_map_len=max_size,
        )
    except (
        ValueError,
        msgpack.ExtraData,
        msgpack.FormatError,
        msgpack.StackError,
        TypeError,
    ) as exc:
        raise ProtocolError("Invalid MessagePack payload") from exc


def validate_id(value: Any) -> None:
    if type(value) is not int or not 0 <= value <= MAX_REQUEST_ID:
        raise ProtocolError("request_id must be a u16 integer")


def validate_code(value: Any) -> None:
    if type(value) is not int or not -(2**31) <= value < 2**31:
        raise ProtocolError("code must be an i32 integer")


@dataclass(slots=True)
class ApiArguments:
    value: list[Any] = field(default_factory=list)

    def __post_init__(self) -> None:
        if not isinstance(self.value, list):
            raise ProtocolError("Arguments must be an array")

    def single(self) -> Any:
        if len(self.value) != 1:
            raise ProtocolError("Expected exactly one argument")
        return self.value[0]

    def get(self) -> list[Any]:
        return self.value

    def optional(self, args_size: int) -> list[Any]:
        if type(args_size) is not int or args_size < 0:
            raise ValueError("args_size must be nonnegative")
        return self.value + [None] * max(0, args_size - len(self.value))


@dataclass(slots=True)
class ApiRequest:
    request_id: int
    method: str
    arguments: ApiArguments = field(default_factory=ApiArguments)

    def __post_init__(self) -> None:
        validate_id(self.request_id)
        if not isinstance(self.method, str) or not self.method:
            raise ProtocolError("method must be a nonempty string")
        if not isinstance(self.arguments, ApiArguments):
            raise ProtocolError("arguments must be ApiArguments")

    @property
    def args(self) -> ApiArguments:
        return self.arguments

    def ok(self, data: Any = None) -> ApiResponse:
        return ApiResponse(self.request_id, 0, "ok", data)

    def err(self, code: int, message: str) -> ApiResponse:
        return ApiResponse(self.request_id, code, message, None)

    def to_array(self) -> list[Any]:
        return [self.request_id, self.method, self.arguments.value]

    def to_msgpack(self) -> bytes:
        return pack(self.to_array())

    @classmethod
    def from_array(cls, value: Any) -> ApiRequest:
        if not isinstance(value, list) or len(value) != 3 or not isinstance(value[2], list):
            raise ProtocolError("Invalid ApiRequest array")
        return cls(value[0], value[1], ApiArguments(value[2]))

    @classmethod
    def from_msgpack(cls, data: bytes) -> ApiRequest:
        return cls.from_array(unpack(data))


@dataclass(slots=True)
class ApiResponse:
    request_id: int
    code: int
    message: str
    data: Any = None

    def __post_init__(self) -> None:
        validate_id(self.request_id)
        validate_code(self.code)
        if not isinstance(self.message, str):
            raise ProtocolError("message must be a string")

    def to_array(self) -> list[Any]:
        return [self.request_id, self.code, self.message, self.data]

    def to_msgpack(self) -> bytes:
        return pack(self.to_array())

    @classmethod
    def from_array(cls, value: Any) -> ApiResponse:
        if not isinstance(value, list) or len(value) != 4:
            raise ProtocolError("Invalid ApiResponse array")
        return cls(*value)

    @classmethod
    def from_msgpack(cls, data: bytes) -> ApiResponse:
        return cls.from_array(unpack(data))


class ApiRemoteError(Exception):
    def __init__(self, response: ApiResponse):
        self.response = response
        self.code = response.code
        super().__init__(f"RPC error {response.code}: {response.message}")


@dataclass(slots=True)
class PendingRequest:
    future: asyncio.Future[ApiResponse]
    adapter: TypeAdapter[Any]


class Multiplexer:
    """One asyncio loop, one IPC session; u16 request IDs never reused in that session."""

    def __init__(
        self,
        send: Callable[[bytes], Awaitable[None]],
        *,
        timeout: float | None = 30.0,
        max_message_size: int = MAX_MESSAGE_SIZE,
    ):
        if timeout is not None and timeout <= 0:
            raise ValueError("timeout must be positive or None")
        if max_message_size <= 0:
            raise ValueError("max_message_size must be positive")
        self._max_message_size = max_message_size
        self._send = send
        self._timeout = timeout
        self._pending: dict[int, PendingRequest] = {}
        self._adapters: dict[Any, TypeAdapter[Any]] = {}
        self._next_id = 0
        self._closed = False
        self._loop: asyncio.AbstractEventLoop | None = None

    @property
    def pending_count(self) -> int:
        return len(self._pending)

    @property
    def closed(self) -> bool:
        return self._closed

    def _get_loop(self) -> asyncio.AbstractEventLoop:
        loop = asyncio.get_running_loop()
        if self._loop is None:
            self._loop = loop
        elif self._loop is not loop:
            raise RuntimeError("Multiplexer used from another asyncio event loop")
        return loop

    def _get_adapter(self, result_type: Any) -> TypeAdapter[Any]:
        try:
            adapter = self._adapters.get(result_type)
        except TypeError:
            return TypeAdapter(result_type)
        if adapter is None:
            adapter = TypeAdapter(result_type)
            self._adapters[result_type] = adapter
        return adapter

    def _allocate_id(self) -> int:
        if self._closed:
            raise MultiplexerClosedError("Multiplexer closed")
        if self._next_id > MAX_REQUEST_ID:
            raise RequestIdsExhausted("u16 IDs exhausted; reconnect to create a fresh session")
        request_id = self._next_id
        self._next_id += 1
        return request_id

    async def request(
        self,
        method: str,
        *args: Any,
        result_type: type[T],
        timeout: float | None = None,
    ) -> ApiResponse:
        loop = self._get_loop()
        adapter = self._get_adapter(result_type)
        if timeout is not None and timeout <= 0:
            raise ValueError("timeout must be positive or None")
        request_id = self._allocate_id()
        payload = pack(
            ApiRequest(request_id, method, ApiArguments(list(args))).to_array(),
            max_size=self._max_message_size,
        )
        future: asyncio.Future[ApiResponse] = loop.create_future()
        self._pending[request_id] = PendingRequest(future, adapter)
        try:
            async with asyncio.timeout(self._timeout if timeout is None else timeout):
                await self._send(payload)
                response = await future
            if response.code != 0:
                raise ApiRemoteError(response)
            return response
        finally:
            self._pending.pop(request_id, None)
            if not future.done():
                future.cancel()
            elif not future.cancelled():
                # Retrieve any exception to avoid "Future exception was never retrieved".
                future.exception()

    async def call(
        self,
        method: str,
        *args: Any,
        result_type: type[T],
        timeout: float | None = None,
    ) -> T:
        response = await self.request(method, *args, result_type=result_type, timeout=timeout)
        return cast(T, response.data)

    def resolve(self, response: ApiResponse) -> bool:
        self._get_loop()

        pending = self._pending.get(response.request_id)

        if pending is None or pending.future.done():
            return False

        if response.code != 0:
            pending.future.set_result(response)
            return True

        try:
            validated = pending.adapter.validate_python(response.data)

        except ValidationError as exc:
            pending.future.set_exception(exc)

        else:
            pending.future.set_result(
                ApiResponse(
                    response.request_id,
                    response.code,
                    response.message,
                    validated,
                )
            )

        return True

    def feed_data(self, data: bytes) -> bool:
        return self.resolve(ApiResponse.from_msgpack(data))

    def cancel(self, request_id: int) -> bool:
        self._get_loop()
        pending = self._pending.get(request_id)
        if pending is None or pending.future.done():
            return False
        pending.future.cancel()
        return True

    def close(self) -> None:
        self._get_loop()
        if self._closed:
            return
        self._closed = True
        for pending in self._pending.values():
            if not pending.future.done():
                pending.future.set_exception(MultiplexerClosedError("Multiplexer closed"))
