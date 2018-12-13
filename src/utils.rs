use cairo;
use cairo::enums::{FontSlant, FontWeight};
use cairo::prelude::SurfaceExt;
use clap::{
    crate_authors, crate_description, crate_name, crate_version, value_t, App, AppSettings, Arg,
};
use css_color_parser::Color as CssColor;
use font_loader::system_fonts;
use itertools::Itertools;
use log::debug;
use regex::Regex;
use std::error::Error;
use std::iter;
use std::str::FromStr;
use std::thread::sleep;
use std::time::{Duration, Instant};
use xcb;
use xcb::ffi::xcb_visualid_t;

use crate::{AppConfig, DesktopWindow, RenderWindow};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HorizontalAlign {
    Left,
    Center,
    Right,
}

impl FromStr for HorizontalAlign {
    type Err = ();

    fn from_str(s: &str) -> Result<HorizontalAlign, ()> {
        match s {
            "left" => Ok(HorizontalAlign::Left),
            "center" => Ok(HorizontalAlign::Center),
            "right" => Ok(HorizontalAlign::Right),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VerticalAlign {
    Top,
    Center,
    Bottom,
}

impl FromStr for VerticalAlign {
    type Err = ();

    fn from_str(s: &str) -> Result<VerticalAlign, ()> {
        match s {
            "top" => Ok(VerticalAlign::Top),
            "center" => Ok(VerticalAlign::Center),
            "bottom" => Ok(VerticalAlign::Bottom),
            _ => Err(()),
        }
    }
}

/// Checks whether the provided fontconfig font `f` is valid.
fn is_truetype_font(f: String) -> Result<(), String> {
    let v: Vec<_> = f.split(':').collect();
    let (family, size) = (v.get(0), v.get(1));
    if family.is_none() || size.is_none() {
        return Err("From font format".to_string());
    }
    if let Err(e) = size.unwrap().parse::<f32>() {
        return Err(e.description().to_string());
    }
    Ok(())
}

/// Validate a color.
fn is_valid_color(c: String) -> Result<(), String> {
    c.parse::<CssColor>().map_err(|_| "Invalid color format")?;
    Ok(())
}

/// Load a system font.
fn load_font(font_family: &str) -> Vec<u8> {
    let font_family_property = system_fonts::FontPropertyBuilder::new()
        .family(font_family)
        .build();
    let (loaded_font, _) =
        if let Some((loaded_font, index)) = system_fonts::get(&font_family_property) {
            (loaded_font, index)
        } else {
            eprintln!("Family not found, falling back to first Monospace font");
            let mut font_monospace_property =
                system_fonts::FontPropertyBuilder::new().monospace().build();
            let sysfonts = system_fonts::query_specific(&mut font_monospace_property);
            eprintln!("Falling back to font '{font}'", font = sysfonts[0]);
            let (loaded_font, index) =
                system_fonts::get(&font_monospace_property).expect("Couldn't find suitable font");
            (loaded_font, index)
        };
    loaded_font
}

/// Parse a color into a tuple of floats.
fn parse_color(color_str: CssColor) -> (f64, f64, f64, f64) {
    (
        f64::from(color_str.r) / 255.0,
        f64::from(color_str.g) / 255.0,
        f64::from(color_str.b) / 255.0,
        f64::from(color_str.a),
    )
}

/// Parse app arguments.
pub fn parse_args() -> AppConfig {
    let matches = App::new(crate_name!())
        .version(crate_version!())
        .author(crate_authors!())
        .about(crate_description!())
        .global_setting(AppSettings::ColoredHelp)
        .arg(
            Arg::with_name("font")
            .short("f")
            .long("font")
            .takes_value(true)
            .validator(is_truetype_font)
            .default_value("Mono:72")
            .help("Use a specific TrueType font with this format: family:size"))
        .arg(
            Arg::with_name("hint_chars")
            .short("c")
            .long("chars")
            .takes_value(true)
            .default_value("sadfjklewcmpgh")
            .help("Define a set of possbile values to use as hint characters"))
        .arg(
            Arg::with_name("margin")
            .short("m")
            .long("margin")
            .takes_value(true)
            .default_value("0.2")
            .help("Add an additional margin around the text box (value is a factor of the box size)"))
        .arg(
            Arg::with_name("text_color")
            .long("textcolor")
            .takes_value(true)
            .validator(is_valid_color)
            .default_value("#dddddd")
            .display_order(49)
            .help("Text color (CSS notation)"))
        .arg(
            Arg::with_name("text_color_alt")
            .long("textcoloralt")
            .takes_value(true)
            .validator(is_valid_color)
            .default_value("#666666")
            .display_order(50)
            .help("Text color alternate (CSS notation)"))
        .arg(
            Arg::with_name("bg_color")
            .long("bgcolor")
            .takes_value(true)
            .validator(is_valid_color)
            .default_value("rgba(30, 30, 30, 0.9)")
            .display_order(51)
            .help("Background color (CSS notation)"))
        .arg(
            Arg::with_name("horizontal_align")
            .long("halign")
            .takes_value(true)
            .possible_values(&["left", "center", "right"])
            .default_value("left")
            .display_order(100)
            .help("Horizontal alignment of the box inside the window"))
        .arg(
            Arg::with_name("vertical_align")
            .long("valign")
            .takes_value(true)
            .possible_values(&["top", "center", "bottom"])
            .default_value("top")
            .display_order(101)
            .help("Vertical alignment of the box inside the window"))
        .arg(
            Arg::with_name("fill")
            .long("fill")
            .conflicts_with_all(&["horizontal_align", "vertical_align", "margin"])
            .display_order(102)
            .help("Completely fill out windows"))
        .arg(
            Arg::with_name("print_only")
            .short("p")
            .long("printonly")
            .help("Print the window id only but don't change focus"))
        .get_matches();

    let font = value_t!(matches, "font", String).unwrap();
    let v: Vec<_> = font.split(':').collect();
    let (font_family, font_size) = (v[0].to_string(), v[1].parse::<f64>().unwrap());
    let hint_chars = value_t!(matches, "hint_chars", String).unwrap();
    let margin = value_t!(matches, "margin", f32).unwrap();
    let text_color_unparsed = value_t!(matches, "text_color", CssColor).unwrap();
    let text_color = parse_color(text_color_unparsed);
    let text_color_alt_unparsed = value_t!(matches, "text_color_alt", CssColor).unwrap();
    let text_color_alt = parse_color(text_color_alt_unparsed);
    let bg_color_unparsed = value_t!(matches, "bg_color", CssColor).unwrap();
    let bg_color = parse_color(bg_color_unparsed);
    let fill = matches.is_present("fill");
    let print_only = matches.is_present("print_only");
    let (horizontal_align, vertical_align) = if fill {
        (HorizontalAlign::Center, VerticalAlign::Center)
    } else {
        (
            value_t!(matches, "horizontal_align", HorizontalAlign).unwrap(),
            value_t!(matches, "vertical_align", VerticalAlign).unwrap(),
        )
    };

    let loaded_font = load_font(&font_family);

    AppConfig {
        font_family,
        font_size,
        loaded_font,
        hint_chars,
        margin,
        text_color,
        text_color_alt,
        bg_color,
        fill,
        print_only,
        horizontal_align,
        vertical_align,
    }
}

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
        &app_config.font_family,
        FontSlant::Normal,
        FontWeight::Normal,
    );
    rw.cairo_context.set_font_size(app_config.font_size);
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
