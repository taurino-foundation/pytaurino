"""Internal command registry with explicit dependency injection and Pydantic validation."""

from __future__ import annotations

import asyncio
import inspect
import json
import logging
import time
from collections.abc import Awaitable, Callable
from dataclasses import dataclass
from enum import Enum
from typing import (
    TYPE_CHECKING,
    Annotated,
    Any,
    TypeVar,
    cast,
    get_args,
    get_origin,
    get_type_hints,
)

from pydantic import BaseModel, ConfigDict, ValidationError, create_model

if TYPE_CHECKING:
    from taurino.app_handle import AppHandle

log = logging.getLogger(__name__)
T = TypeVar("T")
CommandFn = Callable[..., Any]
Provider = Callable[[], Any]
BeforeHook = Callable[[str, dict[str, Any]], Any]
AfterHook = Callable[[str, dict[str, Any], dict[str, Any]], Any]


class CommandErrorCode(str, Enum):
    NO_HANDLER = "NO_HANDLER"
    INVALID_PARAMS = "INVALID_PARAMS"
    VALIDATION_ERROR = "VALIDATION_ERROR"
    DEPENDENCY_ERROR = "DEPENDENCY_ERROR"
    EXECUTION_ERROR = "EXECUTION_ERROR"
    INTERNAL_ERROR = "INTERNAL_ERROR"


class CommandException(Exception):
    def __init__(self, code: CommandErrorCode, message: str, details: Any = None):
        super().__init__(message)
        self.code, self.message, self.details = code, message, details


@dataclass(frozen=True, slots=True)
class Depends:
    """Use Annotated[T, Depends()] for explicit dependency injection."""


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
        self, dep_type: type[T], provider: Callable[[], T | Awaitable[T]]
    ) -> None:
        self._providers[dep_type] = provider

    async def resolve(self, dep_type: type[T]) -> T:
        if dep_type in self._singletons:
            return cast(T, self._singletons[dep_type])
        if dep_type not in self._providers:
            raise CommandException(
                CommandErrorCode.DEPENDENCY_ERROR,
                f"No provider for {dep_type.__name__}",
            )
        lock = self._locks.setdefault(dep_type, asyncio.Lock())
        async with lock:
            if dep_type not in self._singletons:
                provider = self._providers[dep_type]
                value = provider()
                if inspect.isawaitable(value):
                    value = await value
                if not isinstance(value, dep_type):
                    raise CommandException(
                        CommandErrorCode.DEPENDENCY_ERROR,
                        f"Provider returned wrong type for {dep_type.__name__}",
                    )
                self._singletons[dep_type] = value
            return cast(T, self._singletons[dep_type])

    def clear(self) -> None:
        self._singletons.clear()
        self._providers.clear()
        self._locks.clear()


@dataclass(frozen=True)
class CompiledCommand:
    func: CommandFn
    validator: type[BaseModel] | None
    injections: dict[str, type[Any] | None]  # None means AppHandle


class CommandRegistry:
    """Command handlers only. No IPC or Multiplexer dependency."""

    def __init__(
        self,
        *,
        container: DependencyContainer | None = None,
        debug: bool = False,
        enable_tracing: bool = True,
    ):
        self._app_handle: "AppHandle | None" = None
        self._handlers: dict[str, CompiledCommand] = {}
        self.container = container or DependencyContainer()
        self._before: list[BeforeHook] = []
        self._after: list[AfterHook] = []
        self._debug = debug
        self._enable_tracing = enable_tracing

    def bind(self, app_handle: "AppHandle") -> None:
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
            self.register(name or func.__name__, func)
            return func

        return decorator

    def register(self, name: str, func: CommandFn) -> None:
        if not name.strip() or name.startswith("taurino."):
            raise ValueError(
                "Command name must be nonblank and must not use the reserved 'taurino.' prefix"
            )
        if name in self._handlers:
            raise ValueError(f"Duplicate command: {name}")
        self._handlers[name] = self._compile(func)

    @staticmethod
    def _compile(func: CommandFn) -> CompiledCommand:
        from taurino.app_handle import AppHandle

        signature = inspect.signature(func)
        hints = get_type_hints(func, include_extras=True)
        fields: dict[str, tuple[Any, Any]] = {}
        injections: dict[str, type[Any] | None] = {}
        for name, parameter in signature.parameters.items():
            if parameter.kind not in (
                inspect.Parameter.KEYWORD_ONLY,
                inspect.Parameter.POSITIONAL_OR_KEYWORD,
            ):
                raise TypeError(
                    "Commands only accept named arguments, not *args, **kwargs or positional-only args"
                )
            hinted = hints.get(name, Any)
            dependency = False
            if get_origin(hinted) is Annotated:
                base, *annotations = get_args(hinted)
                dependency = any(isinstance(meta, Depends) for meta in annotations)
                hinted = base
            if hinted is AppHandle:
                injections[name] = None
            elif dependency:
                if not isinstance(hinted, type):
                    raise TypeError("Depends requires a concrete class")
                injections[name] = hinted
            else:
                fields[name] = (
                    hinted,
                    (
                        ...
                        if parameter.default is inspect.Parameter.empty
                        else parameter.default
                    ),
                )
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

    def list_commands(self) -> list[str]:
        return sorted(self._handlers)

    def describe(self) -> dict[str, Any]:
        return {
            name: {
                "handler": f"{item.func.__module__}.{item.func.__qualname__}",
                "async": inspect.iscoroutinefunction(item.func),
                "params": list(item.validator.model_fields) if item.validator else [],
                "injected": list(item.injections),
            }
            for name, item in self._handlers.items()
        }

    async def dispatch(
        self, event: str, *, result_id: int, error_id: int, data: dict[str, Any]
    ) -> dict[str, Any]:
        started = time.perf_counter()
        result: dict[str, Any]
        try:
            if not isinstance(data, dict):
                raise CommandException(
                    CommandErrorCode.INVALID_PARAMS, "Command data must be a mapping"
                )
            for hook in self._before:
                await self._invoke_hook(hook, event, data)
            command = self._handlers.get(event)
            if command is None:
                raise CommandException(
                    CommandErrorCode.NO_HANDLER, f"No handler for '{event}'"
                )
            if command.validator is not None:
                try:
                    validated = command.validator.model_validate(data)
                except ValidationError as exc:
                    raise CommandException(
                        CommandErrorCode.VALIDATION_ERROR,
                        "Validation failed",
                        exc.errors(include_input=False, include_url=False),
                    ) from exc
                bound = {
                    name: getattr(validated, name)
                    for name in command.validator.model_fields
                }
            elif data:
                raise CommandException(
                    CommandErrorCode.INVALID_PARAMS,
                    "This command accepts no user parameters",
                )
            else:
                bound = {}
            for name, dep_type in command.injections.items():
                if dep_type is None:
                    if self._app_handle is None:
                        raise CommandException(
                            CommandErrorCode.INTERNAL_ERROR, "AppHandle not bound"
                        )
                    bound[name] = self._app_handle
                else:
                    bound[name] = await self.container.resolve(dep_type)
            if inspect.iscoroutinefunction(command.func):
                value = await command.func(**bound)
            else:
                # Potentially blocking synchronous handlers do not execute on the IPC loop.
                value = await asyncio.to_thread(command.func, **bound)
                if inspect.isawaitable(value):
                    value = await value
            result = {"result_id": result_id, "result": value}
        except CommandException as exc:
            result = self._error(error_id, exc.code, exc.message, exc.details)
        except Exception as exc:
            log.exception("Command '%s' failed", event)
            result = self._error(
                error_id,
                CommandErrorCode.EXECUTION_ERROR,
                "Command execution failed",
                str(exc) if self._debug else None,
            )
        if self._enable_tracing:
            result["meta"] = {
                "event": event,
                "duration_ms": round((time.perf_counter() - started) * 1000, 3),
            }
        for hook in self._after:
            try:
                await self._invoke_hook(hook, event, data, result)
            except Exception:
                log.exception("After-hook failed")
        return result

    @staticmethod
    async def _invoke_hook(func: Callable[..., Any], *args: Any) -> None:
        if inspect.iscoroutinefunction(func):
            await func(*args)
        else:
            value = await asyncio.to_thread(func, *args)
            if inspect.isawaitable(value):
                await value

    @staticmethod
    def _error(
        error_id: int, code: CommandErrorCode, message: str, details: Any
    ) -> dict[str, Any]:
        out: dict[str, Any] = {
            "error_id": error_id,
            "error": f"[{code.value}] {message}",
            "error_obj": {"code": code.value, "message": message, "details": details},
        }
        return out

    def close(self) -> None:
        self.container.clear()
        self._handlers.clear()


def build_send_message_script(msg: dict[str, Any]) -> str:
    """Preserve the original callback bridge helper; only use in a trusted engine context."""
    encoded = json.dumps(msg, separators=(",", ":"), ensure_ascii=False)
    literal = json.dumps(encoded, separators=(",", ":"), ensure_ascii=False)
    return (
        f"window.__TAURINO_RUNCALLBACK__ && window.__TAURINO_RUNCALLBACK__({literal});"
    )
