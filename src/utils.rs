use anyhow::{bail, Context, Result};
use itertools::Itertools;
use log::debug;
use regex::Regex;
use std::ffi::CStr;
use std::iter;
use std::thread::sleep;
use std::time::{Duration, Instant};
use xcb::ffi::xcb_visualid_t;

use crate::args::AppConfig;
use crate::{DesktopWindow, RenderWindow};

/// Given a list of `current_hints` and a bunch of `hint_chars`, this finds a unique combination
/// of characters that doesn't yet exist in `current_hints`. `max_count` is the maximum possible
/// number of hints we need.
pub fn get_next_hint(
    current_hints: Vec<&String>,
    hint_chars: &str,
    max_count: usize,
) -> Result<String> {
    // Figure out which size we need.
    let mut size_required = 1;
    while hint_chars.len().pow(size_required) < max_count {
        size_required += 1;
    }
    let mut ret = hint_chars
        .chars()
        .next()
        .context("No hint_chars found")?
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
    Ok(ret)
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

pub fn extents_for_text(text: &str, family: &str, size: f64) -> Result<cairo::TextExtents> {
    // Create a buffer image that should be large enough.
    // TODO: Figure out the maximum size from the largest window on the desktop.
    // For now we'll use made-up maximum values.
    let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 1024, 1024)
        .context("Couldn't create ImageSurface")?;
    let cr = cairo::Context::new(&surface).context("Couldn't create Cairo Surface")?;
    cr.select_font_face(family, cairo::FontSlant::Normal, cairo::FontWeight::Normal);
    cr.set_font_size(size);
    cr.text_extents(text).context("Couldn't create TextExtents")
}

/// Draw a `text` onto `rw`. In case any `current_hints` are already typed, it will draw those in a
/// different color to show that they were in fact typed.
pub fn draw_hint_text(
    rw: &RenderWindow,
    app_config: &AppConfig,
    text: &str,
    current_hints: &str,
) -> Result<()> {
    // Paint background.
    rw.cairo_context.set_operator(cairo::Operator::Source);

    if rw.desktop_window.is_focused {
        rw.cairo_context.set_source_rgb(
            app_config.bg_color_current.0,
            app_config.bg_color_current.1,
            app_config.bg_color_current.2,
        );
    } else {
        rw.cairo_context.set_source_rgb(
            app_config.bg_color.0,
            app_config.bg_color.1,
            app_config.bg_color.2,
        );
    }
    rw.cairo_context.paint().context("Error trying to draw")?;
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
        if rw.desktop_window.is_focused {
            rw.cairo_context.set_source_rgba(
                app_config.text_color_current_alt.0,
                app_config.text_color_current_alt.1,
                app_config.text_color_current_alt.2,
                app_config.text_color_current_alt.3,
            );
        } else {
            rw.cairo_context.set_source_rgba(
                app_config.text_color_alt.0,
                app_config.text_color_alt.1,
                app_config.text_color_alt.2,
                app_config.text_color_alt.3,
            );
        }
        for c in current_hints.chars() {
            rw.cairo_context
                .show_text(&c.to_string())
                .context("Couldn't display text")?;
        }
    }

    // Paint unselected chars.
    if rw.desktop_window.is_focused {
        rw.cairo_context.set_source_rgba(
            app_config.text_color_current.0,
            app_config.text_color_current.1,
            app_config.text_color_current.2,
            app_config.text_color_current.3,
        );
    } else {
        rw.cairo_context.set_source_rgba(
            app_config.text_color.0,
            app_config.text_color.1,
            app_config.text_color.2,
            app_config.text_color.3,
        );
    }
    let re = Regex::new(&format!("^{}", current_hints)).unwrap();
    for c in re.replace(text, "").chars() {
        rw.cairo_context
            .show_text(&c.to_string())
            .context("Couldn't show text")?;
    }
    rw.cairo_context.target().flush();

    Ok(())
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
) -> Result<()> {
    let now = Instant::now();
    loop {
        if now.elapsed() > timeout {
            bail!("Couldn't grab keyboard input within {:?}", now.elapsed());
        }
        let grab_keyboard_cookie = xcb::xproto::grab_keyboard(
            conn,
            true,
            screen.root(),
            xcb::CURRENT_TIME,
            xcb::GRAB_MODE_ASYNC as u8,
            xcb::GRAB_MODE_ASYNC as u8,
        );
        let grab_keyboard_reply = grab_keyboard_cookie
            .get_reply()
            .context("Couldn't communicate with X")?;
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
pub fn snatch_mouse(conn: &xcb::Connection, screen: &xcb::Screen, timeout: Duration) -> Result<()> {
    let now = Instant::now();
    loop {
        if now.elapsed() > timeout {
            bail!("Couldn't grab keyboard input within {:?}", now.elapsed());
        }
        let grab_pointer_cookie = xcb::xproto::grab_pointer(
            conn,
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
            .context("Couldn't communicate with X")?;
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
    if pressed_keys.contains(kstr) {
        pressed_keys.replace_range(pressed_keys.len() - kstr.len().., "");
    }
}

pub fn get_pressed_symbol(conn: &xcb::Connection, event: &xcb::base::GenericEvent) -> u32 {
    let key_press: &xcb::KeyPressEvent = unsafe { xcb::cast_event(event) };
    let syms = xcb_util::keysyms::KeySymbols::new(conn);
    syms.press_lookup_keysym(key_press, 0)
}

pub fn convert_to_string<'a>(symbol: u32) -> Result<&'a str> {
    unsafe {
        CStr::from_ptr(x11::xlib::XKeysymToString(symbol.into()))
            .to_str()
            .context("Couldn't create Rust string from C string")
    }
}

/// Struct helps to write sequence and check if it is found in list of exit sequences
#[derive(Debug, PartialEq, Eq)]
pub struct Sequence {
    sequence: Vec<String>,
}

impl Sequence {
    pub fn new(string: Option<&str>) -> Sequence {
        match string {
            Some(string) => {
                let mut vec: Vec<String> = Sequence::explode(string, "+");

                Sequence::sort(&mut vec);

                Sequence { sequence: vec }
            }
            None => Sequence {
                sequence: Vec::new(),
            },
        }
    }

    fn explode(string: &str, separator: &str) -> Vec<String> {
        string.split(separator).map(|s| s.to_string()).collect()
    }

    /// Sort vector alphabetically
    fn sort(vec: &mut [String]) {
        vec.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));
    }

    pub fn remove(&mut self, key: &str) {
        self.sequence.retain(|x| x != key);
    }

    pub fn push(&mut self, key: String) {
        self.sequence.push(key);
        Sequence::sort(&mut self.sequence);
    }

    /// Sequence is started if more than one key is pressed
    pub fn is_started(&self) -> bool {
        self.sequence.len() > 1
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

    #[test]
    fn test_sequences_equal() {
        let a = Sequence::new(Some("Control_L+Shift_L+a"));
        let b = Sequence::new(Some("Control_L+a+Shift_L"));

        assert_eq!(a, b);

        let mut c = Sequence::new(None);

        c.push("Shift_L".to_owned());
        c.push("Control_L".to_owned());
        c.push("a".to_owned());

        assert_eq!(a, c);
    }

    #[test]
    fn test_sequences_not_equal() {
        let a = Sequence::new(Some("Control_L+Shift_L+a"));
        let b = Sequence::new(Some("Control_L+a"));

        assert_ne!(a, b);

        let mut c = Sequence::new(None);

        c.push("Shift_L".to_owned());
        c.push("a".to_owned());

        assert_ne!(a, c);
    }

    #[test]
    fn test_sequences_is_started() {
        let mut sequence = Sequence::new(None);
        assert!(!sequence.is_started());

        sequence.push("Control_L".to_owned());
        assert!(!sequence.is_started());

        sequence.push("g".to_owned());
        assert!(sequence.is_started());

        sequence.remove("g");

        assert!(!sequence.is_started());
    }
}
