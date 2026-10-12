from collections.abc import Callable
from typing import Any

def runtime(config: bytes, shutdown_event: Any, status: Callable[[], None]) -> None: ...
