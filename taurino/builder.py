"""Public declarative Builder: configuration, commands and lifecycle hooks."""

from __future__ import annotations

from collections.abc import Callable
from typing import Any, Self

from taurino.app import App

Hook = Callable[..., Any]


class Builder:
    def __init__(self) -> None:
        self._setup_hook: Hook | None = None

        self._loop_event_hook: Hook | None = None
        self._window_event_hook: Hook | None = None
        self._webview_event_hook: Hook | None = None
        self._systemtray_event_hook: Hook | None = None
        self._shortcut_event_hook: Hook | None = None

        self._commands: dict[str, Hook] = {}
        self._built = False

    def _ensure_mutable(self) -> None:
        if self._built:
            raise RuntimeError("Builder is already built")

    def _register_hook(self, attribute: str, hook: Hook) -> Self:
        self._ensure_mutable()

        if not callable(hook):
            raise TypeError(f"{attribute} must be callable")

        if getattr(self, attribute) is not None:
            raise RuntimeError(f"{attribute} already registered")

        setattr(self, attribute, hook)
        return self

    def setup(self, hook: Hook) -> Self:
        return self._register_hook("_setup_hook", hook)

    def on_loop_event(self, hook: Hook) -> Self:
        return self._register_hook("_loop_event_hook", hook)

    def on_window_event(self, hook: Hook) -> Self:
        return self._register_hook("_window_event_hook", hook)

    def on_webview_event(self, hook: Hook) -> Self:
        return self._register_hook("_webview_event_hook", hook)

    def on_systemtray_event(self, hook: Hook) -> Self:
        return self._register_hook("_systemtray_event_hook", hook)

    def on_shortcut_event(self, hook: Hook) -> Self:
        return self._register_hook("_shortcut_event_hook", hook)

    def command(
        self,
        name: str | None = None,
    ) -> Callable[[Hook], Hook]:
        def decorator(fn: Hook) -> Hook:
            self._ensure_mutable()

            if not callable(fn):
                raise TypeError("Command must be callable")

            command_name = name if name is not None else fn.__name__

            if not isinstance(command_name, str) or not command_name.strip():
                raise ValueError("Command name cannot be empty")

            if command_name in self._commands:
                raise ValueError(f"Duplicate command: {command_name}")

            self._commands[command_name] = fn
            return fn

        return decorator

    def build(self) -> App:
        self._ensure_mutable()

        app = App(
            commands=dict(self._commands),
            setup_hook=self._setup_hook,
            loop_event_hook=self._loop_event_hook,
            window_event_hook=self._window_event_hook,
            webview_event_hook=self._webview_event_hook,
            systemtray_event_hook=self._systemtray_event_hook,
            shortcut_event_hook=self._shortcut_event_hook,
        )

        self._built = True
        return app
