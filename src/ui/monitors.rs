//! Where the screens are, so a small window opened beside another lands on the same screen.
//!
//! egui tells a window only the size of its screen, not where that screen starts; with two
//! screens side by side, a window on the right one sits at x > that width and looks off-screen.

use eframe::egui::{pos2, vec2, Pos2, Rect, Vec2};

/// The screen that holds `p`, in the same global points as a viewport's outer rect.
pub fn screen_at(p: Pos2, size: Option<Vec2>) -> Option<Rect> {
    let all = screens();
    if let Some(s) = all.iter().find(|s| s.contains(p)) {
        return Some(*s);
    }
    // no screen list on this system: screens of this size laid side by side from 0
    let size = size?;
    if !(size.x.is_finite() && size.y.is_finite() && size.x > 0.0 && size.y > 0.0) {
        return None;
    }
    let left = (p.x / size.x).floor() * size.x;
    let top = (p.y / size.y).floor() * size.y;
    Some(Rect::from_min_size(pos2(left, top), size))
}

/// x of a window `w` wide opened beside `beside`: to its right, as Toggl does; to its left only
/// when the right has no room on that screen; never across two screens.
pub fn side_x(beside: Rect, w: f32, gap: f32, screen: Rect) -> f32 {
    let right = beside.right() + gap;
    let left = beside.left() - gap - w;
    if right + w <= screen.right() {
        right
    } else if left >= screen.left() {
        left
    } else {
        (screen.right() - w).max(screen.left())
    }
}

#[cfg(target_os = "macos")]
fn screens() -> Vec<Rect> {
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct CGRect {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
    }
    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGGetActiveDisplayList(max: u32, displays: *mut u32, count: *mut u32) -> i32;
        fn CGDisplayBounds(display: u32) -> CGRect;
    }
    let mut ids = [0u32; 16];
    let mut n = 0u32;
    // SAFETY: the list has room for 16 displays and CoreGraphics writes at most that many
    if unsafe { CGGetActiveDisplayList(16, ids.as_mut_ptr(), &mut n) } != 0 {
        return Vec::new();
    }
    ids[..n as usize]
        .iter()
        .map(|&id| {
            // global points, top-left of the main screen at 0,0: what winit reports for windows
            let b = unsafe { CGDisplayBounds(id) };
            Rect::from_min_size(pos2(b.x as f32, b.y as f32), vec2(b.w as f32, b.h as f32))
        })
        .collect()
}

#[cfg(not(target_os = "macos"))]
fn screens() -> Vec<Rect> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect::from_min_size(pos2(x, y), vec2(w, h))
    }

    #[test]
    fn opens_on_the_right_on_the_right_screen_too() {
        // two 1920-wide screens; the compact window near the left edge of the right one
        let screen = r(1920.0, 0.0, 1920.0, 1080.0);
        let compact = r(2000.0, 100.0, 380.0, 700.0);
        assert_eq!(side_x(compact, 404.0, 8.0, screen), 2388.0);
    }

    #[test]
    fn goes_left_only_without_room_on_the_right() {
        let screen = r(1920.0, 0.0, 1920.0, 1080.0);
        let compact = r(3400.0, 100.0, 380.0, 700.0);
        assert_eq!(side_x(compact, 404.0, 8.0, screen), 3400.0 - 8.0 - 404.0);
    }

    #[test]
    fn never_straddles_two_screens() {
        // as wide as the screen leaves no side with room: kept inside it
        let screen = r(1920.0, 0.0, 1000.0, 800.0);
        let compact = r(2100.0, 0.0, 700.0, 600.0);
        assert_eq!(side_x(compact, 404.0, 8.0, screen), 2920.0 - 404.0);
    }

    #[test]
    fn without_a_screen_list_screens_of_one_size_are_assumed_side_by_side() {
        let s = super::screen_at(pos2(2500.0, 300.0), Some(vec2(1920.0, 1080.0))).unwrap();
        // on macOS the real list wins; there the point may be off every screen in a test run
        if super::screens().is_empty() {
            assert_eq!(s.left(), 1920.0);
        }
    }
}
