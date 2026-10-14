"""from uuid import uuid4
import msgpack
import pytest
from pydantic import ValidationError
from config import (
    AppConfig, WebViewConfig, WindowConfig, AllowListNavigation,
    NavigationRule, RulesNewWindow, NewWindowRule, DenyWindowAction,
    NewWindowRequestMatcher, WindowEffectsConfig, WindowEffect,
    IpcClientConfig, IpcSecurity, ShortcutConfig, MenuConfig, MenuItemConfig,
    MenuItemType, TrayIconConfig, BaseDirectory
)


def connection(**overrides):
    kwargs = dict(endpoint='127.0.0.1:9000', connect_timeout=5,
                  operation_timeout=10, handshake_timeout=5,
                  max_message_size=4096, security='none')
    kwargs.update(overrides)
    return IpcClientConfig(**kwargs)


def test_roundtrip():
    cfg = AppConfig(
        product_name='Example', version='1.2.3', app_id=uuid4(),
        connection=connection(),
        shortcuts=[ShortcutConfig(accelerator='Ctrl+Q', action='quit', enabled=True)],
        app_menu=MenuConfig(items=[MenuItemConfig(id='file', text='File', kind=MenuItemType.SUBMENU, enabled=True)]),
        trays=[TrayIconConfig(id='tray', visible=True)],
        windows=[WindowConfig(
            label='main', background_color='#abc',
            window_effects=WindowEffectsConfig(effects=[WindowEffect.MICA]),
            webviews=[WebViewConfig(
                label='root', navigation_policy=AllowListNavigation(
                    rules=[NavigationRule(host='example.com', include_subdomains=True)]
                ),
                new_window_policy=RulesNewWindow(rules=[NewWindowRule(
                    request=NewWindowRequestMatcher(host='example.com'),
                    response=DenyWindowAction(),
                )]),
            )],
        )],
    )
    wire = cfg.to_wire()
    assert 'productName' in wire and 'productname' not in wire
    assert wire['windows'][0]['backgroundColor'] == [170, 187, 204, 255]
    assert wire['windows'][0]['webviews'][0]['navigationPolicy']['type'] == 'allowList'
    assert AppConfig.from_engine_bytes(cfg.to_engine_bytes()).to_wire() == wire


def test_rust_aliases():
    v = WebViewConfig.model_validate({'label': 'root', 'user-agent': 'Taurino', 'drag-drop-enabled': True})
    assert v.to_wire()['userAgent'] == 'Taurino'
    w = WindowConfig.model_validate({'webviews': [], 'min-width': 400})
    assert w.to_wire()['minWidth'] == 400


def test_ipc_authenticated_bin():
    cfg = AppConfig(app_id=uuid4(), windows=[], connection=connection(
        security=IpcSecurity.AUTHENTICATED,
        server_public_key=b'k' * 32,
        client_private_key_path='key.secret',
    ))
    decoded = msgpack.unpackb(cfg.to_engine_bytes(), raw=False)
    assert decoded['connection']['serverPublicKey'] == b'k' * 32
    assert decoded['connection']['clientPrivateKeyPath'] == 'key.secret'
    assert AppConfig.from_engine_bytes(cfg.to_engine_bytes()).connection.server_public_key == b'k' * 32


def test_invalid_ipc():
    with pytest.raises(ValidationError):
        connection(max_message_size=1)
    with pytest.raises(ValidationError):
        connection(security='authenticated')


def test_numeric_base_dir():
    assert int(BaseDirectory.APP_LOG) == 17


def test_default_mismatch_resolved():
    assert WebViewConfig().auto_resize is False
    assert WebViewConfig().enable_clipboard_access is True
    assert WindowConfig().enable_drag_drop is True
"""
