"""Public declarative Builder: configuration, commands and lifecycle hooks."""

from __future__ import annotations

from collections.abc import Callable
from typing import Any, overload

from taurino.app import App

from .config import Config

Hook = Callable[..., Any]


class Builder:
    def __init__(self, config: Config):
        self.config = config
        self._setup_hook: Hook | None = None
        self._event_loop_hook: Hook | None = None
        self._commands: dict[str, Hook] = {}
        self._built = False

    def command(self, name: str | None = None) -> Callable[[Hook], Hook]:
        """Register a command without exposing the CommandRegistry."""

        def decorator(fn: Hook) -> Hook:
            if self._built:
                raise RuntimeError("Builder is already built")
            command_name = name or fn.__name__
            if command_name in self._commands:
                raise ValueError(f"Duplicate command: {command_name}")
            self._commands[command_name] = fn
            return fn

        return decorator

    @overload
    def setup(self, hook: Hook) -> Hook: ...

    @overload
    def setup(self) -> Callable[[Hook], Hook]: ...

    def setup(self, hook: Hook | None = None) -> Hook | Callable[[Hook], Hook]:
        def register(fn: Hook) -> Hook:
            if self._built or self._setup_hook is not None or not callable(fn):
                raise RuntimeError(
                    "Setup hook already registered, invalid, or builder is built"
                )
            self._setup_hook = fn
            return fn

        return register(hook) if hook is not None else register

    @overload
    def on_event(self, hook: Hook) -> Hook: ...

    @overload
    def on_event(self) -> Callable[[Hook], Hook]: ...

    def on_event(self, hook: Hook | None = None) -> Hook | Callable[[Hook], Hook]:
        def register(fn: Hook) -> Hook:
            if self._built or self._event_loop_hook is not None or not callable(fn):
                raise RuntimeError(
                    "Event-loop hook already registered, invalid, or builder is built"
                )
            self._event_loop_hook = fn
            return fn

        return register(hook) if hook is not None else register

    # Alias to match the original pasted Builder.run event-hook decorator.
    run = on_event

    def build(self) -> "App":
        """Build exactly one App; native bridge target is optional but must exist at runtime."""
        from .app import App

        if self._built:
            raise RuntimeError("Builder already used")
        app = App(
            config=self.config,
            commands=dict(self._commands),
            setup_hook=self._setup_hook,
            event_hook=self._event_loop_hook,
        )
        self._built = True
        return app
