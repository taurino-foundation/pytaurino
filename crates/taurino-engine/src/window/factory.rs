use {
    crate::app::index::HTML,
    anyhow::{Context, Result},
    tao::window::Window,
    taurino_config::{
        EngineLoopProxy, EngineWindowTarget,
        dispatcher::{all_initialization_scripts, install_ipc_bridge},
        window::WindowConfig,
    },
    taurino_core::window::WindowBuilder,
    wry::{WebView, WebViewBuilder},
};
pub fn create_window(
    event_loop: &EngineWindowTarget,
    proxy: EngineLoopProxy,
    window_options: &WindowConfig,
) -> Result<(Window, WebView)> {
    let window_builder = WindowBuilder::with_config(window_options);
    let window = window_builder.inner.build(event_loop)?;
    let mut builder = WebViewBuilder::new();
    for script in all_initialization_scripts("main", "root", true, None)? {
        builder = builder.with_initialization_script_for_main_only(script.script, script.for_main_frame_only);
    }

    builder = install_ipc_bridge(builder, "root".to_owned(), proxy);
    let webview = builder.with_html(HTML).build(&window).context("Failed to create WebView")?;

    Ok((window, webview))
}

/*

  if window_builder.prevent_overflow.is_some() || window_builder.center {
    let monitor = if let Some(window_position) = &window_builder.inner.window.position {
      find_monitor_for_position(window_target.available_monitors(), *window_position)
    } else {
      window_target.primary_monitor()
    };
    if let Some(monitor) = monitor {
      let scale_factor = monitor.scale_factor();
      let desired_size = window_builder
        .inner
        .window
        .inner_size
        .unwrap_or_else(|| PhysicalSize::new(800, 600).into());
      let mut inner_size = window_builder
        .inner
        .window
        .inner_size_constraints
        .clamp(desired_size, scale_factor)
        .to_physical::<u32>(scale_factor);
      let mut window_size = inner_size;
      #[cfg(windows)]
      let decorations = window_builder.inner.window.decorations;
      // Left and right window shadow counts as part of the window on Windows
      // We need to include it when calculating positions, but not size
      #[cfg(windows)]
      let shadow_width = apply_shadow_correction(decorations, &mut window_size)?;
      #[cfg(not(windows))]
      let shadow_width = 0;
      if let Some(margin) = window_builder.prevent_overflow {
        let work_area = monitor.work_area();
        let margin = margin.to_physical::<u32>(scale_factor);
        let constraint = PhysicalSize::new(
          work_area.size.width - margin.width,
          work_area.size.height - margin.height,
        );
        if window_size.width > constraint.width || window_size.height > constraint.height {
          if window_size.width > constraint.width {
            inner_size.width = inner_size
              .width
              .saturating_sub(window_size.width - constraint.width);
            window_size.width = constraint.width;
          }
          if window_size.height > constraint.height {
            inner_size.height = inner_size
              .height
              .saturating_sub(window_size.height - constraint.height);
            window_size.height = constraint.height;
          }
          window_builder.inner.window.inner_size = Some(inner_size.into());
        }
      }
      if window_builder.center {
        window_size.width += shadow_width;
        let position = calculate_window_center_position(window_size, monitor);
        let logical_position = position.to_logical::<f64>(scale_factor);
        window_builder = window_builder.position(logical_position.x, logical_position.y);
      }
    }
  };
  #[cfg(any(target_os = "macos", target_os = "linux"))]
  let (initial_position, is_fullscreen) = (
    window_builder.inner.window.position,
    window_builder.inner.window.fullscreen.is_some(),
  );
  // If fullscreen is requested with an explicit position, resolve the target
  // monitor up front so the window is created fullscreen on that display.
  #[cfg(any(target_os = "macos", target_os = "linux"))]
  if let (true, Some(position)) = (is_fullscreen, initial_position) {
    if let Some(target_monitor) =
      find_monitor_for_position(window_target.available_monitors(), position)
    {
      window_builder.inner.window.fullscreen = Some(Fullscreen::Borderless(Some(target_monitor)));
    }
  }
  #[cfg(windows)]
  let background_color = window_builder.inner.window.background_color;
  #[cfg(windows)]
  let is_window_transparent = window_builder.inner.window.transparent;
  #[cfg(target_os = "macos")]
  {
    if window_builder.tabbing_identifier.is_none()
      || window_builder.inner.window.transparent
      || !window_builder.inner.window.decorations
    {
      window_builder.inner = window_builder.inner.with_automatic_window_tabbing(false);
    }
  }



*/
