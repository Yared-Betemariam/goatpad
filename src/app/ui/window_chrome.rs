use super::layout::{RESIZE_BORDER_WIDTH, RESIZE_CORNER_SIZE};
use eframe::egui;

pub(super) fn show_resize_handles(ctx: &egui::Context) {
    if ctx.input(|input| input.viewport().maximized.unwrap_or(false)) {
        return;
    }
    let viewport = ctx.viewport_rect();
    if viewport.width() <= 2.0 * RESIZE_CORNER_SIZE || viewport.height() <= 2.0 * RESIZE_CORNER_SIZE
    {
        return;
    }

    let zones = [
        (
            "north",
            egui::Rect::from_min_max(
                egui::pos2(viewport.left() + RESIZE_CORNER_SIZE, viewport.top()),
                egui::pos2(
                    viewport.right() - RESIZE_CORNER_SIZE,
                    viewport.top() + RESIZE_BORDER_WIDTH,
                ),
            ),
            egui::ResizeDirection::North,
            egui::CursorIcon::ResizeVertical,
        ),
        (
            "south",
            egui::Rect::from_min_max(
                egui::pos2(
                    viewport.left() + RESIZE_CORNER_SIZE,
                    viewport.bottom() - RESIZE_BORDER_WIDTH,
                ),
                egui::pos2(viewport.right() - RESIZE_CORNER_SIZE, viewport.bottom()),
            ),
            egui::ResizeDirection::South,
            egui::CursorIcon::ResizeVertical,
        ),
        (
            "west",
            egui::Rect::from_min_max(
                egui::pos2(viewport.left(), viewport.top() + RESIZE_CORNER_SIZE),
                egui::pos2(
                    viewport.left() + RESIZE_BORDER_WIDTH,
                    viewport.bottom() - RESIZE_CORNER_SIZE,
                ),
            ),
            egui::ResizeDirection::West,
            egui::CursorIcon::ResizeHorizontal,
        ),
        (
            "east",
            egui::Rect::from_min_max(
                egui::pos2(
                    viewport.right() - RESIZE_BORDER_WIDTH,
                    viewport.top() + RESIZE_CORNER_SIZE,
                ),
                egui::pos2(viewport.right(), viewport.bottom() - RESIZE_CORNER_SIZE),
            ),
            egui::ResizeDirection::East,
            egui::CursorIcon::ResizeHorizontal,
        ),
        (
            "north_west",
            egui::Rect::from_min_size(viewport.left_top(), egui::Vec2::splat(RESIZE_CORNER_SIZE)),
            egui::ResizeDirection::NorthWest,
            egui::CursorIcon::ResizeNwSe,
        ),
        (
            "north_east",
            egui::Rect::from_min_size(
                egui::pos2(viewport.right() - RESIZE_CORNER_SIZE, viewport.top()),
                egui::Vec2::splat(RESIZE_CORNER_SIZE),
            ),
            egui::ResizeDirection::NorthEast,
            egui::CursorIcon::ResizeNeSw,
        ),
        (
            "south_west",
            egui::Rect::from_min_size(
                egui::pos2(viewport.left(), viewport.bottom() - RESIZE_CORNER_SIZE),
                egui::Vec2::splat(RESIZE_CORNER_SIZE),
            ),
            egui::ResizeDirection::SouthWest,
            egui::CursorIcon::ResizeNeSw,
        ),
        (
            "south_east",
            egui::Rect::from_min_size(
                egui::pos2(
                    viewport.right() - RESIZE_CORNER_SIZE,
                    viewport.bottom() - RESIZE_CORNER_SIZE,
                ),
                egui::Vec2::splat(RESIZE_CORNER_SIZE),
            ),
            egui::ResizeDirection::SouthEast,
            egui::CursorIcon::ResizeNwSe,
        ),
    ];

    for (name, zone, direction, cursor) in zones {
        egui::Area::new(egui::Id::new(("viewport_resize", name)))
            .order(egui::Order::Foreground)
            .fixed_pos(zone.min)
            .constrain(false)
            .show(ctx, |ui| {
                let response = ui
                    .allocate_response(zone.size(), egui::Sense::drag())
                    .on_hover_cursor(cursor);
                if response.drag_started_by(egui::PointerButton::Primary) {
                    ctx.send_viewport_cmd(egui::ViewportCommand::BeginResize(direction));
                }
            });
    }
}
