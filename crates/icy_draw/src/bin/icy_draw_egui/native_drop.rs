//! X11 supplies file-drop events without cursor motion. Query the native window during a drag.

use eframe::egui;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use x11rb::{protocol::xproto::ConnectionExt, rust_connection::RustConnection};

pub struct DropPointer {
    connection: RustConnection,
    window: u32,
}

impl DropPointer {
    pub fn new(creation: &eframe::CreationContext<'_>) -> Result<Option<Self>, String> {
        let handle = creation
            .window_handle()
            .map_err(|error| format!("Cannot inspect native drop window: {error}"))?
            .as_raw();
        let window = match handle {
            RawWindowHandle::Xlib(handle) => u32::try_from(handle.window).map_err(|_| "X11 window ID is out of range")?,
            RawWindowHandle::Xcb(handle) => handle.window.get(),
            _ => return Ok(None),
        };
        let (connection, _) = x11rb::connect(None).map_err(|error| format!("Cannot connect to X11 for drop-position tracking: {error}"))?;
        Ok(Some(Self { connection, window }))
    }

    pub fn position(&self, pixels_per_point: f32) -> Result<Option<egui::Pos2>, String> {
        let reply = self
            .connection
            .query_pointer(self.window)
            .map_err(|error| format!("Cannot query X11 drop position: {error}"))?
            .reply()
            .map_err(|error| format!("Cannot read X11 drop position: {error}"))?;
        position(reply.same_screen, reply.win_x, reply.win_y, pixels_per_point)
    }
}

fn position(same_screen: bool, x: i16, y: i16, pixels_per_point: f32) -> Result<Option<egui::Pos2>, String> {
    if !pixels_per_point.is_finite() || pixels_per_point <= 0.0 {
        return Err("Invalid scale for native drop position.".into());
    }
    Ok(same_screen.then(|| egui::pos2(f32::from(x) / pixels_per_point, f32::from(y) / pixels_per_point)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires a running X11 display; creates only an unmapped test window"]
    fn queries_an_actual_x11_window_without_moving_the_cursor() {
        use x11rb::{
            connection::Connection,
            protocol::xproto::{CreateWindowAux, WindowClass},
            COPY_DEPTH_FROM_PARENT,
        };
        let (connection, screen) = x11rb::connect(None).unwrap();
        let root = connection.setup().roots[screen].root;
        let window = connection.generate_id().unwrap();
        connection
            .create_window(
                COPY_DEPTH_FROM_PARENT,
                window,
                root,
                0,
                0,
                32,
                32,
                0,
                WindowClass::INPUT_OUTPUT,
                0,
                &CreateWindowAux::default(),
            )
            .unwrap()
            .check()
            .unwrap();
        let pointer = DropPointer { connection, window };
        assert!(pointer.position(1.0).unwrap().is_some());
        pointer.connection.destroy_window(window).unwrap().check().unwrap();
    }

    #[test]
    fn native_coordinates_use_window_space_and_current_egui_scale() {
        assert_eq!(position(true, 900, 300, 1.5).unwrap(), Some(egui::pos2(600.0, 200.0)));
        assert_eq!(position(true, -30, 60, 2.0).unwrap(), Some(egui::pos2(-15.0, 30.0)));
        assert_eq!(position(false, 10, 20, 1.0).unwrap(), None);
        for scale in [0.0, -1.0, f32::NAN, f32::INFINITY] {
            assert!(position(true, 10, 20, scale).is_err());
        }
    }
}
