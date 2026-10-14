use taurino_config::PhysicalRect;

#[cfg(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd"
))]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

pub trait MonitorExt {
    /// Get the work area of this monitor
    ///
    /// ## Platform-specific:
    ///
    /// - **Android / iOS**: Unsupported.
    fn work_area(&self) -> PhysicalRect<i32, u32>;
}

pub trait WindowExt {
    /// Enable or disable the window
    ///
    /// ## Platform-specific:
    ///
    /// - **Android / iOS**: Unsupported.
    fn set_enabled(&self, enabled: bool);

    /// Whether the window is enabled or disabled.
    ///
    /// ## Platform-specific:
    ///
    /// - **Android / iOS**: Unsupported, always returns `true`.
    fn is_enabled(&self) -> bool;

    /// Center the window
    ///
    /// ## Platform-specific:
    ///
    /// - **Android / iOS**: Unsupported.
    fn center(&self) {}

    /// Clears the window surface. i.e make it transparent.
    #[cfg(windows)]
    fn draw_surface(
        &self,
        surface: &mut softbuffer::Surface<std::sync::Arc<tao::window::Window>, std::sync::Arc<tao::window::Window>>,
        background_color: Option<tao::window::RGBA>,
    );
}

#[cfg(any(
    windows,
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd"
))]
pub mod undecorated_resizing;
mod util;
