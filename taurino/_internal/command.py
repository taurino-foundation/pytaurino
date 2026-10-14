"""Internal Taurino command registry for JavaScript invoke requests."""

from __future__ import annotations

import asyncio
import inspect
import logging
import time
from collections.abc import Awaitable, Callable, Mapping
from dataclasses import dataclass
from enum import StrEnum
from typing import (
    TYPE_CHECKING,
    Annotated,
    Any,
    Self,
    TypeVar,
    cast,
    get_args,
    get_origin,
    get_type_hints,
)

import msgpack
from pydantic import (
    AliasChoices,
    BaseModel,
    ConfigDict,
    Field,
    ValidationError,
    create_model,
)
from pydantic.types import JsonValue
from pydantic_core import to_jsonable_python

if TYPE_CHECKING:
    from taurino.app_handle import AppHandle


log = logging.getLogger(__name__)
T = TypeVar("T")
CommandFn = Callable[..., Any]
Provider = Callable[[], Any | Awaitable[Any]]
BeforeHook = Callable[[str, dict[str, Any]], Any]
AfterHook = Callable[[str, dict[str, Any], dict[str, Any]], Any]

UInt32 = Annotated[int, Field(strict=True, ge=0, le=0xFFFF_FFFF)]


class InvokeRequest(BaseModel):
    """JS -> Rust -> Python. Identical field names to Rust InvokeRequest."""

    model_config = ConfigDict(extra="ignore", strict=True)

    cmd: str
    callback: UInt32
    error: UInt32
    payload: JsonValue = None
    options: JsonValue | None = None

    @classmethod
    def from_any(
        cls,
        data: Any,
    ) -> Self:
        return cls.model_validate(data)


class InvokeResponse(BaseModel):
    """Python -> Rust. Rust uses original request callback/error IDs.

    data is *exactly* the value passed to JS runCallback(id, data),
    not an RPC envelope that JavaScript would have to unwrap.
    """

    model_config = ConfigDict(extra="forbid")

    ok: bool
    data: JsonValue

    def to_bytes(self) -> bytes:
        packed = msgpack.packb(
            self.model_dump(mode="json"),
            use_bin_type=True,
        )

        if not isinstance(packed, bytes):
            raise TypeError("MessagePack serialization must return bytes")

        return packed


class CommandErrorCode(StrEnum):
    NO_HANDLER = "NO_HANDLER"
    INVALID_PARAMS = "INVALID_PARAMS"
    VALIDATION_ERROR = "VALIDATION_ERROR"
    DEPENDENCY_ERROR = "DEPENDENCY_ERROR"
    EXECUTION_ERROR = "EXECUTION_ERROR"
    INTERNAL_ERROR = "INTERNAL_ERROR"


class CommandException(Exception):
    def __init__(
        self,
        code: CommandErrorCode,
        message: str,
        details: Any = None,
    ) -> None:
        super().__init__(message)
        self.code = code
        self.message = message
        self.details = details


@dataclass(frozen=True, slots=True)
class Depends:
    """Explicit dependency injection: Annotated[Service, Depends()]."""


class DependencyContainer:
    def __init__(self) -> None:
        self._singletons: dict[type[Any], Any] = {}
        self._providers: dict[type[Any], Provider] = {}
        self._locks: dict[type[Any], asyncio.Lock] = {}

    def register_singleton(self, dep_type: type[T], instance: T) -> None:
        if not isinstance(instance, dep_type):
            raise TypeError(f"Expected instance of {dep_type.__name__}")
        self._singletons[dep_type] = instance

    def register_factory(
        self,
        dep_type: type[T],
        provider: Callable[[], T | Awaitable[T]],
    ) -> None:
        self._providers[dep_type] = provider
        self._singletons.pop(dep_type, None)

    async def resolve(self, dep_type: type[T]) -> T:
        existing = self._singletons.get(dep_type)
        if existing is not None:
            return cast(T, existing)

        if dep_type not in self._providers:
            raise CommandException(
                CommandErrorCode.DEPENDENCY_ERROR,
                f"No provider for {dep_type.__name__}",
            )

        lock = self._locks.setdefault(dep_type, asyncio.Lock())
        async with lock:
            if dep_type not in self._singletons:
                provided = self._providers[dep_type]()
                if inspect.isawaitable(provided):
                    provided = await provided
                if not isinstance(provided, dep_type):
                    raise CommandException(
                        CommandErrorCode.DEPENDENCY_ERROR,
                        f"Provider returned the wrong type for {dep_type.__name__}",
                    )
                self._singletons[dep_type] = provided
            return cast(T, self._singletons[dep_type])

    def clear(self) -> None:
        self._singletons.clear()
        self._providers.clear()
        self._locks.clear()


@dataclass(frozen=True, slots=True)
class CompiledCommand:
    func: CommandFn
    validator: type[BaseModel] | None
    injections: dict[str, type[Any] | None]  # None stands for AppHandle.


class CommandRegistry:
    """Internal command execution only; no IPC or Multiplexer dependency."""

    def __init__(
        self,
        *,
        container: DependencyContainer | None = None,
        debug: bool = False,
        enable_tracing: bool = True,
    ) -> None:
        self._app_handle: AppHandle | None = None
        self._handlers: dict[str, CompiledCommand] = {}
        self._registered_keys: set[str] = set()
        self.container = container if container is not None else DependencyContainer()
        self._before: list[BeforeHook] = []
        self._after: list[AfterHook] = []
        self._debug = debug
        self._enable_tracing = enable_tracing

    def bind(self, app_handle: AppHandle) -> None:
        if self._app_handle is not None:
            raise RuntimeError("CommandRegistry already bound")
        self._app_handle = app_handle

    def before(self, hook: BeforeHook) -> BeforeHook:
        self._before.append(hook)
        return hook

    def after(self, hook: AfterHook) -> AfterHook:
        self._after.append(hook)
        return hook

    def command(self, name: str | None = None) -> Callable[[CommandFn], CommandFn]:
        def decorator(func: CommandFn) -> CommandFn:
            self.register(name if name is not None else func.__name__, func)
            return func

        return decorator

    def register(self, name: str, func: CommandFn) -> None:
        if not isinstance(name, str) or not name.strip():
            raise ValueError("Command name must be a non-empty string")
        if name.startswith("taurino."):
            raise ValueError("The 'taurino.' namespace is reserved")
        if name in self._registered_keys:
            raise ValueError(f"Duplicate command: {name}")

        compiled = self._compile(func)  # Compile before updating either collection.
        self._handlers[name] = compiled
        self._registered_keys.add(name)

    def registered_keys(self) -> set[str]:
        """A copy of the registered command names for later use."""
        return self._registered_keys.copy()

    def is_registered(self, name: str) -> bool:
        return name in self._registered_keys

    def list_commands(self) -> list[str]:
        return sorted(self._registered_keys)

    def describe(self) -> dict[str, Any]:
        return {
            name: {
                "handler": f"{compiled.func.__module__}.{compiled.func.__qualname__}",
                "async": inspect.iscoroutinefunction(compiled.func),
                "params": (
                    list(compiled.validator.model_fields) if compiled.validator is not None else []
                ),
                "injected": list(compiled.injections),
            }
            for name, compiled in self._handlers.items()
        }

    @staticmethod
    def _compile(func: CommandFn) -> CompiledCommand:
        from taurino.app_handle import AppHandle

        if not callable(func):
            raise TypeError("Command handler must be callable")

        signature = inspect.signature(func)
        hints = get_type_hints(func, include_extras=True)
        fields: dict[str, Any] = {}
        injections: dict[str, type[Any] | None] = {}

        for name, parameter in signature.parameters.items():
            if parameter.kind not in (
                inspect.Parameter.KEYWORD_ONLY,
                inspect.Parameter.POSITIONAL_OR_KEYWORD,
            ):
                raise TypeError(
                    "Commands only accept named arguments, not positional-only, *args or **kwargs"
                )

            hinted = hints.get(name, Any)
            dependency = False
            if get_origin(hinted) is Annotated:
                base, *annotations = get_args(hinted)
                dependency = any(isinstance(meta, Depends) for meta in annotations)
                hinted = base

            if hinted is AppHandle or dependency:
                if parameter.default is not inspect.Parameter.empty:
                    raise TypeError(f"Injected parameter '{name}' cannot have a default")
                if hinted is AppHandle:
                    injections[name] = None
                elif isinstance(hinted, type):
                    injections[name] = hinted
                else:
                    raise TypeError("Depends requires a concrete class")
                continue

            # JS commonly sends camelCase, while Python uses snake_case.
            # Both spellings are accepted; sending both is rejected as an extra field.
            parts = name.split("_")
            camel_name = parts[0] + "".join(part.capitalize() for part in parts[1:])
            default = ... if parameter.default is inspect.Parameter.empty else parameter.default
            if camel_name != name:
                fields[name] = (
                    hinted,
                    Field(
                        default=default,
                        validation_alias=AliasChoices(name, camel_name),
                    ),
                )
            else:
                fields[name] = (hinted, default)

        validator = (
            create_model(
                f"Command_{func.__name__}_Params",
                __config__=ConfigDict(extra="forbid"),
                **fields,
            )
            if fields
            else None
        )
        return CompiledCommand(func, validator, injections)

    async def dispatch(self, request: InvokeRequest | Mapping[str, Any]) -> InvokeResponse:
        """Run a JS invoke request and return a transport-neutral outcome.

        The transport must catch malformed *envelopes* that fail request validation.
        A valid request always yields a response, including handler failures.
        """
        if not isinstance(request, InvokeRequest):
            request = InvokeRequest.model_validate(request)

        started = time.perf_counter()
        payload: dict[str, Any] = {}
        response: InvokeResponse

        try:
            if request.payload is None:
                payload = {}
            elif isinstance(request.payload, dict):
                payload = request.payload
            else:
                raise CommandException(
                    CommandErrorCode.INVALID_PARAMS,
                    "Invoke payload must be a JSON object or null",
                )

            command = self._handlers.get(request.cmd)
            if command is None:
                raise CommandException(
                    CommandErrorCode.NO_HANDLER,
                    f"No command registered as '{request.cmd}'",
                )

            for hook in self._before:
                await self._invoke_hook(hook, request.cmd, payload)

            if command.validator is None:
                if payload:
                    raise CommandException(
                        CommandErrorCode.INVALID_PARAMS,
                        "This command accepts no payload parameters",
                    )
                bound: dict[str, Any] = {}
            else:
                try:
                    validated = command.validator.model_validate(payload)
                except ValidationError as exc:
                    raise CommandException(
                        CommandErrorCode.VALIDATION_ERROR,
                        "Command payload validation failed",
                        exc.errors(include_input=False, include_url=False),
                    ) from exc
                bound = {name: getattr(validated, name) for name in command.validator.model_fields}

            for name, dep_type in command.injections.items():
                if dep_type is None:
                    if self._app_handle is None:
                        raise CommandException(
                            CommandErrorCode.INTERNAL_ERROR,
                            "AppHandle is not bound",
                        )
                    bound[name] = self._app_handle
                else:
                    bound[name] = await self.container.resolve(dep_type)

            if inspect.iscoroutinefunction(command.func):
                value = await command.func(**bound)
            else:
                # Avoid blocking the Python IPC event loop.
                value = await asyncio.to_thread(command.func, **bound)
                if inspect.isawaitable(value):
                    value = await value

            # Ensure that Rust receives JSON-compatible data for JS callbacks.
            response = InvokeResponse(ok=True, data=to_jsonable_python(value))
        except CommandException as exc:
            response = self._error_response(exc.code, exc.message, exc.details)
        except Exception as exc:
            log.exception("Command '%s' failed", request.cmd)
            response = self._error_response(
                CommandErrorCode.EXECUTION_ERROR,
                "Command execution failed",
                str(exc) if self._debug else None,
            )

        if self._enable_tracing:
            log.debug(
                "Command '%s' completed (ok=%s, elapsed_ms=%.3f)",
                request.cmd,
                response.ok,
                (time.perf_counter() - started) * 1000,
            )

        for hook in self._after:
            try:
                await self._invoke_hook(
                    hook, request.cmd, payload, response.model_dump(mode="json")
                )
            except Exception:
                log.exception("Command after-hook failed")

        return response

    @staticmethod
    async def _invoke_hook(func: Callable[..., Any], *args: Any) -> None:
        if inspect.iscoroutinefunction(func):
            await func(*args)
        else:
            value = await asyncio.to_thread(func, *args)
            if inspect.isawaitable(value):
                await value

    @staticmethod
    def _error_response(
        code: CommandErrorCode,
        message: str,
        details: Any = None,
    ) -> InvokeResponse:
        return InvokeResponse(
            ok=False,
            data={
                "code": code.value,
                "message": message,
                "details": to_jsonable_python(details),
            },
        )

    def close(self) -> None:
        self.container.clear()
        self._handlers.clear()
        self._registered_keys.clear()
        self._before.clear()
        self._after.clear()
        self._app_handle = None
