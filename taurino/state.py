"""Thread-safe typed application state."""

from __future__ import annotations

import threading
from collections.abc import Callable
from typing import Any, TypeVar, cast

from .error import StateAlreadyExistsError, StateNotFoundError

T = TypeVar("T")


class State:
    """Stores one object per *exact* Python type; a registry, not a deep-copy store."""

    def __init__(self) -> None:
        self._values: dict[type[Any], Any] = {}
        self._lock = threading.RLock()

    def set(self, value: T, *, replace: bool = True) -> T:
        key = type(value)
        with self._lock:
            if not replace and key in self._values:
                raise StateAlreadyExistsError(f"State already exists: {key.__name__}")
            self._values[key] = value
        return value

    def get(self, state_type: type[T]) -> T:
        with self._lock:
            try:
                return cast(T, self._values[state_type])
            except KeyError:
                raise StateNotFoundError(
                    f"State not found: {state_type.__name__}"
                ) from None

    def try_get(self, state_type: type[T]) -> T | None:
        with self._lock:
            return cast(T | None, self._values.get(state_type))

    def contains(self, state_type: type[Any]) -> bool:
        with self._lock:
            return state_type in self._values

    def remove(self, state_type: type[T]) -> T:
        with self._lock:
            try:
                return cast(T, self._values.pop(state_type))
            except KeyError:
                raise StateNotFoundError(
                    f"State not found: {state_type.__name__}"
                ) from None

    def get_or_create(self, state_type: type[T], factory: Callable[[], T]) -> T:
        with self._lock:
            if state_type not in self._values:
                value = factory()
                if not isinstance(value, state_type):
                    raise TypeError("Factory returned an incompatible type")
                self._values[state_type] = value
            return cast(T, self._values[state_type])

    def update(self, state_type: type[T], updater: Callable[[T], T]) -> T:
        with self._lock:
            updated = updater(self.get(state_type))
            if not isinstance(updated, state_type):
                raise TypeError("Updater returned an incompatible type")
            self._values[state_type] = updated
            return updated

    def clear(self) -> None:
        with self._lock:
            self._values.clear()

    def snapshot(self) -> dict[type[Any], Any]:
        with self._lock:
            return self._values.copy()

    def __len__(self) -> int:
        with self._lock:
            return len(self._values)
