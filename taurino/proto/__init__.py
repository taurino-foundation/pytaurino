from __future__ import annotations

from typing import Annotated

from pydantic import BaseModel, ConfigDict, Field
from pydantic.types import JsonValue

UInt32 = Annotated[
    int,
    Field(strict=True, ge=0, le=2**32 - 1),
]


class InvokeRequest(BaseModel):
    model_config = ConfigDict(
        extra="ignore",
        strict=True,
    )

    cmd: str
    callback: UInt32
    error: UInt32

    payload: JsonValue = None
    options: JsonValue | None = None
