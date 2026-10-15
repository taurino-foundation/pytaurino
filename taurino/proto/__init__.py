"""Pydantic v2 mirror of the Serde configurations supplied for Taurino.

Models use Python snake_case, but serialize as Rust-compatible camelCase.
External Rust types absent from the supplied source are documented below.
"""

from __future__ import annotations

import json
import math
import re
from enum import IntEnum, StrEnum
from pathlib import Path
from typing import Annotated, Any, Literal
from urllib.parse import urlsplit
from uuid import UUID, uuid4

import msgpack
from pydantic import (
    BaseModel,
    ConfigDict,
    Field,
    field_validator,
    model_validator,
)
from pydantic.alias_generators import to_camel
from pydantic.types import JsonValue


class SerdeModel(BaseModel):
    model_config = ConfigDict(
        alias_generator=to_camel,
        populate_by_name=True,
        extra="forbid",
        validate_assignment=True,
    )

    def to_wire(self) -> dict[str, Any]:
        return self.model_dump(mode="json", by_alias=True, exclude_none=False)

    def to_msgpack(self) -> bytes:
        packed = msgpack.packb(self.to_wire(), use_bin_type=True)

        if packed is None:
            raise RuntimeError("MessagePack serialization failed")

        return packed


def _omit_none(data: dict[str, Any]) -> dict[str, Any]:
    return {k: v for k, v in data.items() if v is not None}


def _url(value: str, *, schemes: set[str] | None = None) -> str:
    if not isinstance(value, str) or not value:
        raise ValueError("Expected a non-empty URL")
    scheme = urlsplit(value).scheme.lower()
    if not scheme or (schemes is not None and scheme not in schemes):
        raise ValueError(f"Invalid URL scheme: {scheme!r}")
    if scheme in {"http", "https", "socks5"} and not urlsplit(value).hostname:
        raise ValueError("URL must include a host")
    return value


U8 = Annotated[int, Field(ge=0, le=255)]
U16 = Annotated[int, Field(ge=0, le=65535)]
U32 = Annotated[int, Field(ge=0, le=4294967295)]
U64 = Annotated[int, Field(ge=0, le=18446744073709551615)]


class BackgroundThrottlingPolicy(StrEnum):
    DISABLED = "disabled"
    SUSPEND = "suspend"
    THROTTLE = "throttle"


class ScrollBarStyle(StrEnum):
    DEFAULT = "default"
    FLUENT_OVERLAY = "fluentOverlay"


class WebContentProcessTerminatePolicy(StrEnum):
    RELOAD = "reload"
    IGNORE = "ignore"


class TitleBarStyle(StrEnum):
    VISIBLE = "Visible"
    TRANSPARENT = "Transparent"
    OVERLAY = "Overlay"

    @classmethod
    def _missing_(cls, value: object):
        if isinstance(value, str):
            return next((v for v in cls if v.value.lower() == value.lower()), cls.VISIBLE)
        return cls.VISIBLE


class Theme(StrEnum):
    LIGHT = "light"
    DARK = "dark"

    @classmethod
    def _missing_(cls, value: object):
        if isinstance(value, str):
            return cls.DARK if value.lower() == "dark" else cls.LIGHT
        return None


class WindowEffect(StrEnum):
    APPEARANCE_BASED = "appearanceBased"
    LIGHT = "light"
    DARK = "dark"
    MEDIUM_LIGHT = "mediumLight"
    ULTRA_DARK = "ultraDark"
    TITLEBAR = "titlebar"
    SELECTION = "selection"
    MENU = "menu"
    POPOVER = "popover"
    SIDEBAR = "sidebar"
    HEADER_VIEW = "headerView"
    SHEET = "sheet"
    WINDOW_BACKGROUND = "windowBackground"
    HUD_WINDOW = "hudWindow"
    FULL_SCREEN_UI = "fullScreenUI"
    TOOLTIP = "tooltip"
    CONTENT_BACKGROUND = "contentBackground"
    UNDER_WINDOW_BACKGROUND = "underWindowBackground"
    UNDER_PAGE_BACKGROUND = "underPageBackground"
    LIQUID_GLASS_REGULAR = "liquidGlassRegular"
    LIQUID_GLASS_CLEAR = "liquidGlassClear"
    MICA = "mica"
    MICA_DARK = "micaDark"
    MICA_LIGHT = "micaLight"
    TABBED = "tabbed"
    TABBED_DARK = "tabbedDark"
    TABBED_LIGHT = "tabbedLight"
    BLUR = "blur"
    ACRYLIC = "acrylic"


class WindowEffectState(StrEnum):
    FOLLOWS_WINDOW_ACTIVE_STATE = "followsWindowActiveState"
    ACTIVE = "active"
    INACTIVE = "inactive"


class ProgressBarStatus(StrEnum):
    NONE = "none"
    NORMAL = "normal"
    INDETERMINATE = "indeterminate"
    PAUSED = "paused"
    ERROR = "error"


class ResizeDirection(StrEnum):
    EAST = "East"
    NORTH = "North"
    NORTH_EAST = "NorthEast"
    NORTH_WEST = "NorthWest"
    SOUTH = "South"
    SOUTH_EAST = "SouthEast"
    SOUTH_WEST = "SouthWest"
    WEST = "West"


class ActivationPolicy(StrEnum):
    REGULAR = "Regular"
    ACCESSORY = "Accessory"
    PROHIBITED = "Prohibited"


class CursorIcon(StrEnum):
    DEFAULT = "default"
    CROSSHAIR = "crosshair"
    HAND = "hand"
    ARROW = "arrow"
    MOVE = "move"
    TEXT = "text"
    WAIT = "wait"
    HELP = "help"
    PROGRESS = "progress"
    NOT_ALLOWED = "notallowed"
    CONTEXT_MENU = "contextmenu"
    CELL = "cell"
    VERTICAL_TEXT = "verticaltext"
    ALIAS = "alias"
    COPY = "copy"
    NO_DROP = "nodrop"
    GRAB = "grab"
    GRABBING = "grabbing"
    ALL_SCROLL = "allscroll"
    ZOOM_IN = "zoomin"
    ZOOM_OUT = "zoomout"
    E_RESIZE = "eresize"
    N_RESIZE = "nresize"
    NE_RESIZE = "neresize"
    NW_RESIZE = "nwresize"
    S_RESIZE = "sresize"
    SE_RESIZE = "seresize"
    SW_RESIZE = "swresize"
    W_RESIZE = "wresize"
    EW_RESIZE = "ewresize"
    NS_RESIZE = "nsresize"
    NESW_RESIZE = "neswresize"
    NWSE_RESIZE = "nwseresize"
    COL_RESIZE = "colresize"
    ROW_RESIZE = "rowresize"

    @classmethod
    def _missing_(cls, value: object):
        # Rust's custom Deserialize defaults to Default for unrecognised strings.
        if isinstance(value, str):
            return next((v for v in cls if v.value == value.lower()), cls.DEFAULT)
        return None


class UserAttentionCritical(SerdeModel):
    type: Literal["Critical"] = "Critical"


class UserAttentionInformational(SerdeModel):
    type: Literal["Informational"] = "Informational"


UserAttentionType = Annotated[UserAttentionCritical | UserAttentionInformational, Field(discriminator="type")]


class DeviceEventsAlways(SerdeModel):
    type: Literal["Always"] = "Always"


class DeviceEventsUnfocused(SerdeModel):
    type: Literal["Unfocused"] = "Unfocused"


class DeviceEventsNever(SerdeModel):
    type: Literal["Never"] = "Never"


DeviceEventFilter = Annotated[
    DeviceEventsAlways | DeviceEventsUnfocused | DeviceEventsNever, Field(discriminator="type")
]


class ProgressBarState(SerdeModel):
    status: ProgressBarStatus | None = None
    progress: U64 | None = None
    desktop_filename: str | None = None


class WindowSizeConstraints(SerdeModel):
    # dpi::PixelUnit: external dependency, encoded as a numeric logical/physical unit.
    # See caveats in README about exact dpi enum representation.
    min_width: Any = None
    min_height: Any = None
    max_width: Any = None
    max_height: Any = None


class WebviewBounds(SerdeModel):
    # Rust uses snake_case here (no rename_all) and this type is Serialize only.
    x_rate: float
    y_rate: float
    width_rate: float
    height_rate: float

    def to_wire(self) -> dict[str, Any]:
        return self.model_dump(mode="json", by_alias=False)


class Icon(SerdeModel):
    rgba: bytes
    width: U32
    height: U32

    def to_wire(self) -> dict[str, Any]:
        return self.model_dump(by_alias=False)


class NavigationRule(SerdeModel):
    host: str
    scheme: str | None = None
    port: U16 | None = None
    include_subdomains: bool = False


class AllowAllNavigation(SerdeModel):
    type: Literal["allowAll"] = "allowAll"


class DenyAllNavigation(SerdeModel):
    type: Literal["denyAll"] = "denyAll"


class SameOriginNavigation(SerdeModel):
    type: Literal["sameOrigin"] = "sameOrigin"


class AllowListNavigation(SerdeModel):
    type: Literal["allowList"] = "allowList"
    rules: list[NavigationRule]


class BlockListNavigation(SerdeModel):
    type: Literal["blockList"] = "blockList"
    rules: list[NavigationRule]


NavigationPolicy = Annotated[
    AllowAllNavigation | DenyAllNavigation | SameOriginNavigation | AllowListNavigation | BlockListNavigation,
    Field(discriminator="type"),
]


class NewWindowRequestMatcher(SerdeModel):
    scheme: str | None = None
    host: str | None = None
    include_subdomains: bool = False
    port: U16 | None = None
    path_prefix: str | None = None


class AllowWindowAction(SerdeModel):
    type: Literal["allow"] = "allow"


class DenyWindowAction(SerdeModel):
    type: Literal["deny"] = "deny"


class CreateWindowAction(SerdeModel):
    type: Literal["create"] = "create"
    window: WindowConfig


NewWindowAction = Annotated[AllowWindowAction | DenyWindowAction | CreateWindowAction, Field(discriminator="type")]


class NewWindowRule(SerdeModel):
    request: NewWindowRequestMatcher
    response: NewWindowAction


class AllowNewWindow(SerdeModel):
    type: Literal["allow"] = "allow"


class DenyNewWindow(SerdeModel):
    type: Literal["deny"] = "deny"


class RulesNewWindow(SerdeModel):
    type: Literal["rules"] = "rules"
    rules: list[NewWindowRule]
    fallback: NewWindowAction = Field(default_factory=DenyWindowAction)


NewWindowPolicy = Annotated[AllowNewWindow | DenyNewWindow | RulesNewWindow, Field(discriminator="type")]


class PermissionRequestPolicy(SerdeModel):
    # Exact Rust struct is empty; any field is rejected.
    pass


class InitializationScript(SerdeModel):
    # Rust has no camelCase attribute on this struct.
    script: str
    for_main_frame_only: bool

    def to_wire(self) -> dict[str, Any]:
        return self.model_dump(mode="json", by_alias=False)


class PreventOverflowMargin(SerdeModel):
    width: U32
    height: U32


PreventOverflowConfig = bool | PreventOverflowMargin


# Rust Color custom-deserializes from hex, RGB/RGBA arrays, or an RGBA object.
# Rust Serializes Color as four integer values. Normalize to a 4-element tuple.
Color = tuple[U8, U8, U8, U8]


def parse_color(value: object) -> tuple[int, int, int, int]:
    def channel(v: object) -> int:
        if type(v) is not int or not 0 <= v <= 255:
            raise ValueError("Color channels must be integers between 0 and 255")
        return v

    if isinstance(value, str):
        h = value.strip().removeprefix("#")

        if len(h) == 3:
            h = "".join(c * 2 for c in h) + "ff"
        elif len(h) == 6:
            h += "ff"
        elif len(h) != 8:
            raise ValueError("Hex color must have 3, 6 or 8 digits")

        try:
            raw = bytes.fromhex(h)
        except ValueError as exc:
            raise ValueError("Invalid hex color") from exc

        return raw[0], raw[1], raw[2], raw[3]

    if isinstance(value, dict):
        if set(value) - {"red", "green", "blue", "alpha"}:
            raise ValueError("Unexpected color key")

        return (
            channel(value.get("red")),
            channel(value.get("green")),
            channel(value.get("blue")),
            channel(value.get("alpha", 255)),
        )

    if isinstance(value, (list, tuple)):
        if len(value) == 3:
            return (
                channel(value[0]),
                channel(value[1]),
                channel(value[2]),
                255,
            )

        if len(value) == 4:
            return (
                channel(value[0]),
                channel(value[1]),
                channel(value[2]),
                channel(value[3]),
            )

    raise ValueError("Invalid color representation")


class WindowEffectsConfig(SerdeModel):
    effects: list[WindowEffect]
    state: WindowEffectState | None = None
    radius: float | None = None
    color: Color | None = None
    interactive: bool = False

    @field_validator("color", mode="before")
    @classmethod
    def normalize_color(cls, v: Any):
        return None if v is None else parse_color(v)

    def to_wire(self) -> dict[str, Any]:
        return _omit_none(super().to_wire())


class WebViewConfig(SerdeModel):
    label: str = "root"
    child: bool = False
    auto_resize: bool = False  # Serde missing field -> false; Rust Default impl -> true
    url: str = "index.html"
    user_agent: str | None = None
    drag_drop_enabled: bool = True
    additional_browser_args: str | None = None
    incognito: bool = False
    proxy_url: str | None = None
    zoom_hotkeys_enabled: bool = False
    browser_extensions_enabled: bool = False
    use_https_scheme: bool = False
    devtools: bool | None = None
    background_throttling: BackgroundThrottlingPolicy | None = None
    javascript_disabled: bool = False
    allow_link_preview: bool = True
    disable_input_accessory_view: bool = False
    data_directory: str | None = None
    data_store_identifier: tuple[U8, U8, U8, U8, U8, U8, U8, U8, U8, U8, U8, U8, U8, U8, U8, U8] | None = None
    scroll_bar_style: ScrollBarStyle = ScrollBarStyle.DEFAULT
    limit_navigations_to_app_bound_domains: bool = False
    activity_name: str | None = None
    created_by_activity_name: str | None = None
    requested_by_scene_identifier: str | None = None
    general_autofill_enabled: bool = True
    enable_clipboard_access: bool = True  # Serde default_true; Rust Default impl uses false
    navigation_policy: NavigationPolicy | None = None
    new_window_policy: NewWindowPolicy | None = None
    permission_request_policy: PermissionRequestPolicy | None = None
    web_content_process_terminate_policy: WebContentProcessTerminatePolicy | None = None
    bounds: dict[str, Any] | None = None  # dpi::Position/Size representation not in input

    @model_validator(mode="before")
    @classmethod
    def accept_serde_aliases(cls, raw: Any):
        return _rename_aliases(raw, cls)

    @field_validator("url")
    @classmethod
    def validate_url(cls, v: str):
        if ":" in v.split("/")[0]:
            _url(v)
        return v

    @field_validator("proxy_url")
    @classmethod
    def validate_proxy(cls, v: str | None):
        return _url(v, schemes={"http", "socks5"}) if v is not None else v

    def to_wire(self) -> dict[str, Any]:
        return _omit_none(super().to_wire())


class WindowConfig(SerdeModel):
    label: str = "main"
    create: bool = True
    x: float | None = None
    y: float | None = None
    webviews: list[WebViewConfig] = Field(default_factory=list)
    center: bool = False
    width: float = 800.0
    height: float = 600.0
    min_width: float | None = None
    min_height: float | None = None
    max_width: float | None = None
    max_height: float | None = None
    prevent_overflow: PreventOverflowConfig | None = None
    resizable: bool = True
    maximizable: bool = True
    minimizable: bool = True
    closable: bool = True
    title: str = "Taurino App"
    fullscreen: bool = False
    focus: bool = True
    focusable: bool = True
    transparent: bool = False
    maximized: bool = False
    visible: bool = True
    decorations: bool = True
    always_on_bottom: bool = False
    always_on_top: bool = False
    visible_on_all_workspaces: bool = False
    content_protected: bool = False
    skip_taskbar: bool = False
    window_classname: str | None = None
    no_redirection_bitmap: bool = False
    theme: Theme | None = None
    title_bar_style: TitleBarStyle = TitleBarStyle.VISIBLE
    traffic_light_position: dict[str, Any] | None = None  # dpi::LogicalPosition shape external
    hidden_title: bool = False
    accept_first_mouse: bool = False
    tabbing_identifier: str | None = None
    shadow: bool = True
    window_effects: WindowEffectsConfig | None = None
    parent: str | None = None
    background_color: Color | None = None
    enable_drag_drop: bool = True  # Serde default_true; Rust Default impl uses false

    @model_validator(mode="before")
    @classmethod
    def accept_serde_aliases(cls, raw: Any):
        return _rename_aliases(raw, cls)

    @field_validator("background_color", mode="before")
    @classmethod
    def normalize_color(cls, v: Any):
        return None if v is None else parse_color(v)

    def to_wire(self) -> dict[str, Any]:
        data = _omit_none(super().to_wire())
        data["webviews"] = [wv.to_wire() for wv in self.webviews]
        if self.window_effects is not None:
            data["windowEffects"] = self.window_effects.to_wire()
        return data


SERDE_EXTRA_ALIASES = {
    "product_name": ("product-name",),
    "dev_url": ("dev-url",),
    "user_agent": ("user-agent",),
    "drag_drop_enabled": ("drag-drop-enabled",),
    "additional_browser_args": ("additional-browser-args",),
    "proxy_url": ("proxy-url",),
    "zoom_hotkeys_enabled": ("zoom-hotkeys-enabled",),
    "browser_extensions_enabled": ("browser-extensions-enabled",),
    "use_https_scheme": ("use-https-scheme",),
    "background_throttling": ("background-throttling",),
    "javascript_disabled": ("javascript-disabled",),
    "allow_link_preview": ("allow-link-preview",),
    "disable_input_accessory_view": (
        "disable-input-accessory-view",
        "disable_input_accessory_view",
    ),
    "data_directory": ("data-directory",),
    "data_store_identifier": ("data-store-identifier",),
    "scroll_bar_style": ("scroll-bar-style",),
    "limit_navigations_to_app_bound_domains": ("limit-navigations-to-app-bound-domains",),
    "activity_name": ("activity-name",),
    "created_by_activity_name": ("created-by-activity-name",),
    "requested_by_scene_identifier": ("requested-by-scene-identifier",),
    "general_autofill_enabled": ("general-autofill-enabled",),
    "enable_clipboard_access": ("enable-clipboard-access",),
    "min_width": ("min-width",),
    "min_height": ("min-height",),
    "max_width": ("max-width",),
    "max_height": ("max-height",),
    "prevent_overflow": ("prevent-overflow",),
    "always_on_bottom": ("always-on-bottom",),
    "always_on_top": ("always-on-top",),
    "visible_on_all_workspaces": ("visible-on-all-workspaces",),
    "content_protected": ("content-protected",),
    "skip_taskbar": ("skip-taskbar",),
    "no_redirection_bitmap": ("no-redirection-bitmap",),
    "title_bar_style": ("title-bar-style",),
    "traffic_light_position": ("traffic-light-position",),
    "hidden_title": ("hidden-title",),
    "accept_first_mouse": ("accept-first-mouse",),
    "tabbing_identifier": ("tabbing-identifier",),
    "window_effects": ("window-effects",),
    "background_color": ("background-color",),
}


def _rename_aliases(raw: Any, cls: type[BaseModel]) -> Any:
    if not isinstance(raw, dict):
        return raw
    data = dict(raw)
    for attr in cls.model_fields:
        canonical = to_camel(attr)
        choices = SERDE_EXTRA_ALIASES.get(attr, ())
        present = [name for name in choices if name in data and name != attr]
        if present:
            if canonical in data or attr in data or len(present) > 1:
                raise ValueError(f"Duplicate spelling for {canonical}")
            data[canonical] = data.pop(present[0])
    return data


class IpcSecurity(StrEnum):
    NONE = "none"
    AUTHENTICATED = "authenticated"


class MenuItemType(StrEnum):
    ITEM = "item"
    SEPARATOR = "separator"
    SUBMENU = "submenu"


class IpcClientConfig(SerdeModel):
    endpoint: str = Field(
        default_factory=lambda: f"taurino-{uuid4().hex}",
        min_length=1,
    )

    connect_timeout: float = Field(default=10.0, gt=0)
    operation_timeout: float = Field(default=30.0, gt=0)
    handshake_timeout: float = Field(default=10.0, gt=0)

    max_message_size: int = Field(
        default=1024 * 1024,
        ge=1024,
        le=64 * 1024 * 1024,
    )

    max_clients: int = Field(default=1, ge=1, le=256)
    permissions: int | str | None = None

    security: IpcSecurity = IpcSecurity.NONE

    server_private_key_path: Path | None = None
    client_public_key: bytes | None = None
    client_private_key_path: Path | None = None

    @model_validator(mode="after")
    def validate_client(self) -> IpcClientConfig:
        if not self.endpoint.strip():
            raise ValueError("IPC endpoint must not be blank")

        for name in (
            "connect_timeout",
            "operation_timeout",
            "handshake_timeout",
        ):
            value = getattr(self, name)

            if not math.isfinite(value):
                raise ValueError(f"{name} must be finite")

        if self.security == IpcSecurity.NONE:
            if any(
                value is not None
                for value in (
                    self.server_private_key_path,
                    self.client_public_key,
                    self.client_private_key_path,
                )
            ):
                raise ValueError("Unauthenticated IPC must not contain identity keys")

        elif self.security == IpcSecurity.AUTHENTICATED:
            if self.server_private_key_path is None:
                raise ValueError("Missing serverPrivateKeyPath")

            if self.client_public_key is None or len(self.client_public_key) != 32:
                raise ValueError("clientPublicKey must contain 32 bytes")

            if self.client_private_key_path is None:
                raise ValueError("Missing clientPrivateKeyPath")

        return self

    def to_wire(self) -> dict[str, Any]:
        """Serialize configuration for MessagePack/Serde."""
        return self.model_dump(
            mode="python",
            by_alias=True,
            exclude_none=True,
        )


class ShortcutConfig(SerdeModel):
    accelerator: str
    action: str
    enabled: bool


class MenuItemConfig(SerdeModel):
    id: str
    text: str
    kind: MenuItemType
    enabled: bool
    accelerator: str | None = None
    children: list[MenuItemConfig] = Field(default_factory=list)


class MenuConfig(SerdeModel):
    items: list[MenuItemConfig] = Field(default_factory=list)


class TrayIconConfig(SerdeModel):
    id: str
    icon: str | None = None
    tooltip: str | None = None
    visible: bool
    menu: MenuConfig | None = None


class BaseDirectory(IntEnum):
    AUDIO = 1
    CACHE = 2
    CONFIG = 3
    DATA = 4
    LOCAL_DATA = 5
    DOCUMENT = 6
    DOWNLOAD = 7
    PICTURE = 8
    PUBLIC = 9
    VIDEO = 10
    RESOURCE = 11
    TEMP = 12
    APP_CONFIG = 13
    APP_DATA = 14
    APP_LOCAL_DATA = 15
    APP_CACHE = 16
    APP_LOG = 17
    DESKTOP = 18
    EXECUTABLE = 19
    FONT = 20
    HOME = 21
    RUNTIME = 22
    TEMPLATE = 23


class InvokeRequest(SerdeModel):
    cmd: str
    callback: U32
    error: U32
    payload: JsonValue = None
    options: JsonValue | None = None

    def to_wire(self):
        return self.model_dump(mode="json", by_alias=False)


class InvokeResponse(SerdeModel):
    ok: bool
    data: Any

    def to_wire(self):
        return self.model_dump(mode="json", by_alias=False)


class WebviewNotification(SerdeModel):
    webview_label: str
    event: str
    payload: Any

    def to_wire(self):
        return self.model_dump(mode="json", by_alias=False)


class AppConfig(SerdeModel):
    product_name: str | None = None
    version: str | None = None
    app_id: UUID
    connection: IpcClientConfig
    windows: list[WindowConfig]
    dev_url: str | None = None
    shortcuts: list[ShortcutConfig] = Field(default_factory=list)
    trays: list[TrayIconConfig] = Field(default_factory=list)
    app_menu: MenuConfig | None = None
    schema_version: U32 = 1

    @model_validator(mode="before")
    @classmethod
    def accept_serde_aliases(cls, raw: Any):
        return _rename_aliases(raw, cls)

    @field_validator("version")
    @classmethod
    def validate_version(cls, value: str | None) -> str | None:
        if value is None:
            return None

        path = Path(value)

        if path.is_file():
            data = json.loads(path.read_text(encoding="utf-8"))

            if not isinstance(data, dict):
                raise ValueError("Expected a JSON object")

            version = data.get("version")

            if not isinstance(version, str):
                raise ValueError("Missing or invalid version in package.json")

            value = version

        pattern = (
            r"(0|[1-9]\d*)\."
            r"(0|[1-9]\d*)\."
            r"(0|[1-9]\d*)"
            r"(?:-[0-9A-Za-z.-]+)?"
            r"(?:\+[0-9A-Za-z.-]+)?"
        )

        if re.fullmatch(pattern, value) is None:
            raise ValueError("Expected semantic version or package.json path")

        return value

    @field_validator("dev_url")
    @classmethod
    def validate_dev_url(cls, value: str | None):
        return _url(value) if value is not None else None

    @model_validator(mode="after")
    def validate_engine(self):
        if self.schema_version != 1:
            raise ValueError("Unsupported schemaVersion")
        window_labels = [w.label for w in self.windows]
        webview_labels = [v.label for w in self.windows for v in w.webviews]
        if len(window_labels) != len(set(window_labels)):
            raise ValueError("Duplicate window label")
        if len(webview_labels) != len(set(webview_labels)):
            raise ValueError("Duplicate WebView label")
        return self

    def to_wire(self) -> dict[str, Any]:
        data = super().to_wire()
        data["windows"] = [window.to_wire() for window in self.windows]
        data["connection"] = self.connection.to_wire()
        return data

    def to_engine_bytes(self) -> bytes:
        payload = self.to_msgpack()
        if len(payload) > 8 * 1024 * 1024:
            raise ValueError("Engine configuration exceeds 8 MiB")
        return payload

    @classmethod
    def from_engine_bytes(cls, payload: bytes) -> AppConfig:
        if not payload or len(payload) > 8 * 1024 * 1024:
            raise ValueError("Invalid engine config size")
        return cls.model_validate(msgpack.unpackb(payload, raw=False))


# Pydantic needs to resolve WindowConfig inside CreateWindowAction.
CreateWindowAction.model_rebuild()
NewWindowRule.model_rebuild()
RulesNewWindow.model_rebuild()
WebViewConfig.model_rebuild()
WindowConfig.model_rebuild()
AppConfig.model_rebuild()
