use itertools::Itertools;
use log::debug;
use regex::Regex;
use std::iter;
use std::thread::sleep;
use std::time::{Duration, Instant};
use xcb::ffi::xcb_visualid_t;

use crate::args::AppConfig;
use crate::{DesktopWindow, RenderWindow};

/// Given a list of `current_hints` and a bunch of `hint_chars`, this finds a unique combination
/// of characters that doesn't yet exist in `current_hints`. `max_count` is the maximum possible
/// number of hints we need.
pub fn get_next_hint(current_hints: Vec<&String>, hint_chars: &str, max_count: usize) -> String {
    // Figure out which size we need.
    let mut size_required = 1;
    while hint_chars.len().pow(size_required) < max_count {
        size_required += 1;
    }
    let mut ret = hint_chars
        .chars()
        .next()
        .expect("No hint_chars found")
        .to_string();
    let it = iter::repeat(hint_chars.chars().rev())
        .take(size_required as usize)
        .multi_cartesian_product();
    for c in it {
        let folded = c.into_iter().collect();
        if !current_hints.contains(&&folded) {
            ret = folded;
        }
    }
    debug!("Returning next hint: {}", ret);
    ret
}

pub fn find_visual(conn: &xcb::Connection, visual: xcb_visualid_t) -> Option<xcb::Visualtype> {
    for screen in conn.get_setup().roots() {
        for depth in screen.allowed_depths() {
            for vis in depth.visuals() {
                if visual == vis.visual_id() {
                    return Some(vis);
                }
            }
        }
    }
    None
}

pub fn extents_for_text(text: &str, family: &str, size: f64) -> cairo::TextExtents {
    // Create a buffer image that should be large enough.
    // TODO: Figure out the maximum size from the largest window on the desktop.
    // For now we'll use made-up maximum values.
    let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 1024, 1024)
        .expect("Couldn't create ImageSurface");
    let cr = cairo::Context::new(&surface);
    cr.select_font_face(family, cairo::FontSlant::Normal, cairo::FontWeight::Normal);
    cr.set_font_size(size);
    cr.text_extents(text)
}

/// Draw a `text` onto `rw`. In case any `current_hints` are already typed, it will draw those in a
/// different color to show that they were in fact typed.
pub fn draw_hint_text(rw: &RenderWindow, app_config: &AppConfig, text: &str, current_hints: &str) {
    // Paint background.
    rw.cairo_context.set_operator(cairo::Operator::Source);
    rw.cairo_context.set_source_rgb(
        app_config.bg_color.0,
        app_config.bg_color.1,
        app_config.bg_color.2,
    );
    rw.cairo_context.paint();
    rw.cairo_context.set_operator(cairo::Operator::Over);

    rw.cairo_context.select_font_face(
        &app_config.font.font_family,
        cairo::FontSlant::Normal,
        cairo::FontWeight::Normal,
    );
    rw.cairo_context.set_font_size(app_config.font.font_size);
    rw.cairo_context.move_to(rw.draw_pos.0, rw.draw_pos.1);
    if text.starts_with(current_hints) {
        // Paint already selected chars.
        rw.cairo_context.set_source_rgba(
            app_config.text_color_alt.0,
            app_config.text_color_alt.1,
            app_config.text_color_alt.2,
            app_config.text_color_alt.3,
        );
        for c in current_hints.chars() {
            rw.cairo_context.show_text(&c.to_string());
        }
    }

    // Paint unselected chars.
    rw.cairo_context.set_source_rgba(
        app_config.text_color.0,
        app_config.text_color.1,
        app_config.text_color.2,
        app_config.text_color.3,
    );
    let re = Regex::new(&format!("^{}", current_hints)).unwrap();
    for c in re.replace(text, "").chars() {
        rw.cairo_context.show_text(&c.to_string());
    }
    rw.cairo_context.get_target().flush();
}

/// Try to grab the keyboard until `timeout` is reached.
///
/// Generally with X, I found that you can't grab global keyboard input without it failing
/// sometimes due to other clients grabbing it occasionally. Hence, we'll have to keep retrying
/// until we eventually succeed.
pub fn snatch_keyboard(
    conn: &xcb::Connection,
    screen: &xcb::Screen,
    timeout: Duration,
) -> Result<(), String> {
    let now = Instant::now();
    loop {
        if now.elapsed() > timeout {
            return Err(format!(
                "Couldn't grab keyboard input within {:?}",
                now.elapsed()
            ));
        }
        let grab_keyboard_cookie = xcb::xproto::grab_keyboard(
            &conn,
            true,
            screen.root(),
            xcb::CURRENT_TIME,
            xcb::GRAB_MODE_ASYNC as u8,
            xcb::GRAB_MODE_ASYNC as u8,
        );
        let grab_keyboard_reply = grab_keyboard_cookie
            .get_reply()
            .map_err(|_| "Couldn't communicate with X")?;
        if grab_keyboard_reply.status() == xcb::GRAB_STATUS_SUCCESS as u8 {
            return Ok(());
        }
        sleep(Duration::from_millis(1));
    }
}

/// Try to grab the mouse until `timeout` is reached.
///
/// Generally with X, I found that you can't grab global mouse input without it failing sometimes
/// due to other clients grabbing it occasionally. Hence, we'll have to keep retrying until we
/// eventually succeed.
pub fn snatch_mouse(
    conn: &xcb::Connection,
    screen: &xcb::Screen,
    timeout: Duration,
) -> Result<(), String> {
    let now = Instant::now();
    loop {
        if now.elapsed() > timeout {
            return Err(format!(
                "Couldn't grab keyboard input within {:?}",
                now.elapsed()
            ));
        }
        let grab_pointer_cookie = xcb::xproto::grab_pointer(
            &conn,
            true,
            screen.root(),
            xcb::EVENT_MASK_BUTTON_PRESS as u16,
            xcb::GRAB_MODE_ASYNC as u8,
            xcb::GRAB_MODE_ASYNC as u8,
            xcb::NONE,
            xcb::NONE,
            xcb::CURRENT_TIME,
        );
        let grab_pointer_reply = grab_pointer_cookie
            .get_reply()
            .map_err(|_| "Couldn't communicate with X")?;
        if grab_pointer_reply.status() == xcb::GRAB_STATUS_SUCCESS as u8 {
            return Ok(());
        }
        sleep(Duration::from_millis(1));
    }
}

/// Sort list of `DesktopWindow`s by position.
///
/// This sorts by column first and row second.
pub fn sort_by_pos(mut dws: Vec<DesktopWindow>) -> Vec<DesktopWindow> {
    dws.sort_by_key(|w| w.pos.0);
    dws.sort_by_key(|w| w.pos.1);
    dws
}

/// Returns true if `r1` and `r2` overlap.
fn intersects(r1: (i32, i32, i32, i32), r2: (i32, i32, i32, i32)) -> bool {
    let left_corner_inside = r1.0 < r2.0 + r2.2;
    let right_corner_inside = r1.0 + r1.2 > r2.0;
    let top_corner_inside = r1.1 < r2.1 + r2.3;
    let bottom_corner_inside = r1.1 + r1.3 > r2.1;
    left_corner_inside && right_corner_inside && top_corner_inside && bottom_corner_inside
}

/// Finds overlaps and returns a list of those rects in the format (x, y, w, h).
pub fn find_overlaps(
    rws: Vec<&RenderWindow>,
    rect: (i32, i32, i32, i32),
) -> Vec<(i32, i32, i32, i32)> {
    let mut overlaps = vec![];
    for rw in rws {
        if intersects(rw.rect, rect) {
            overlaps.push(rw.rect);
        }
    }
    overlaps
}

/// Remove last pressed key from pressed keys
pub fn remove_last_key(pressed_keys: &mut String, kstr: &str) {
    pressed_keys.replace_range(pressed_keys.len() - kstr.len().., "");
}

/// Struct helps to write sequence and check if it is found in list of exit sequences
pub struct ExitSequence<'a> {
    sequence: Vec<String>,
    exit_keys: &'a Vec<String>
}

impl<'a> ExitSequence<'a> {
    pub fn new(exit_keys: &'a Vec<String>) -> ExitSequence<'a> {

        ExitSequence {
            sequence: Vec::new(),
            exit_keys: exit_keys,
        }
    }

    pub fn pop(&mut self) -> Option<String> {
        self.sequence.pop()
    }

    pub fn push(&mut self, key: String) {
        self.sequence.push(key);
    }

    pub fn is_exit(&self) -> bool {
        let separator = "+";
        self.exit_keys.contains(&self.sequence.join(separator))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_intersects() {
        assert!(intersects((1905, 705, 31, 82), (1905, 723, 38, 64)));
    }

    #[test]
    fn test_no_intersect() {
        assert!(!intersects((1905, 705, 31, 82), (2000, 723, 38, 64)));
    }
}
