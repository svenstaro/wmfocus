use std::collections::HashMap;
use std::iter::Iterator;
use std::time::Duration;

use anyhow::{Context, Result};
use log::{debug, info, warn};
use x11rb::xcb_ffi::XCBConnection;

use x11rb::{
    self,
    connection::Connection,
    protocol::xproto::{self, ConnectionExt as _},
    protocol::Event,
    wrapper::ConnectionExt,
};

mod args;
mod utils;

#[cfg(feature = "i3")]
extern crate i3ipc;

#[cfg(feature = "i3")]
mod wm_i3;

#[cfg(feature = "i3")]
use crate::wm_i3 as wm;

#[derive(Debug)]
pub struct DesktopWindow {
    id: i64,
    x_window_id: Option<i32>,
    pos: (i32, i32),
    size: (i32, i32),
    is_focused: bool,
}

#[derive(Debug)]
pub struct RenderWindow<'a> {
    desktop_window: &'a DesktopWindow,
    cairo_context: cairo::Context,
    draw_pos: (f64, f64),
    rect: (i32, i32, i32, i32),
}

#[cfg(any(feature = "i3", feature = "add_some_other_wm_here"))]
fn main() -> Result<()> {
    pretty_env_logger::init();
    let app_config = args::parse_args();

    // Get the windows from each specific window manager implementation.
    let desktop_windows_raw = wm::get_windows().context("Couldn't get desktop windows")?;

    // Sort by position to make hint position more deterministic.
    let desktop_windows = utils::sort_by_pos(desktop_windows_raw);

    let (conn, screen_num) = XCBConnection::connect(None).context("No Xorg connection")?;
    let screen = &conn.setup().roots[screen_num];

    // Assemble RenderWindows from DesktopWindows.
    let mut render_windows = HashMap::new();
    for desktop_window in &desktop_windows {
        // We need to estimate the font size before rendering because we want the window to only be
        // the size of the font.
        let hint = utils::get_next_hint(
            render_windows.keys().collect(),
            &app_config.hint_chars,
            desktop_windows.len(),
        )
        .context("Couldn't get next hint")?;

        // Figure out how large the window actually needs to be.
        let text_extents = utils::extents_for_text(
            &hint,
            &app_config.font.font_family,
            app_config.font.font_size,
        )
        .context("Couldn't create extents for text")?;
        let (width, height, margin_width, margin_height) = if app_config.fill {
            (
                desktop_window.size.0 as u16,
                desktop_window.size.1 as u16,
                (f64::from(desktop_window.size.0) - text_extents.width()) / 2.0,
                (f64::from(desktop_window.size.1) - text_extents.height()) / 2.0,
            )
        } else {
            let margin_factor = 1.0 + 0.2;
            (
                (text_extents.width() * margin_factor).round() as u16,
                (text_extents.height() * margin_factor).round() as u16,
                ((text_extents.width() * margin_factor) - text_extents.width()) / 2.0,
                ((text_extents.height() * margin_factor) - text_extents.height()) / 2.0,
            )
        };

        // Due to the way cairo lays out text, we'll have to calculate the actual coordinates to
        // put the cursor. See:
        // https://www.cairographics.org/samples/text_align_center/
        // https://www.cairographics.org/samples/text_extents/
        // https://www.cairographics.org/tutorial/#L1understandingtext
        let draw_pos = (
            margin_width - text_extents.x_bearing(),
            text_extents.height() + margin_height
                - (text_extents.height() + text_extents.y_bearing()),
        );

        debug!(
            "Spawning RenderWindow for this DesktopWindow: {:?}",
            desktop_window
        );

        let x_offset = app_config.offset.x;
        let mut x = match app_config.horizontal_align {
            args::HorizontalAlign::Left => (desktop_window.pos.0 + x_offset) as i16,
            args::HorizontalAlign::Center => {
                (desktop_window.pos.0 + desktop_window.size.0 / 2 - i32::from(width) / 2) as i16
            }
            args::HorizontalAlign::Right => {
                (desktop_window.pos.0 + desktop_window.size.0 - i32::from(width) - x_offset) as i16
            }
        };

        let y_offset = app_config.offset.y;
        let y = match app_config.vertical_align {
            args::VerticalAlign::Top => (desktop_window.pos.1 + y_offset) as i16,
            args::VerticalAlign::Center => {
                (desktop_window.pos.1 + desktop_window.size.1 / 2 - i32::from(height) / 2) as i16
            }
            args::VerticalAlign::Bottom => {
                (desktop_window.pos.1 + desktop_window.size.1 - i32::from(height) - y_offset) as i16
            }
        };

        // If this is overlapping then we'll nudge the new RenderWindow a little bit out of the
        // way.
        let mut overlaps = utils::find_overlaps(
            render_windows.values().collect(),
            (x.into(), y.into(), width.into(), height.into()),
        );
        while !overlaps.is_empty() {
            x += overlaps.pop().unwrap().2 as i16;
            overlaps = utils::find_overlaps(
                render_windows.values().collect(),
                (x.into(), y.into(), width.into(), height.into()),
            );
        }

        let xcb_window_id = conn.generate_id()?;

        let win_aux = xproto::CreateWindowAux::new()
            .event_mask(
                xproto::EventMask::EXPOSURE
                    | xproto::EventMask::KEY_PRESS
                    | xproto::EventMask::BUTTON_PRESS
                    | xproto::EventMask::BUTTON_RELEASE,
            )
            .backing_pixel(screen.black_pixel)
            .override_redirect(1);

        // Create the actual window.
        xproto::create_window(
            &conn,
            x11rb::COPY_FROM_PARENT as u8,
            xcb_window_id,
            screen.root,
            x,
            y,
            width,
            height,
            0,
            xproto::WindowClass::INPUT_OUTPUT,
            screen.root_visual,
            &win_aux,
        )?;

        conn.map_window(xcb_window_id)?;

        // Set transparency.
        let opacity_atom = conn
            .intern_atom(false, b"_NET_WM_WINDOW_OPACITY")?
            .reply()
            .context("Couldn't create atom _NET_WM_WINDOW_OPACITY")?
            .atom;
        let opacity = (0xFFFFFFFFu64 as f64 * app_config.bg_color.3) as u64;
        conn.change_property32(
            xproto::PropMode::REPLACE,
            xcb_window_id,
            opacity_atom,
            xproto::AtomEnum::CARDINAL,
            &[opacity as u32],
        )?;

        conn.flush()?;

        let mut visual = utils::find_xcb_visualtype(&conn, screen.root_visual)
            .context("Couldn't find visual")?;
        let cairo_conn =
            unsafe { cairo::XCBConnection::from_raw_none(conn.get_raw_xcb_connection() as _) };
        let cairo_visual =
            unsafe { cairo::XCBVisualType::from_raw_none(&mut visual as *mut _ as _) };

        let surface = cairo::XCBSurface::create(
            &cairo_conn,
            &cairo::XCBDrawable(xcb_window_id),
            &cairo_visual,
            width.into(),
            height.into(),
        )
        .context("Couldn't create Cairo Surface")?;
        let cairo_context =
            cairo::Context::new(&surface).context("Couldn't create Cairo Context")?;

        let render_window = RenderWindow {
            desktop_window,
            cairo_context,
            draw_pos,
            rect: (x.into(), y.into(), width.into(), height.into()),
        };

        render_windows.insert(hint, render_window);
    }

    // Receive keyboard events.
    utils::snatch_keyboard(&conn, screen, Duration::from_secs(1))?;

    // Receive mouse events.
    utils::snatch_mouse(&conn, screen, Duration::from_secs(1))?;

    // Since we might have lots of windows on the desktop, it might be required
    // to enter a sequence in order to get to the correct window.
    // We'll have to track the keys pressed so far.
    let mut pressed_keys = String::default();
    let mut sequence = utils::Sequence::new(None);

    let mut closed = false;
    while !closed {
        let event = conn.wait_for_event().context("No events")?;
        let event_option = Some(event);
        if let Some(e) = event_option {
            match e {
                Event::Expose(_) => {
                    for (hint, rw) in &render_windows {
                        utils::draw_hint_text(rw, &app_config, hint, &pressed_keys)
                            .context("Couldn't draw hint text")?;
                        conn.flush()?;
                    }
                }
                Event::ButtonPress(_) => {
                    closed = true;
                }
                Event::KeyRelease(_) => {
                    let ksym = utils::get_pressed_symbol(&conn, e);
                    let kstr = xkeysym::name(ksym)
                        .context("Couldn't convert ksym to string")?
                        .replace("XK_", "");
                    sequence.remove(&kstr);
                }
                Event::KeyPress(_) => {
                    let ksym = utils::get_pressed_symbol(&conn, e);
                    let kstr = xkeysym::name(ksym)
                        .context("Couldn't convert ksym to string")?
                        .replace("XK_", "");

                    sequence.push(kstr.to_owned());

                    if app_config.hint_chars.contains(&kstr) {
                        info!("Adding '{}' to key sequence", kstr);
                        pressed_keys.push_str(&kstr);
                    } else {
                        warn!("Pressed key '{}' is not a valid hint characters", kstr);
                    }

                    info!("Current key sequence: '{}'", pressed_keys);

                    if ksym == xkeysym::KEY_Escape || app_config.exit_keys.contains(&sequence) {
                        info!("{:?} is exit sequence", sequence);
                        closed = true;
                        continue;
                    }

                    // Attempt to match the current sequence of keys as a string to the window
                    // hints shown.
                    // If there is an exact match, we're done. We'll then focus the window
                    // and exit. However, we also want to check whether there is still any
                    // chance to focus any windows from the current key sequence. If there
                    // is not then we will also just exit and focus no new window.
                    // If there still is a chance we might find a window then we'll just
                    // keep going for now.
                    if sequence.is_started() {
                        utils::remove_last_key(&mut pressed_keys, &kstr);
                    } else if let Some(rw) = &render_windows.get(&pressed_keys) {
                        info!("Found matching window, focusing");
                        if app_config.print_only {
                            println!("0x{:x}", rw.desktop_window.x_window_id.unwrap_or(0));
                        } else {
                            wm::focus_window(rw.desktop_window).context("Couldn't focus window")?;
                        }
                        closed = true;
                    } else if !pressed_keys.is_empty()
                        && render_windows.keys().any(|k| k.starts_with(&pressed_keys))
                    {
                        for (hint, rw) in &render_windows {
                            utils::draw_hint_text(rw, &app_config, hint, &pressed_keys)
                                .context("Couldn't draw hint text")?;
                            conn.flush()?;
                        }
                        continue;
                    } else {
                        warn!("No more matches possible with current key sequence");
                        closed = app_config.exit_keys.is_empty();
                        utils::remove_last_key(&mut pressed_keys, &kstr);
                    }
                }
                _ => {}
            }
        } else {
            closed = true;
        }
    }

    Ok(())
}

#[cfg(not(any(feature = "i3", feature = "add_some_other_wm_here")))]
fn main() -> Result<()> {
    eprintln!(
        "You need to enable support for at least one window manager.\n
Currently supported:
    --features i3"
    );

    Ok(())
}
