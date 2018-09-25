#[macro_use]
extern crate clap;
#[macro_use]
extern crate log;
extern crate pretty_env_logger;
extern crate css_color_parser;
extern crate font_loader;
extern crate itertools;
extern crate cairo;
extern crate cairo_sys;
extern crate xcb;
extern crate xcb_util;
extern crate xkbcommon;

use cairo::enums::{FontSlant, FontWeight};
use cairo::prelude::SurfaceExt;
use xcb::ffi::xcb_visualid_t;
use xcb::Visualtype;
use xkbcommon::xkb;

use std::iter::Iterator;
use std::collections::HashMap;


mod utils;

#[cfg(feature = "i3")]
extern crate i3ipc;

#[cfg(feature = "i3")]
mod wm_i3;

#[cfg(feature = "i3")]
use wm_i3 as wm;

#[derive(Debug)]
pub struct DesktopWindow {
    id: i64,
    title: String,
    pos: (i32, i32),
    size: (i32, i32),
}

pub struct RenderWindow<'a> {
    desktop_window: &'a DesktopWindow,
    xcb_window_id: u32,
    cairo_context: cairo::Context,
    size: (u16, u16),
    text_extents: cairo::TextExtents,
}

#[derive(Debug)]
pub struct AppConfig {
    pub font_family: String,
    pub font_size: f64,
    pub loaded_font: Vec<u8>,
    pub margin: f32,
    pub text_color: (f32, f32, f32, f32),
    pub bg_color: (f32, f32, f32, f32),
    pub fill: bool,
    pub horizontal_align: utils::HorizontalAlign,
    pub vertical_align: utils::VerticalAlign,
}

static HINT_CHARS: &'static str = "sadfjklewcmpgh";

#[cfg(any(feature = "i3", feature = "add_some_other_wm_here"))]
fn main() {
    let app_config = utils::parse_args();

    // Get the windows from each specific window manager implementation.
    let desktop_windows = wm::get_windows();

    let (conn, screen_num) = xcb::Connection::connect(None).unwrap();
    let setup = conn.get_setup();
    let screen = setup.roots().nth(screen_num as usize).unwrap();

    let values = [
        (
            xcb::CW_EVENT_MASK,
            xcb::EVENT_MASK_EXPOSURE
                | xcb::EVENT_MASK_KEY_PRESS
                | xcb::EVENT_MASK_BUTTON_PRESS
                | xcb::EVENT_MASK_BUTTON_RELEASE,
        ),
        (xcb::CW_OVERRIDE_REDIRECT, 1),
    ];

    let mut render_windows = HashMap::new();
    for desktop_window in &desktop_windows {
        // We need to estimate the font size before rendering because we want the window to only be
        // the size of the font.
        let hint = utils::get_next_hint(
            render_windows.keys().collect(),
            HINT_CHARS,
            desktop_windows.len(),
        );

        // Figure out how large the window actually needs to be.
        let text_extents = utils::extents_for_text(&hint, &app_config.font_family, app_config.font_size);
        let (width, height) = if app_config.fill {
            (desktop_window.size.0 as u16, desktop_window.size.1 as u16)
        } else {
            let border_factor = 1.0 + 0.2;
            (
                (text_extents.width * border_factor).round() as u16,
                (text_extents.height * border_factor).round() as u16,
            )
        };

        debug!("Spawning RenderWindow for this DesktopWindow: {:?}", desktop_window);

        let x = match app_config.horizontal_align {
            utils::HorizontalAlign::Left => desktop_window.pos.0 as i16,
            utils::HorizontalAlign::Center => {
                (desktop_window.pos.0 + desktop_window.size.0 / 2 - width as i32 / 2) as i16
            }
            utils::HorizontalAlign::Right => {
                (desktop_window.pos.0 + desktop_window.size.0 - width as i32) as i16
            }
        };

        let y = match app_config.vertical_align {
            utils::VerticalAlign::Top => desktop_window.pos.1 as i16,
            utils::VerticalAlign::Center => {
                (desktop_window.pos.1 + desktop_window.size.1 / 2 - height as i32 / 2) as i16
            }
            utils::VerticalAlign::Bottom => {
                (desktop_window.pos.1 + desktop_window.size.1 - height as i32) as i16
            }
        };

        let xcb_window_id = conn.generate_id();

        xcb::create_window(
            &conn,
            xcb::COPY_FROM_PARENT as u8,
            xcb_window_id,
            screen.root(),
            x,
            y,
            width,
            height,
            0,
            xcb::WINDOW_CLASS_INPUT_OUTPUT as u16,
            screen.root_visual(),
            &values,
        );

        xcb::map_window(&conn, xcb_window_id);

        // Set title.
        let title = crate_name!();
        xcb::change_property(
            &conn,
            xcb::PROP_MODE_REPLACE as u8,
            xcb_window_id,
            xcb::ATOM_WM_NAME,
            xcb::ATOM_STRING,
            8,
            title.as_bytes(),
        );

        conn.flush();

        let mut visual = utils::find_visual(&conn, screen.root_visual()).unwrap();
        let cairo_xcb_conn = unsafe {
            cairo::XCBConnection::from_raw_none(conn.get_raw_conn() as *mut cairo_sys::xcb_connection_t)
        };
        let cairo_xcb_drawable = cairo::XCBDrawable(xcb_window_id);
        let raw_visualtype = &mut visual.base as *mut xcb::ffi::xcb_visualtype_t;
        let cairo_xcb_visual = unsafe {
            cairo::XCBVisualType::from_raw_none(raw_visualtype as *mut cairo_sys::xcb_visualtype_t)
        };
        let surface = <cairo::Surface as cairo::XCBSurface>::create(
            &cairo_xcb_conn,
            &cairo_xcb_drawable,
            &cairo_xcb_visual,
            width as i32,
            height as i32,
        );
        let cairo_context = cairo::Context::new(&surface);

        let render_window = RenderWindow {
            desktop_window,
            xcb_window_id,
            cairo_context,
            size: (width, height),
            text_extents,
        };

        render_windows.insert(hint, render_window);
    }

    // Receive keyboard events.
    let grab_keyboard_cookie = xcb::xproto::grab_keyboard(
        &conn,
        true,
        screen.root(),
        xcb::CURRENT_TIME,
        xcb::GRAB_MODE_ASYNC as u8,
        xcb::GRAB_MODE_ASYNC as u8,
    );
    println!("{:?}", grab_keyboard_cookie.get_reply().unwrap().status());

    // Receive mouse events.
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
    println!("{:?}", grab_pointer_cookie.get_reply().unwrap().status());

    let mut closed = false;
    while !closed {
        let event = conn.wait_for_event();
        match event {
            None => {
                break;
            }
            Some(event) => {
                let r = event.response_type();
                match r {
                    xcb::EXPOSE => {
                        for (hint, rw) in &render_windows {
                            let e = rw.text_extents;
                            rw.cairo_context.set_source_rgb(1.0, 1.0, 1.0);
                            rw.cairo_context.paint();
                            rw.cairo_context.select_font_face(&app_config.font_family, FontSlant::Normal, FontWeight::Normal);
                            rw.cairo_context.set_font_size(app_config.font_size);
                            rw.cairo_context.move_to(0.0 + e.x_bearing / 2.0, rw.size.1 as f64 + e.y_bearing / 2.0);
                            rw.cairo_context.set_source_rgb(0.0, 0.0, 0.0);
                            rw.cairo_context.show_text(&hint);
                            rw.cairo_context.get_target().flush();
                            conn.flush();
                        }
                    }
                    xcb::BUTTON_PRESS => {
                        break;
                    }
                    xcb::KEY_PRESS => {
                        let key_press: &xcb::KeyPressEvent = unsafe { xcb::cast_event(&event) };

                        let syms = xcb_util::keysyms::KeySymbols::new(&conn);
                        let ksym = syms.press_lookup_keysym(key_press, 0);
                        println!("ksym {}", ksym == xkb::KEY_q);

                        println!("Key '{}' pressed", key_press.detail());
                        if key_press.detail() == 0x18 {
                            // Q (on qwerty)
                            break;
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    //     events_loop.poll_events(|event| match event {
    //         glutin::Event::WindowEvent { event, .. } => match event {
    //             glutin::WindowEvent::Focused(false) => {
    //                 unfocused_events_seen += 1;
    //                 if render_windows.len() == unfocused_events_seen {
    //                     closed = true;
    //                 }
    //             }
    //             glutin::WindowEvent::CloseRequested => closed = true,
    //             glutin::WindowEvent::KeyboardInput {
    //                 input:
    //                     glutin::KeyboardInput {
    //                         virtual_keycode: Some(virtual_code),
    //                         state,
    //                         ..
    //                     },
    //                 ..
    //             } => match (virtual_code, state) {
    //                 (glutin::VirtualKeyCode::Escape, _) => closed = true,
    //                 _ => {
    //                     debug!("Received input: {:?}", virtual_code);
    //                     // So this is probably fairly hacky but what am I to do!
    //                     // I've got to match the enum by variant name and this is the only way I
    //                     // see.
    //                     let key_str = format!("{:?}", virtual_code);
    //                     if let Some(rw) = &render_windows.get(&key_str.to_lowercase()) {
    //                         wm::focus_window(&rw.desktop_window);
    //                     }
    //                     closed = true;
    //                 }
    //             },
    //             _ => (),
    //         },
    //         _ => {}
    //     });
    //
    //     for (hint, render_window) in &mut render_windows {
    //         unsafe {
    //             render_window
    //                 .glutin_window
    //                 .make_current()
    //                 .expect("Couldn't activate context");
    //         }
    //         render_window.encoder.clear(
    //             &render_window.rtv,
    //             [
    //                 app_config.bg_color.0,
    //                 app_config.bg_color.1,
    //                 app_config.bg_color.2,
    //                 app_config.bg_color.3,
    //             ],
    //         );
    //         render_window.encoder.clear_depth(&render_window.dsv, 1.0);
    //
    //         let (width, height, ..) = render_window.rtv.get_dimensions();
    //         let (width, height) = (f32::from(width), f32::from(height));
    //
    //         render_window.glyph_brush.queue(Section {
    //             screen_position: (width / 2.0, height / 2.0),
    //             text: hint,
    //             scale: gfx_glyph::Scale::uniform(app_config.font_size),
    //             color: [
    //                 app_config.text_color.0,
    //                 app_config.text_color.1,
    //                 app_config.text_color.2,
    //                 app_config.text_color.3,
    //             ],
    //             font_id: gfx_glyph::FontId(0),
    //             layout: gfx_glyph::Layout::default()
    //                 .h_align(gfx_glyph::HorizontalAlign::Center)
    //                 .v_align(gfx_glyph::VerticalAlign::Center),
    //             ..Section::default()
    //         });
    //
    //         render_window
    //             .glyph_brush
    //             .draw_queued(
    //                 &mut render_window.encoder,
    //                 &render_window.rtv,
    //                 &render_window.dsv,
    //             ).expect("Couldn't submit draw call");
    //
    //         render_window.encoder.flush(&mut render_window.device);
    //         render_window
    //             .glutin_window
    //             .swap_buffers()
    //             .expect("Failed to swap buffers");
    //         render_window.device.cleanup();
    //     }
    // }
}

#[cfg(not(any(feature = "i3", feature = "add_some_other_wm_here")))]
fn main() {
    eprintln!(
        "You need to enable to enabe support for at least one window manager.\n
Currently supported:
    --features i3"
    );
}
