use {
    crate::window::{Window, inner_size},
    serde::Serialize,
    std::{path::PathBuf, sync::atomic::Ordering},
    tao::{
        dpi::{PhysicalPosition, PhysicalSize},
        event::{
            DeviceEvent, ElementState, Force, KeyEvent, MouseButton as TaoMouseButton, MouseScrollDelta, RawKeyEvent,
            Touch, TouchPhase, WindowEvent as TaoWindowEvent,
        },
        keyboard::{Key, KeyCode, KeyLocation, ModifiersState},
        window::Theme,
    },
    taurino_config::{Rect, utils::DeviceRegistry, window::WindowId},
    tray_icon::TrayIconId,
};
/// Describes the mouse button state.
#[derive(Default, Clone, Copy, PartialEq, Eq, Debug, Serialize)]
pub enum MouseButtonState {
    /// Mouse button released.
    #[default]
    Up,

    /// Mouse button pressed.
    Down,
}

impl From<tray_icon::MouseButtonState> for MouseButtonState {
    fn from(value: tray_icon::MouseButtonState) -> Self {
        match value {
            tray_icon::MouseButtonState::Up => Self::Up,
            tray_icon::MouseButtonState::Down => Self::Down,
        }
    }
}

/// Describes which mouse button triggered the event.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Default)]
pub enum MouseButton {
    #[default]
    Left,
    Right,
    Middle,
}

impl From<tray_icon::MouseButton> for MouseButton {
    fn from(value: tray_icon::MouseButton) -> Self {
        match value {
            tray_icon::MouseButton::Left => Self::Left,
            tray_icon::MouseButton::Right => Self::Right,
            tray_icon::MouseButton::Middle => Self::Middle,
        }
    }
}

/// Describes a tray icon event.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", content = "payload", rename_all = "camelCase")]
#[non_exhaustive]
pub enum TrayIconEvent {
    #[serde(rename_all = "camelCase")]
    Click {
        id: TrayIconId,
        position: PhysicalPosition<f64>,
        rect: Rect,
        button: MouseButton,
        button_state: MouseButtonState,
    },

    DoubleClick {
        id: TrayIconId,
        position: PhysicalPosition<f64>,
        rect: Rect,
        button: MouseButton,
    },

    Enter {
        id: TrayIconId,
        position: PhysicalPosition<f64>,
        rect: Rect,
    },

    Move {
        id: TrayIconId,
        position: PhysicalPosition<f64>,
        rect: Rect,
    },

    Leave {
        id: TrayIconId,
        position: PhysicalPosition<f64>,
        rect: Rect,
    },
}

impl TrayIconEvent {
    pub fn id(&self) -> &TrayIconId {
        match self {
            Self::Click { id, .. }
            | Self::DoubleClick { id, .. }
            | Self::Enter { id, .. }
            | Self::Move { id, .. }
            | Self::Leave { id, .. } => id,
        }
    }
}

impl From<tray_icon::TrayIconEvent> for TrayIconEvent {
    fn from(value: tray_icon::TrayIconEvent) -> Self {
        match value {
            tray_icon::TrayIconEvent::Click { id, position, rect, button, button_state } => Self::Click {
                id,
                position,
                rect: Rect { position: rect.position.into(), size: rect.size.into() },
                button: button.into(),
                button_state: button_state.into(),
            },

            tray_icon::TrayIconEvent::DoubleClick { id, position, rect, button } => Self::DoubleClick {
                id,
                position,
                rect: Rect { position: rect.position.into(), size: rect.size.into() },
                button: button.into(),
            },

            tray_icon::TrayIconEvent::Enter { id, position, rect } => {
                Self::Enter { id, position, rect: Rect { position: rect.position.into(), size: rect.size.into() } }
            }

            tray_icon::TrayIconEvent::Move { id, position, rect } => {
                Self::Move { id, position, rect: Rect { position: rect.position.into(), size: rect.size.into() } }
            }

            tray_icon::TrayIconEvent::Leave { id, position, rect } => {
                Self::Leave { id, position, rect: Rect { position: rect.position.into(), size: rect.size.into() } }
            }

            // tray_icon::TrayIconEvent is #[non_exhaustive].
            // With the currently supported tray-icon version these are all
            // known variants.
            _ => unreachable!("unsupported tray-icon event variant"),
        }
    }
}

/// An event from a window.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", content = "payload", rename_all = "camelCase")]
pub enum WebViewEvent {
    /// An event associated with the drag and drop action.
    DragDrop(DragDropEvent),
    FullscreenChanged(WindowId, bool),
}

/// The drag drop event payload.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", content = "payload", rename_all = "camelCase")]
pub enum DragDropEvent {
    /// A drag operation has entered the webview.
    Enter {
        /// List of paths that are being dragged onto the webview.
        paths: Vec<PathBuf>,
        /// The position of the mouse cursor.
        position: dpi::PhysicalPosition<f64>,
    },
    /// A drag operation is moving over the webview.
    Over {
        /// The position of the mouse cursor.
        position: dpi::PhysicalPosition<f64>,
    },
    /// The file(s) have been dropped onto the webview.
    Drop {
        /// List of paths that are being dropped onto the window.
        paths: Vec<PathBuf>,
        /// The position of the mouse cursor.
        position: dpi::PhysicalPosition<f64>,
    },
    /// The drag operation has been cancelled or left the window.
    Leave,
}

#[derive(Debug, Clone)]
pub enum SynthesizedEvent {
    Focused(bool),
    DragDrop(DragDropEvent),
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", content = "payload", rename_all = "camelCase")]
pub enum WindowEvent {
    Resized(SerdePhysicalSizeU32),
    Moved(SerdePhysicalPositionI32),
    CloseRequested,
    DragDrop(DragDropEvent),
    Destroyed,

    Started,

    Suspended,

    Resumed,

    Stopped,

    DroppedFile(String),

    HoveredFile(String),

    HoveredFileCancelled,

    ReceivedImeText(String),

    Focused(bool),

    KeyboardInput { device_id: u32, event: SerdeKeyEvent, is_synthetic: bool },

    ModifiersChanged(SerdeModifiersState),

    CursorMoved { device_id: u32, position: SerdePhysicalPositionF64, modifiers: SerdeModifiersState },

    CursorEntered(u32),

    CursorLeft(u32),

    MouseWheel { device_id: u32, delta: SerdeMouseScrollDelta, phase: SerdeTouchPhase, modifiers: SerdeModifiersState },

    MouseInput { device_id: u32, state: SerdeElementState, button: SerdeMouseButton, modifiers: SerdeModifiersState },

    TouchpadPressure { device_id: u32, pressure: f32, stage: i64 },

    AxisMotion { device_id: u32, axis: u32, value: f64 },

    Touch(SerdeTouch),

    ScaleFactorChanged { scale_factor: f64, new_inner_size: SerdePhysicalSizeU32 },

    ThemeChanged(SerdeTheme),

    DecorationsClick,

    Unsupported,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", content = "payload", rename_all = "camelCase")]
pub enum SerdeDeviceEvent {
    Added,

    Removed,

    MouseMotion { delta_x: f64, delta_y: f64 },

    MouseWheel { delta: SerdeMouseScrollDelta },

    Motion { axis: u32, value: f64 },

    Button { button: u32, state: SerdeElementState },

    Key { event: SerdeRawKeyEvent },

    Text { codepoint: char },

    Unsupported,
}

#[derive(Debug, Clone, Serialize)]
pub struct SerdeKeyEvent {
    pub physical_key: KeyCode,
    pub logical_key: Key<'static>,
    pub text: Option<String>,
    pub location: KeyLocation,
    pub state: SerdeElementState,
    pub repeat: bool,
    pub text_with_all_modifiers: Option<String>,
    pub key_without_modifiers: Key<'static>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SerdeRawKeyEvent {
    pub physical_key: KeyCode,
    pub state: SerdeElementState,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct SerdeModifiersState {
    pub shift_key: bool,
    pub control_key: bool,
    pub alt_key: bool,
    pub super_key: bool,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct SerdePhysicalSizeU32 {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct SerdePhysicalPositionI32 {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct SerdePhysicalPositionF64 {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(tag = "type", content = "payload", rename_all = "camelCase")]
pub enum SerdeMouseScrollDelta {
    LineDelta { x: f32, y: f32 },

    PixelDelta { position: SerdePhysicalPositionF64 },
}

#[derive(Debug, Clone, Copy, Serialize)]
pub enum SerdeTouchPhase {
    Started,
    Moved,
    Ended,
    Cancelled,
    Unsupported,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub enum SerdeElementState {
    Pressed,
    Released,
    Unsupported,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(tag = "type", content = "payload", rename_all = "camelCase")]
pub enum SerdeMouseButton {
    Left,
    Right,
    Middle,
    Other { button: u16 },
    Unsupported,
}

#[derive(Debug, Clone, Serialize)]
pub struct SerdeTouch {
    pub device_id: u32,
    pub phase: SerdeTouchPhase,
    pub location: SerdePhysicalPositionF64,
    pub force: Option<SerdeForce>,
    pub id: u64,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(tag = "type", content = "payload", rename_all = "camelCase")]
pub enum SerdeForce {
    Calibrated { force: f64, max_possible_force: f64, altitude_angle: Option<f64> },

    Normalized { force: f64 },

    Unsupported,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub enum SerdeTheme {
    Light,
    Dark,
    Unsupported,
}

impl From<PhysicalSize<u32>> for SerdePhysicalSizeU32 {
    fn from(value: PhysicalSize<u32>) -> Self { Self { width: value.width, height: value.height } }
}

impl From<&PhysicalSize<u32>> for SerdePhysicalSizeU32 {
    fn from(value: &PhysicalSize<u32>) -> Self { Self { width: value.width, height: value.height } }
}

impl From<PhysicalPosition<i32>> for SerdePhysicalPositionI32 {
    fn from(value: PhysicalPosition<i32>) -> Self { Self { x: value.x, y: value.y } }
}

impl From<&PhysicalPosition<i32>> for SerdePhysicalPositionI32 {
    fn from(value: &PhysicalPosition<i32>) -> Self { Self { x: value.x, y: value.y } }
}

impl From<PhysicalPosition<f64>> for SerdePhysicalPositionF64 {
    fn from(value: PhysicalPosition<f64>) -> Self { Self { x: value.x, y: value.y } }
}

impl From<&PhysicalPosition<f64>> for SerdePhysicalPositionF64 {
    fn from(value: &PhysicalPosition<f64>) -> Self { Self { x: value.x, y: value.y } }
}

impl From<ModifiersState> for SerdeModifiersState {
    fn from(value: ModifiersState) -> Self {
        Self {
            shift_key: value.shift_key(),
            control_key: value.control_key(),
            alt_key: value.alt_key(),
            super_key: value.super_key(),
        }
    }
}

impl From<&ModifiersState> for SerdeModifiersState {
    fn from(value: &ModifiersState) -> Self {
        Self {
            shift_key: value.shift_key(),
            control_key: value.control_key(),
            alt_key: value.alt_key(),
            super_key: value.super_key(),
        }
    }
}

impl From<ElementState> for SerdeElementState {
    fn from(value: ElementState) -> Self {
        match value {
            ElementState::Pressed => Self::Pressed,
            ElementState::Released => Self::Released,
            _ => Self::Unsupported,
        }
    }
}

impl From<&ElementState> for SerdeElementState {
    fn from(value: &ElementState) -> Self { (*value).into() }
}

impl From<TouchPhase> for SerdeTouchPhase {
    fn from(value: TouchPhase) -> Self {
        match value {
            TouchPhase::Started => Self::Started,
            TouchPhase::Moved => Self::Moved,
            TouchPhase::Ended => Self::Ended,
            TouchPhase::Cancelled => Self::Cancelled,
            _ => Self::Unsupported,
        }
    }
}

impl From<&TouchPhase> for SerdeTouchPhase {
    fn from(value: &TouchPhase) -> Self { (*value).into() }
}

impl From<TaoMouseButton> for SerdeMouseButton {
    fn from(value: TaoMouseButton) -> Self {
        match value {
            TaoMouseButton::Left => Self::Left,
            TaoMouseButton::Right => Self::Right,
            TaoMouseButton::Middle => Self::Middle,
            TaoMouseButton::Other(button) => Self::Other { button },
            _ => Self::Unsupported,
        }
    }
}

impl From<&TaoMouseButton> for SerdeMouseButton {
    fn from(value: &TaoMouseButton) -> Self { (*value).into() }
}

impl From<MouseScrollDelta> for SerdeMouseScrollDelta {
    fn from(value: MouseScrollDelta) -> Self {
        match value {
            MouseScrollDelta::LineDelta(x, y) => Self::LineDelta { x, y },

            MouseScrollDelta::PixelDelta(position) => Self::PixelDelta { position: position.into() },

            _ => unreachable!(),
        }
    }
}

impl From<&MouseScrollDelta> for SerdeMouseScrollDelta {
    fn from(value: &MouseScrollDelta) -> Self { (*value).into() }
}

impl From<Theme> for SerdeTheme {
    fn from(value: Theme) -> Self {
        match value {
            Theme::Light => Self::Light,
            Theme::Dark => Self::Dark,
            _ => Self::Unsupported,
        }
    }
}

impl From<&Theme> for SerdeTheme {
    fn from(value: &Theme) -> Self { (*value).into() }
}

impl From<Force> for SerdeForce {
    fn from(value: Force) -> Self {
        match value {
            Force::Calibrated { force, max_possible_force, altitude_angle, .. } => {
                Self::Calibrated { force, max_possible_force, altitude_angle }
            }

            Force::Normalized(force) => Self::Normalized { force },

            _ => Self::Unsupported,
        }
    }
}

impl From<&Force> for SerdeForce {
    fn from(value: &Force) -> Self { (*value).into() }
}

impl From<&RawKeyEvent> for SerdeRawKeyEvent {
    fn from(value: &RawKeyEvent) -> Self { Self { physical_key: value.physical_key, state: value.state.into() } }
}

impl From<&KeyEvent> for SerdeKeyEvent {
    fn from(value: &KeyEvent) -> Self {
        Self {
            physical_key: value.physical_key,
            logical_key: value.logical_key.clone(),
            text: value.text.map(ToOwned::to_owned),
            location: value.location,
            state: value.state.into(),
            repeat: value.repeat,
            text_with_all_modifiers: value.text_with_all_modifiers().map(ToOwned::to_owned),
            key_without_modifiers: value.key_without_modifiers(),
        }
    }
}

pub fn serialize_touch(touch: &Touch, device_id_registry: DeviceRegistry) -> SerdeTouch {
    SerdeTouch {
        device_id: device_id_registry.get_or_create_id(touch.device_id),
        phase: touch.phase.into(),
        location: touch.location.into(),
        force: touch.force.map(Into::into),
        id: touch.id,
    }
}

pub fn serialize_device_event(event: &DeviceEvent) -> SerdeDeviceEvent {
    match event {
        DeviceEvent::Added => SerdeDeviceEvent::Added,

        DeviceEvent::Removed => SerdeDeviceEvent::Removed,

        DeviceEvent::MouseMotion { delta, .. } => SerdeDeviceEvent::MouseMotion { delta_x: delta.0, delta_y: delta.1 },

        DeviceEvent::MouseWheel { delta, .. } => SerdeDeviceEvent::MouseWheel { delta: delta.into() },

        DeviceEvent::Motion { axis, value, .. } => SerdeDeviceEvent::Motion { axis: *axis, value: *value },

        DeviceEvent::Button { button, state, .. } => SerdeDeviceEvent::Button { button: *button, state: state.into() },

        DeviceEvent::Key(event) => SerdeDeviceEvent::Key { event: event.into() },

        DeviceEvent::Text { codepoint, .. } => SerdeDeviceEvent::Text { codepoint: *codepoint },

        _ => SerdeDeviceEvent::Unsupported,
    }
}

pub struct WindowEventWrapper(pub Option<WindowEvent>);

impl WindowEventWrapper {
    pub fn map_from_tao(
        event: &TaoWindowEvent<'_>,
        #[cfg(windows)] window: &Window,
        device_id_registry: DeviceRegistry,
    ) -> Self {
        let event = match event {
            TaoWindowEvent::Resized(size) => WindowEvent::Resized(size.into()),

            TaoWindowEvent::Moved(position) => WindowEvent::Moved(position.into()),

            TaoWindowEvent::CloseRequested => WindowEvent::CloseRequested,

            TaoWindowEvent::Destroyed => WindowEvent::Destroyed,

            TaoWindowEvent::Started => WindowEvent::Started,

            TaoWindowEvent::Suspended => WindowEvent::Suspended,

            TaoWindowEvent::Resumed => WindowEvent::Resumed,

            TaoWindowEvent::Stopped => WindowEvent::Stopped,

            TaoWindowEvent::DroppedFile(path) => WindowEvent::DroppedFile(path.to_string_lossy().into_owned()),

            TaoWindowEvent::HoveredFile(path) => WindowEvent::HoveredFile(path.to_string_lossy().into_owned()),

            TaoWindowEvent::HoveredFileCancelled => WindowEvent::HoveredFileCancelled,

            TaoWindowEvent::ReceivedImeText(text) => WindowEvent::ReceivedImeText(text.clone()),
            TaoWindowEvent::Focused(focused) => {
                #[cfg(not(windows))]
                return Self(Some(WindowEvent::Focused(*focused)));
                // on multiwebview mode, if there's no focused webview, it means we're receiving a direct window focus change
                // (without receiving a webview focus, such as when clicking the taskbar app icon or using Alt + Tab)
                // in this case we must send the focus change event here
                #[cfg(windows)]
                if window.has_children.load(Ordering::Relaxed) {
                    use taurino_config::FocusState;

                    if !*focused {
                        // Blur events are handled in the webview side (add_LostFocus)
                        return Self(None);
                    }

                    let mut focused_webview = window.focused_webview.lock().unwrap();
                    if let FocusState::Blured { last_focused_webview_label } = &*focused_webview {
                        let should_focus_webview =
                            last_focused_webview_label.as_deref().and_then(|last_focused_webview_label| {
                                window.webviews().iter().find(|w| w.label == last_focused_webview_label)
                            });
                        *focused_webview = FocusState::WindowFocused;
                        if let Some(should_focus_webview) = should_focus_webview {
                            drop(focused_webview);
                            let _ = should_focus_webview.focus();
                        }
                        WindowEvent::Focused(true)
                    } else {
                        // Already focused
                        return Self(None);
                    }
                } else if window.webviews().is_empty() {
                    // Raw tao window without webviews, forward the event
                    WindowEvent::Focused(*focused)
                } else {
                    // when not on multiwebview mode, wry will set focus to the webview,
                    // and we will handle focus change events on the webview (add_GotFocus and add_LostFocus)
                    return Self(None);
                }
            }
            TaoWindowEvent::KeyboardInput { device_id: tao_device_id, event, is_synthetic, .. } => {
                WindowEvent::KeyboardInput {
                    device_id: device_id_registry.get_or_create_id(*tao_device_id),
                    event: event.into(),
                    is_synthetic: *is_synthetic,
                }
            }

            TaoWindowEvent::ModifiersChanged(modifiers) => WindowEvent::ModifiersChanged(modifiers.into()),

            TaoWindowEvent::CursorMoved {
                device_id: tao_device_id,
                position,
                #[allow(deprecated)]
                modifiers,
            } => WindowEvent::CursorMoved {
                device_id: device_id_registry.get_or_create_id(*tao_device_id),
                position: position.into(),
                modifiers: modifiers.into(),
            },

            TaoWindowEvent::CursorEntered { device_id: tao_device_id } => {
                WindowEvent::CursorEntered(device_id_registry.get_or_create_id(*tao_device_id))
            }

            TaoWindowEvent::CursorLeft { device_id: tao_device_id } => {
                WindowEvent::CursorLeft(device_id_registry.get_or_create_id(*tao_device_id))
            }

            TaoWindowEvent::MouseWheel {
                device_id: tao_device_id,
                delta,
                phase,
                #[allow(deprecated)]
                modifiers,
            } => WindowEvent::MouseWheel {
                device_id: device_id_registry.get_or_create_id(*tao_device_id),
                delta: delta.into(),
                phase: phase.into(),
                modifiers: modifiers.into(),
            },

            TaoWindowEvent::MouseInput {
                device_id: tao_device_id,
                state,
                button,
                #[allow(deprecated)]
                modifiers,
            } => WindowEvent::MouseInput {
                device_id: device_id_registry.get_or_create_id(*tao_device_id),
                state: state.into(),
                button: button.into(),
                modifiers: modifiers.into(),
            },

            TaoWindowEvent::TouchpadPressure { device_id: tao_device_id, pressure, stage } => {
                WindowEvent::TouchpadPressure {
                    device_id: device_id_registry.get_or_create_id(*tao_device_id),
                    pressure: *pressure,
                    stage: *stage,
                }
            }

            TaoWindowEvent::AxisMotion { device_id: tao_device_id, axis, value } => WindowEvent::AxisMotion {
                device_id: device_id_registry.get_or_create_id(*tao_device_id),
                axis: *axis,
                value: *value,
            },

            TaoWindowEvent::Touch(touch) => WindowEvent::Touch(serialize_touch(touch, device_id_registry)),

            TaoWindowEvent::ScaleFactorChanged { scale_factor, new_inner_size } => WindowEvent::ScaleFactorChanged {
                scale_factor: *scale_factor,
                new_inner_size: SerdePhysicalSizeU32 { width: new_inner_size.width, height: new_inner_size.height },
            },

            TaoWindowEvent::ThemeChanged(theme) => WindowEvent::ThemeChanged(theme.into()),

            TaoWindowEvent::DecorationsClick => WindowEvent::DecorationsClick,

            _ => WindowEvent::Unsupported,
        };
        Self(Some(event))
    }

    pub fn parse(
        window: &Window,
        event: &TaoWindowEvent<'_>,
        device_id_registry: DeviceRegistry,
    ) -> anyhow::Result<Self> {
        match event {
            // resized event from tao doesn't include a reliable size on macOS
            // because wry replaces the NSView
            TaoWindowEvent::Resized(_) => {
                if let Some(w) = &window.inner {
                    let size = inner_size(w, &window.webviews(), window.has_children.load(Ordering::Relaxed))?;
                    Ok(Self(Some(WindowEvent::Resized(size.into()))))
                } else {
                    Ok(Self(None))
                }
            }
            e => Ok(Self::map_from_tao(
                e,
                #[cfg(windows)]
                window,
                device_id_registry,
            )),
        }
    }
}

impl From<SynthesizedEvent> for WindowEventWrapper {
    fn from(event: SynthesizedEvent) -> Self {
        let event = match event {
            SynthesizedEvent::Focused(focused) => WindowEvent::Focused(focused),
            SynthesizedEvent::DragDrop(event) => WindowEvent::DragDrop(event),
        };
        Self(Some(event))
    }
}
