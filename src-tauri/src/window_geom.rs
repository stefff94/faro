//! Pure window-geometry helpers (no Tauri/OS deps) so they can be unit-tested.

/// A rectangle in physical pixels, top-left origin.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RectPx {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

/// Logical content offset+size within the window (CSS px), reported by the frontend.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ContentFit {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// Physical rectangle of the painted content given the window's outer position,
/// the logical content fit, and the display scale factor.
pub fn content_rect_physical(outer_x: i32, outer_y: i32, fit: ContentFit, scale: f64) -> RectPx {
    let left = outer_x + (fit.x * scale).round() as i32;
    let top = outer_y + (fit.y * scale).round() as i32;
    RectPx {
        left,
        top,
        right: left + (fit.w * scale).round() as i32,
        bottom: top + (fit.h * scale).round() as i32,
    }
}

/// True if the physical point lies inside the rectangle (left/top inclusive,
/// right/bottom exclusive — matches the existing cursor_in_window convention).
pub fn point_in_rect(px: i32, py: i32, r: RectPx) -> bool {
    px >= r.left && px < r.right && py >= r.top && py < r.bottom
}

/// Right-edge anchored window position (physical px): flush to the right of the
/// screen, vertically in the upper quarter. All params are physical px.
pub fn right_edge_position(screen_w: i32, screen_h: i32, win_w: i32, win_h: i32) -> (i32, i32) {
    let x = (screen_w - win_w).max(0);
    let y = ((screen_h - win_h) / 4).max(0);
    (x, y)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_rect_at_scale_1() {
        let fit = ContentFit { x: 40.0, y: 30.0, w: 100.0, h: 50.0 };
        let r = content_rect_physical(1000, 200, fit, 1.0);
        assert_eq!(r, RectPx { left: 1040, top: 230, right: 1140, bottom: 280 });
    }

    #[test]
    fn content_rect_at_scale_2() {
        let fit = ContentFit { x: 40.0, y: 30.0, w: 100.0, h: 50.0 };
        let r = content_rect_physical(1000, 200, fit, 2.0);
        assert_eq!(r, RectPx { left: 1080, top: 260, right: 1280, bottom: 360 });
    }

    #[test]
    fn point_inside_is_true() {
        let r = RectPx { left: 10, top: 10, right: 20, bottom: 20 };
        assert!(point_in_rect(10, 10, r)); // left/top inclusive
        assert!(point_in_rect(19, 19, r));
    }

    #[test]
    fn point_on_right_or_bottom_edge_is_false() {
        let r = RectPx { left: 10, top: 10, right: 20, bottom: 20 };
        assert!(!point_in_rect(20, 15, r)); // right exclusive
        assert!(!point_in_rect(15, 20, r)); // bottom exclusive
    }

    #[test]
    fn point_outside_is_false() {
        let r = RectPx { left: 10, top: 10, right: 20, bottom: 20 };
        assert!(!point_in_rect(5, 15, r));
        assert!(!point_in_rect(15, 5, r));
    }

    #[test]
    fn right_edge_flush_and_upper_quarter() {
        let (x, y) = right_edge_position(2000, 1200, 200, 100);
        assert_eq!(x, 1800); // 2000 - 200
        assert_eq!(y, 275); // (1200 - 100) / 4
    }

    #[test]
    fn right_edge_clamps_oversized_window() {
        let (x, y) = right_edge_position(800, 600, 1000, 800);
        assert_eq!(x, 0);
        assert_eq!(y, 0);
    }
}
