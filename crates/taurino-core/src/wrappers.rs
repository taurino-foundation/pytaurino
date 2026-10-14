/* anyhow::{self, Context},
dpi::Rect,
image::Icon,
tao::{
    event_loop::DeviceEventFilter as TaoDeviceEventFilter,
    monitor::MonitorHandle,
    window::{
        CursorIcon as TaoCursorIcon, Icon as TaoWindowIcon,
        ProgressBarState as TaoProgressBarState, ProgressState as TaoProgressState,
        UserAttentionType as TaoUserAttentionType,
    },
}, */

/* use crate::{
    platforms::MonitorExt,
    schema::{
        Rect,
        window::{
            CursorIcon, DeviceEventFilter, Icon, Monitor, ProgressBarState, ProgressBarStatus,
            UserAttentionType,
        },
    },
}; */
use {
    crate::platforms::MonitorExt,
    tao::{
        event_loop::DeviceEventFilter as TaoDeviceEventFilter,
        monitor::MonitorHandle,
        window::{
            CursorIcon as TaoCursorIcon, Icon as TaoWindowIcon, ProgressBarState as TaoProgressBarState,
            ProgressState as TaoProgressState, UserAttentionType as TaoUserAttentionType,
        },
    },
    taurino_config::{
        Rect,
        window::{
            CursorIcon, DeviceEventFilter, Icon, Monitor, ProgressBarState, ProgressBarStatus, UserAttentionType,
        },
    },
};

/// Wrapper around a [`tao::window::Icon`] that can be created from an [`Icon`].
pub struct DeviceEventFilterWrapper(pub TaoDeviceEventFilter);

impl From<DeviceEventFilter> for DeviceEventFilterWrapper {
    fn from(item: DeviceEventFilter) -> Self {
        match item {
            DeviceEventFilter::Always => Self(TaoDeviceEventFilter::Always),
            DeviceEventFilter::Never => Self(TaoDeviceEventFilter::Never),
            DeviceEventFilter::Unfocused => Self(TaoDeviceEventFilter::Unfocused),
        }
    }
}

pub struct MonitorHandleWrapper(pub MonitorHandle);

impl From<MonitorHandleWrapper> for Monitor {
    fn from(monitor: MonitorHandleWrapper) -> Monitor {
        Self {
            name: monitor.0.name(),
            position: monitor.0.position(),
            size: monitor.0.size(),
            work_area: monitor.0.work_area(),
            scale_factor: monitor.0.scale_factor(),
        }
    }
}

pub struct RectWrapper(pub wry::Rect);

impl From<Rect> for RectWrapper {
    fn from(value: Rect) -> Self { RectWrapper(wry::Rect { position: value.position, size: value.size }) }
}

#[derive(Debug, Clone)]
pub struct UserAttentionTypeWrapper(pub TaoUserAttentionType);

impl From<UserAttentionType> for UserAttentionTypeWrapper {
    fn from(request_type: UserAttentionType) -> Self {
        let o = match request_type {
            UserAttentionType::Critical => TaoUserAttentionType::Critical,
            UserAttentionType::Informational => TaoUserAttentionType::Informational,
        };
        Self(o)
    }
}

#[derive(Debug)]
pub struct CursorIconWrapper(pub TaoCursorIcon);

impl From<CursorIcon> for CursorIconWrapper {
    fn from(icon: CursorIcon) -> Self {
        use CursorIcon::*;

        let i = match icon {
            Default => TaoCursorIcon::Default,
            Crosshair => TaoCursorIcon::Crosshair,
            Hand => TaoCursorIcon::Hand,
            Arrow => TaoCursorIcon::Arrow,
            Move => TaoCursorIcon::Move,
            Text => TaoCursorIcon::Text,
            Wait => TaoCursorIcon::Wait,
            Help => TaoCursorIcon::Help,
            Progress => TaoCursorIcon::Progress,
            NotAllowed => TaoCursorIcon::NotAllowed,
            ContextMenu => TaoCursorIcon::ContextMenu,
            Cell => TaoCursorIcon::Cell,
            VerticalText => TaoCursorIcon::VerticalText,
            Alias => TaoCursorIcon::Alias,
            Copy => TaoCursorIcon::Copy,
            NoDrop => TaoCursorIcon::NoDrop,
            Grab => TaoCursorIcon::Grab,
            Grabbing => TaoCursorIcon::Grabbing,
            AllScroll => TaoCursorIcon::AllScroll,
            ZoomIn => TaoCursorIcon::ZoomIn,
            ZoomOut => TaoCursorIcon::ZoomOut,
            EResize => TaoCursorIcon::EResize,
            NResize => TaoCursorIcon::NResize,
            NeResize => TaoCursorIcon::NeResize,
            NwResize => TaoCursorIcon::NwResize,
            SResize => TaoCursorIcon::SResize,
            SeResize => TaoCursorIcon::SeResize,
            SwResize => TaoCursorIcon::SwResize,
            WResize => TaoCursorIcon::WResize,
            EwResize => TaoCursorIcon::EwResize,
            NsResize => TaoCursorIcon::NsResize,
            NeswResize => TaoCursorIcon::NeswResize,
            NwseResize => TaoCursorIcon::NwseResize,
            ColResize => TaoCursorIcon::ColResize,
            RowResize => TaoCursorIcon::RowResize,
            #[allow(unreachable_patterns)]
            _ => TaoCursorIcon::Default,
        };

        Self(i)
    }
}

pub struct ProgressStateWrapper(pub TaoProgressState);

impl From<ProgressBarStatus> for ProgressStateWrapper {
    fn from(status: ProgressBarStatus) -> Self {
        let state = match status {
            ProgressBarStatus::None => TaoProgressState::None,
            ProgressBarStatus::Normal => TaoProgressState::Normal,
            ProgressBarStatus::Indeterminate => TaoProgressState::Indeterminate,
            ProgressBarStatus::Paused => TaoProgressState::Paused,
            ProgressBarStatus::Error => TaoProgressState::Error,
        };

        Self(state)
    }
}

pub struct ProgressBarStateWrapper(pub TaoProgressBarState);

impl From<ProgressBarState> for ProgressBarStateWrapper {
    fn from(progress_state: ProgressBarState) -> Self {
        Self(TaoProgressBarState {
            progress: progress_state.progress,
            state: progress_state.status.map(|state| ProgressStateWrapper::from(state).0),
            desktop_filename: progress_state.desktop_filename,
        })
    }
}

/// Wrapper around a [`tao::window::Icon`] that can be created from an [`Icon`].
pub struct TaoIcon(pub TaoWindowIcon);

impl TryFrom<Icon<'_>> for TaoIcon {
    type Error = anyhow::Error;

    fn try_from(icon: Icon<'_>) -> std::result::Result<Self, Self::Error> {
        TaoWindowIcon::from_rgba(icon.rgba.to_vec(), icon.width, icon.height).map(Self).map_err(|e| anyhow::anyhow!(e))
    }
}
