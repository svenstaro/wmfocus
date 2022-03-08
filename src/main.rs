use anyhow::{Context, Result};
use log::{debug, info, warn};
use std::collections::HashMap;
use std::iter::Iterator;
use std::time::Duration;
use xcb::x;

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

    let (conn, screen_num) = xcb::Connection::connect(None).context("No Xorg connection")?;
    let setup = conn.get_setup();
    let screen = setup
        .roots()
        .nth(screen_num as usize)
        .context("Couldn't get screen")?;

    let value_list = &[
        (x::Cw::BackPixel(screen.black_pixel())),
        (x::Cw::OverrideRedirect(true)),
        (x::Cw::EventMask(
            x::EventMask::EXPOSURE
                | x::EventMask::KEY_PRESS
                | x::EventMask::BUTTON_PRESS
                | x::EventMask::BUTTON_RELEASE,
        )),
    ];

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
                (f64::from(desktop_window.size.0) - text_extents.width) / 2.0,
                (f64::from(desktop_window.size.1) - text_extents.height) / 2.0,
            )
        } else {
            let margin_factor = 1.0 + 0.2;
            (
                (text_extents.width * margin_factor).round() as u16,
                (text_extents.height * margin_factor).round() as u16,
                ((text_extents.width * margin_factor) - text_extents.width) / 2.0,
                ((text_extents.height * margin_factor) - text_extents.height) / 2.0,
            )
        };

        // Due to the way cairo lays out text, we'll have to calculate the actual coordinates to
        // put the cursor. See:
        // https://www.cairographics.org/samples/text_align_center/
        // https://www.cairographics.org/samples/text_extents/
        // https://www.cairographics.org/tutorial/#L1understandingtext
        let draw_pos = (
            margin_width - text_extents.x_bearing,
            text_extents.height + margin_height - (text_extents.height + text_extents.y_bearing),
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

        let window: x::Window = conn.generate_id();

        // Create the actual window.
        let create_window_cookie = conn.send_request_checked(&x::CreateWindow {
            depth: x::COPY_FROM_PARENT as u8,
            wid: window,
            parent: screen.root(),
            x,
            y,
            width,
            height,
            border_width: 0,
            class: x::WindowClass::InputOutput,
            visual: screen.root_visual(),
            value_list,
        });
        conn.check_request(create_window_cookie)?;

        let map_window_cookie = conn.send_request_checked(&x::MapWindow { window });
        conn.check_request(map_window_cookie)?;

        // Set transparency.
        let opacity_cookie = conn.send_request(&x::InternAtom {
            only_if_exists: false,
            name: b"_NET_WM_WINDOW_OPACITY",
        });
        let opacity_atom = conn
            .wait_for_reply(opacity_cookie)
            .context("Couldn't create atom _NET_WM_WINDOW_OPACITY")?
            .atom();
        let opacity = (0xFFFFFFFFu64 as f64 * app_config.bg_color.3) as u32;
        conn.send_request(&x::ChangeProperty {
            mode: x::PropMode::Replace,
            window,
            property: opacity_atom,
            r#type: x::ATOM_CARDINAL,
            data: &[opacity],
        });

        conn.flush()?;

        let mut visual =
            utils::find_visual(&conn, screen.root_visual()).context("Couldn't find visual")?;
        let cairo_xcb_conn = unsafe {
            cairo::XCBConnection::from_raw_none(
                conn.get_raw_conn() as *mut cairo_sys::xcb_connection_t
            )
        };
        let cairo_xcb_drawable = cairo::XCBDrawable(window.resource_id());
        let cairo_xcb_visual = unsafe {
            // Lil' dragon den right here, beware.
            let raw_visualtype = std::mem::transmute::<
                &mut x::Visualtype,
                *mut cairo_sys::xcb_visualtype_t,
            >(&mut visual);
            cairo::XCBVisualType::from_raw_none(raw_visualtype)
        };
        let surface = cairo::XCBSurface::create(
            &cairo_xcb_conn,
            &cairo_xcb_drawable,
            &cairo_xcb_visual,
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
    utils::snatch_keyboard(&conn, &screen, Duration::from_secs(1))?;

    // Receive mouse events.
    utils::snatch_mouse(&conn, &screen, Duration::from_secs(1))?;

    // Since we might have lots of windows on the desktop, it might be required
    // to enter a sequence in order to get to the correct window.
    // We'll have to track the keys pressed so far.

    use x11::{keysym, xlib::KeySym};
    use xcb::{Raw, Xid};
    let mut pressed_keys = String::default();
    let mut sequence = utils::Sequence::new(None);

    let mut closed = false;
    while !closed {
        let event = conn.wait_for_event();
        match event {
            Err(_) => {
                closed = true;
            }
            Ok(event) => {
                match event {
                    xcb::Event::X(x::Event::Expose(_)) => {
                        for (hint, rw) in &render_windows {
                            utils::draw_hint_text(rw, &app_config, hint, &pressed_keys)
                                .context("Couldn't draw hint text")?;
                            conn.flush()?;
                        }
                    }
                    xcb::Event::X(x::Event::ButtonPress(_)) => {
                        closed = true;
                    }
                    xcb::Event::X(x::Event::KeyRelease(key_release_event)) => {
                        let keysym = utils::keycode_to_keysym(&conn, key_release_event.detail());
                        dbg!(&keysym);
                        let kstr = utils::keysym_to_string(keysym)
                            .context("Couldn't convert ksym to string")?;
                        sequence.remove(kstr);
                    }
                    xcb::Event::X(x::Event::KeyPress(key_press_event)) => {
                        let keysym = utils::keycode_to_keysym(&conn, key_press_event.detail());
                        dbg!(&keysym);
                        let kstr = utils::keysym_to_string(keysym)
                            .context("Couldn't convert ksym to string")?;

                        sequence.push(kstr.to_owned());

                        if app_config.hint_chars.contains(kstr) {
                            info!("Adding '{}' to key sequence", kstr);
                            pressed_keys.push_str(kstr);
                        } else {
                            warn!("Pressed key '{}' is not a valid hint characters", kstr);
                        }

                        info!("Current key sequence: '{}'", pressed_keys);

                        if keysym == keysym::XK_Escape as KeySym
                            || app_config.exit_keys.contains(&sequence)
                        {
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
                            utils::remove_last_key(&mut pressed_keys, kstr);
                        } else if let Some(rw) = &render_windows.get(&pressed_keys) {
                            info!("Found matching window, focusing");
                            if app_config.print_only {
                                println!("0x{:x}", rw.desktop_window.x_window_id.unwrap_or(0));
                            } else {
                                wm::focus_window(rw.desktop_window)
                                    .context("Couldn't focus window")?;
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
                            utils::remove_last_key(&mut pressed_keys, kstr);
                        }
                    }
                    _ => {}
                }
            }
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
