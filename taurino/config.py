"""Fully typed fluent builders for the existing Taurino Pydantic v2 config models.

Each configuration field has an explicit builder method. Nested builders can be
passed directly to app/window collections; `build()` validates with Pydantic.
"""

from __future__ import annotations

from copy import deepcopy
from typing import Any, Generic, Self, TypeAlias, TypeVar
from uuid import UUID

from pydantic import BaseModel

from .proto import (
    AppConfig,
    BackgroundThrottlingPolicy,
    Color,
    IpcClientConfig,
    MenuConfig,
    NavigationPolicy,
    NewWindowPolicy,
    PermissionRequestPolicy,
    PreventOverflowConfig,
    ScrollBarStyle,
    ShortcutConfig,
    Theme,
    TitleBarStyle,
    TrayIconConfig,
    WebContentProcessTerminatePolicy,
    WebViewConfig,
    WindowConfig,
    WindowEffectsConfig,
)

T = TypeVar("T", bound=BaseModel)

# Pydantic validates the exact 16 U8 channels at build time.
DataStoreIdentifier: TypeAlias = tuple[int, ...]
# WindowConfig normalizes several color inputs with parse_color.
ColorInput: TypeAlias = Color | str | list[int] | dict[str, int]


class ConfigBuilder(Generic[T]):
    """Mutable fluent builder. `build()` returns an independent validated model."""

    model_type: type[T]

    def __init__(self, **values: Any) -> None:
        self._values: dict[str, Any] = deepcopy(values)

    def _set(self, name: str, value: Any) -> Self:
        if name not in self.model_type.model_fields:
            raise ValueError(f"Unknown configuration field: {name!r}")
        self._values[name] = deepcopy(value)
        return self

    def set(self, **values: Any) -> Self:
        """Escape hatch for model fields; named methods offer better type checking."""
        unknown = set(values).difference(self.model_type.model_fields)
        if unknown:
            raise ValueError(f"Unknown configuration field(s): {sorted(unknown)}")
        self._values.update(deepcopy(values))
        return self

    def build(self) -> T:
        """Build a fresh model and run every Pydantic field/model validator."""
        return self.model_type.model_validate(deepcopy(self._values))


class WebViewBuilder(ConfigBuilder[WebViewConfig]):
    """Fluent builder for WebViewConfig."""

    model_type = WebViewConfig

    def __init__(self, label: str = "root") -> None:
        super().__init__(label=label)

    def label(self, value: str) -> Self:
        return self._set("label", value)

    def child(self, value: bool = True) -> Self:
        return self._set("child", value)

    def auto_resize(self, value: bool = True) -> Self:
        return self._set("auto_resize", value)

    def url(self, value: str) -> Self:
        return self._set("url", value)

    def user_agent(self, value: str) -> Self:
        return self._set("user_agent", value)

    def drag_drop_enabled(self, value: bool = True) -> Self:
        return self._set("drag_drop_enabled", value)

    def additional_browser_args(self, value: str) -> Self:
        return self._set("additional_browser_args", value)

    def incognito(self, value: bool = True) -> Self:
        return self._set("incognito", value)

    def proxy_url(self, value: str) -> Self:
        return self._set("proxy_url", value)

    def zoom_hotkeys_enabled(self, value: bool = True) -> Self:
        return self._set("zoom_hotkeys_enabled", value)

    def browser_extensions_enabled(self, value: bool = True) -> Self:
        return self._set("browser_extensions_enabled", value)

    def use_https_scheme(self, value: bool = True) -> Self:
        return self._set("use_https_scheme", value)

    def devtools(self, value: bool) -> Self:
        return self._set("devtools", value)

    def background_throttling(self, value: BackgroundThrottlingPolicy) -> Self:
        return self._set("background_throttling", value)

    def javascript_disabled(self, value: bool = True) -> Self:
        return self._set("javascript_disabled", value)

    def allow_link_preview(self, value: bool = True) -> Self:
        return self._set("allow_link_preview", value)

    def disable_input_accessory_view(self, value: bool = True) -> Self:
        return self._set("disable_input_accessory_view", value)

    def data_directory(self, value: str) -> Self:
        return self._set("data_directory", value)

    def data_store_identifier(self, value: DataStoreIdentifier) -> Self:
        return self._set("data_store_identifier", value)

    def scroll_bar_style(self, value: ScrollBarStyle) -> Self:
        return self._set("scroll_bar_style", value)

    def limit_navigations_to_app_bound_domains(self, value: bool = True) -> Self:
        return self._set("limit_navigations_to_app_bound_domains", value)

    def activity_name(self, value: str) -> Self:
        return self._set("activity_name", value)

    def created_by_activity_name(self, value: str) -> Self:
        return self._set("created_by_activity_name", value)

    def requested_by_scene_identifier(self, value: str) -> Self:
        return self._set("requested_by_scene_identifier", value)

    def general_autofill_enabled(self, value: bool = True) -> Self:
        return self._set("general_autofill_enabled", value)

    def enable_clipboard_access(self, value: bool = True) -> Self:
        return self._set("enable_clipboard_access", value)

    def navigation_policy(self, value: NavigationPolicy) -> Self:
        return self._set("navigation_policy", value)

    def new_window_policy(self, value: NewWindowPolicy) -> Self:
        return self._set("new_window_policy", value)

    def permission_request_policy(self, value: PermissionRequestPolicy) -> Self:
        return self._set("permission_request_policy", value)

    def web_content_process_terminate_policy(self, value: WebContentProcessTerminatePolicy) -> Self:
        return self._set("web_content_process_terminate_policy", value)

    def bounds(self, value: dict[str, Any]) -> Self:
        return self._set("bounds", value)


class WindowBuilder(ConfigBuilder[WindowConfig]):
    """Fluent builder for WindowConfig."""

    model_type = WindowConfig

    def __init__(self, label: str = "main") -> None:
        super().__init__(label=label, webviews=[])

    def label(self, value: str) -> Self:
        return self._set("label", value)

    def create(self, value: bool = True) -> Self:
        return self._set("create", value)

    def x(self, value: float) -> Self:
        return self._set("x", value)

    def y(self, value: float) -> Self:
        return self._set("y", value)

    def webview(self, value: WebViewConfig | WebViewBuilder) -> Self:
        config = value.build() if isinstance(value, WebViewBuilder) else value
        self._values["webviews"].append(config.model_copy(deep=True))
        return self

    def webviews(self, *values: WebViewConfig | WebViewBuilder) -> Self:
        """Append one or more WebViews."""
        for value in values:
            self.webview(value)
        return self

    def center(self, value: bool = True) -> Self:
        return self._set("center", value)

    def width(self, value: float) -> Self:
        return self._set("width", value)

    def height(self, value: float) -> Self:
        return self._set("height", value)

    def size(self, width: float, height: float) -> Self:
        return self.set(width=width, height=height)

    def position(self, x: float, y: float) -> Self:
        return self.set(x=x, y=y)

    def min_size(self, width: float | None, height: float | None) -> Self:
        return self.set(min_width=width, min_height=height)

    def max_size(self, width: float | None, height: float | None) -> Self:
        return self.set(max_width=width, max_height=height)

    def min_width(self, value: float) -> Self:
        return self._set("min_width", value)

    def min_height(self, value: float) -> Self:
        return self._set("min_height", value)

    def max_width(self, value: float) -> Self:
        return self._set("max_width", value)

    def max_height(self, value: float) -> Self:
        return self._set("max_height", value)

    def prevent_overflow(self, value: PreventOverflowConfig) -> Self:
        return self._set("prevent_overflow", value)

    def resizable(self, value: bool = True) -> Self:
        return self._set("resizable", value)

    def maximizable(self, value: bool = True) -> Self:
        return self._set("maximizable", value)

    def minimizable(self, value: bool = True) -> Self:
        return self._set("minimizable", value)

    def closable(self, value: bool = True) -> Self:
        return self._set("closable", value)

    def title(self, value: str) -> Self:
        return self._set("title", value)

    def fullscreen(self, value: bool = True) -> Self:
        return self._set("fullscreen", value)

    def focus(self, value: bool = True) -> Self:
        return self._set("focus", value)

    def focusable(self, value: bool = True) -> Self:
        return self._set("focusable", value)

    def transparent(self, value: bool = True) -> Self:
        return self._set("transparent", value)

    def maximized(self, value: bool = True) -> Self:
        return self._set("maximized", value)

    def visible(self, value: bool = True) -> Self:
        return self._set("visible", value)

    def decorations(self, value: bool = True) -> Self:
        return self._set("decorations", value)

    def always_on_bottom(self, value: bool = True) -> Self:
        return self._set("always_on_bottom", value)

    def always_on_top(self, value: bool = True) -> Self:
        return self._set("always_on_top", value)

    def visible_on_all_workspaces(self, value: bool = True) -> Self:
        return self._set("visible_on_all_workspaces", value)

    def content_protected(self, value: bool = True) -> Self:
        return self._set("content_protected", value)

    def skip_taskbar(self, value: bool = True) -> Self:
        return self._set("skip_taskbar", value)

    def window_classname(self, value: str) -> Self:
        return self._set("window_classname", value)

    def no_redirection_bitmap(self, value: bool = True) -> Self:
        return self._set("no_redirection_bitmap", value)

    def theme(self, value: Theme) -> Self:
        return self._set("theme", value)

    def title_bar_style(self, value: TitleBarStyle) -> Self:
        return self._set("title_bar_style", value)

    def traffic_light_position(self, value: dict[str, Any]) -> Self:
        return self._set("traffic_light_position", value)

    def hidden_title(self, value: bool = True) -> Self:
        return self._set("hidden_title", value)

    def accept_first_mouse(self, value: bool = True) -> Self:
        return self._set("accept_first_mouse", value)

    def tabbing_identifier(self, value: str) -> Self:
        return self._set("tabbing_identifier", value)

    def shadow(self, value: bool = True) -> Self:
        return self._set("shadow", value)

    def window_effects(self, value: WindowEffectsConfig) -> Self:
        return self._set("window_effects", value)

    def parent(self, value: str) -> Self:
        return self._set("parent", value)

    def background_color(self, value: ColorInput) -> Self:
        return self._set("background_color", value)

    def enable_drag_drop(self, value: bool = True) -> Self:
        return self._set("enable_drag_drop", value)


class AppBuilder(ConfigBuilder[AppConfig]):
    """Fluent builder for AppConfig."""

    model_type = AppConfig

    def __init__(self) -> None:
        super().__init__(windows=[], shortcuts=[], trays=[])

    def product_name(self, value: str) -> Self:
        return self._set("product_name", value)

    def version(self, value: str) -> Self:
        return self._set("version", value)

    def app_id(self, value: UUID) -> Self:
        return self._set("app_id", value)

    def connection(self, value: IpcClientConfig) -> Self:
        return self._set("connection", value)

    def window(self, value: WindowConfig | WindowBuilder) -> Self:
        config = value.build() if isinstance(value, WindowBuilder) else value
        self._values["windows"].append(config.model_copy(deep=True))
        return self

    def windows(self, *values: WindowConfig | WindowBuilder) -> Self:
        """Append one or more windows."""
        for value in values:
            self.window(value)
        return self

    def dev_url(self, value: str) -> Self:
        return self._set("dev_url", value)

    def shortcut(self, value: ShortcutConfig) -> Self:
        self._values["shortcuts"].append(value.model_copy(deep=True))
        return self

    def shortcuts(self, *values: ShortcutConfig) -> Self:
        """Append one or more shortcuts."""
        for value in values:
            self.shortcut(value)
        return self

    def tray(self, value: TrayIconConfig) -> Self:
        self._values["trays"].append(value.model_copy(deep=True))
        return self

    def trays(self, *values: TrayIconConfig) -> Self:
        """Append one or more trays."""
        for value in values:
            self.tray(value)
        return self

    def app_menu(self, value: MenuConfig) -> Self:
        return self._set("app_menu", value)

    def schema_version(self, value: int) -> Self:
        return self._set("schema_version", value)


__all__ = ["ConfigBuilder", "WebViewBuilder", "WindowBuilder", "AppBuilder"]
