"""Validated Pydantic configuration and safe Rust bootstrap serialization."""

from __future__ import annotations

from enum import Enum
from pathlib import Path
from typing import Any
from uuid import UUID, uuid4

import msgpack
from pydantic import BaseModel, ConfigDict, Field, field_validator, model_validator


def to_camel(name: str) -> str:
    parts = name.split("_")
    return parts[0] + "".join(p.title() for p in parts[1:])


class FrozenConfig(BaseModel):
    model_config = ConfigDict(
        frozen=True, populate_by_name=True, alias_generator=to_camel, extra="forbid"
    )


class WindowPosition(str, Enum):
    CENTER = "center"
    DEFAULT = "default"


class MenuItemType(str, Enum):
    ITEM = "item"
    SEPARATOR = "separator"
    SUBMENU = "submenu"


class IpcSecurity(str, Enum):
    NONE = "none"
    AUTHENTICATED = "authenticated"


class IpcConnectionConfig(FrozenConfig):
    endpoint: str = Field(
        default_factory=lambda: f"taurino-{uuid4().hex}", min_length=1
    )
    connect_timeout: float = Field(default=10.0, gt=0)
    operation_timeout: float = Field(default=30.0, gt=0)
    handshake_timeout: float = Field(default=10.0, gt=0)
    max_message_size: int = Field(default=1024 * 1024, ge=1024, le=64 * 1024 * 1024)
    max_clients: int = Field(default=1, ge=1, le=256)
    permissions: int | str | None = None
    security: IpcSecurity = IpcSecurity.NONE
    server_private_key_path: Path | None = None
    client_public_key: bytes | None = None
    client_private_key_path: Path | None = None

    @model_validator(mode="after")
    def ensure_security(self) -> "IpcConnectionConfig":
        if not self.endpoint.strip():
            raise ValueError("endpoint must not be blank")
        if self.security == IpcSecurity.AUTHENTICATED:
            if (
                not self.server_private_key_path
                or not self.client_private_key_path
                or self.client_public_key is None
            ):
                raise ValueError(
                    "authenticated IPC needs both identity key paths and the pinned client public key"
                )
            if len(self.client_public_key) != 32:
                raise ValueError(
                    "client_public_key must contain 32 raw Ed25519 public-key bytes"
                )
        return self

    def to_client_bootstrap(self) -> dict[str, Any]:
        """Never expose the Python server private key in the engine config."""
        out: dict[str, Any] = {
            "endpoint": self.endpoint,
            "connectTimeout": self.connect_timeout,
            "operationTimeout": self.operation_timeout,
            "handshakeTimeout": self.handshake_timeout,
            "maxMessageSize": self.max_message_size,
            "security": self.security.value,
        }
        if self.security == IpcSecurity.AUTHENTICATED:
            from taunicorn.security import identity_public_key

            assert self.server_private_key_path is not None
            assert self.client_private_key_path is not None
            server_private = self.server_private_key_path.read_bytes()
            if len(server_private) != 32:
                raise ValueError("server Ed25519 private key must be exactly 32 bytes")
            out["serverPublicKey"] = identity_public_key(server_private)
            out["clientPrivateKeyPath"] = str(self.client_private_key_path)
        return out


class WebViewConfig(FrozenConfig):
    url: str
    label: str = Field(min_length=1)
    x: int = 0
    y: int = 0
    width: int | None = Field(default=None, gt=0)
    height: int | None = Field(default=None, gt=0)
    visible: bool = True
    devtools: bool = False


class WindowConfig(FrozenConfig):
    label: str = Field(min_length=1)
    center: bool = True
    title: str = "Taurino"
    width: int = Field(default=1000, gt=0)
    height: int = Field(default=700, gt=0)
    min_width: int | None = Field(default=None, gt=0)
    min_height: int | None = Field(default=None, gt=0)
    resizable: bool = True
    decorations: bool = True
    transparent: bool = False
    visible: bool = True
    fullscreen: bool = False
    maximized: bool = False
    webviews: tuple[WebViewConfig, ...] = ()


class ShortcutConfig(FrozenConfig):
    accelerator: str
    action: str
    enabled: bool = True


class MenuItemConfig(FrozenConfig):
    id: str
    text: str = ""
    kind: MenuItemType = MenuItemType.ITEM
    enabled: bool = True
    accelerator: str | None = None
    children: tuple["MenuItemConfig", ...] = ()


class MenuConfig(FrozenConfig):
    items: tuple[MenuItemConfig, ...] = ()


class TrayIconConfig(FrozenConfig):
    id: str
    icon: str | None = None
    tooltip: str | None = None
    visible: bool = True
    menu: MenuConfig | None = None


class Config(FrozenConfig):
    productname: str = Field(min_length=1)
    version: str = Field(min_length=1)
    app_id: UUID
    connection: IpcConnectionConfig = Field(
        default_factory=IpcConnectionConfig, exclude=True
    )
    windows: tuple[WindowConfig, ...] = ()
    shortcuts: tuple[ShortcutConfig, ...] = ()
    trays: tuple[TrayIconConfig, ...] = ()
    app_menu: MenuConfig | None = None
    schema_version: int = Field(default=1, ge=1, le=1)

    @field_validator("productname", "version")
    @classmethod
    def non_blank(cls, value: str) -> str:
        if not value.strip():
            raise ValueError("must not be blank")
        return value

    @model_validator(mode="after")
    def unique_labels(self) -> "Config":
        win_labels = [w.label for w in self.windows]
        view_labels = [v.label for w in self.windows for v in w.webviews]
        if len(set(win_labels)) != len(win_labels):
            raise ValueError("Duplicate window labels")
        if len(set(view_labels)) != len(view_labels):
            raise ValueError("Duplicate WebView labels")
        return self

    def to_engine_bytes(self) -> bytes:
        """Serialize actual app config plus the *client-only* IPC bootstrap using MessagePack.

        Rust must use a compatible serde naming scheme and this versioned schema.
        No server-side private identity key is transferred.
        """
        data = self.model_dump(mode="json", by_alias=True)
        data["connection"] = self.connection.to_client_bootstrap()
        return msgpack.packb(data, use_bin_type=True)
