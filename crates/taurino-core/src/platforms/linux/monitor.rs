// Copyright 2019-2024 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use {
    dpi::{LogicalPosition, LogicalSize},
    gtk::prelude::MonitorExt,
    tao::platform::unix::MonitorHandleExtUnix,
    taurino_config::PhysicalRect,
};

impl crate::platforms::MonitorExt for tao::monitor::MonitorHandle {
    fn work_area(&self) -> PhysicalRect<i32, u32> {
        let rect = self.gdk_monitor().workarea();
        let scale_factor = self.scale_factor();
        PhysicalRect {
            size: LogicalSize::new(rect.width() as u32, rect.height() as u32).to_physical(scale_factor),
            position: LogicalPosition::new(rect.x(), rect.y()).to_physical(scale_factor),
        }
    }
}
