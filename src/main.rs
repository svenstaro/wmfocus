#[macro_use]
extern crate clap;
extern crate font_loader;
extern crate fps_clock;
extern crate itertools;
extern crate rusttype;
extern crate glutin;

use std::collections::HashMap;
use glutin::dpi::{PhysicalPosition, LogicalPosition, LogicalSize};
use glutin::os::unix::{XWindowType, WindowBuilderExt};

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
}

pub struct RenderWindow<'a> {
    desktop_window: &'a DesktopWindow,
    glutin_window: glutin::Window,
    // text_system: glium_text::TextSystem,
    // font: glium_text::FontTexture,
    // display: glium::Display,
}

#[derive(Debug)]
pub struct AppConfig {
    pub font_family: String,
    pub font_size: u32,
    pub loaded_font: Vec<u8>,
}

static HINT_CHARS: &'static str = "sadfjklewcmpgh";

#[cfg(any(feature = "i3", feature = "add_some_other_wm_here"))]
fn main() {
    let app_config = utils::parse_args();

    // Get the windows from each specific window manager implementation.
    let desktop_windows = wm::get_windows();

    // Limit FPS to preserve performance
    let mut fps = fps_clock::FpsClock::new(30);

    // let font = Font::from_bytes(&loaded_font).expect("Couldn't load font");

    let mut events_loop = glutin::EventsLoop::new();
    let mut render_windows = HashMap::new();
    for desktop_window in &desktop_windows {
        // We need to estimate the font size before rendering because we want the window to only be
        // the size of the font.
        let hint = utils::get_next_hint(render_windows.keys().collect(), HINT_CHARS, desktop_windows.len());
        // let hint_text = font.layout(
        //     &hint,
        //     rusttype::Scale::uniform(app_config.font_size as f32),
        //     rusttype::Point { x: 0.0, y: 0.0 },
        // );
        // let (width, height) = hint_text.fold((0, 0), |acc, current| {
        //     (
        //         max(acc.0, current.pixel_bounding_box().unwrap().max.x),
        //         max(acc.1, current.pixel_bounding_box().unwrap().height()),
        //     )
        // });

        println!("{:?}", desktop_window);
        let glutin_window = glutin::WindowBuilder::new()
            // .with_decorations(false)
            // .with_always_on_top(true)
            // .with_x11_window_type(XWindowType::Splash)
            .with_override_redirect(true)
            // .with_transparency(true)
            .with_dimensions((50, 50).into())
            .build(&events_loop)
            .unwrap();
        let dpi = glutin_window.get_hidpi_factor();
        glutin_window.set_position(PhysicalPosition::from(desktop_window.pos).to_logical(dpi));
        println!("{:?}", glutin_window.get_position());

        // let context = glutin::ContextBuilder::new();
        // let display = glium::Display::new(gwindow, context, &events_loop).unwrap();
        // let dpi = display.gl_window().get_hidpi_factor();
        // display.gl_window().set_position(window.pos.to_logical(dpi));

        // let text_system = glium_text::TextSystem::new(&display);
        // let font = glium_text::FontTexture::new(
        //     &display,
        //     loaded_font.as_slice(),
        //     app_config.font_size,
        //     HINT_CHARS.chars(),
        // ).expect("Error loading font");
        //
        // let dpi = display.gl_window().get_hidpi_factor();
        let render_window = RenderWindow {
            desktop_window,
            glutin_window,
            // text_system,
            // font,
            // display,
        };

        render_windows.insert(hint.clone(), render_window);
        // let rw = &render_windows[&hint];
        // let text = glium_text::TextDisplay::new(&rw.text_system, &rw.font, &hint);
        // let (text_width, text_height) = (text.get_width(), text.get_height());
        // let ratio = (text_height / text_width) as f64;
        //
        // let window_width = 50.0f64;
        // let window_height = window_width * ratio;
        // let window_size = PhysicalSize::new(window_width, window_height);

        // println!(
        //     "text_width {} text_height {}, ratio {}, window_width {} window_height {}",
        //     text_width, text_height, ratio, window_width, window_height
        //     );

        // rw.display
        //     .gl_window()
        //     .set_inner_size(window_size.to_logical(dpi));
    }

    // We have to ignore the first few events because whenever a new window is created, the old one
    // is unfocused. However, we don't want close all the windows after the second one is
    // created. Therefore, we have to count how many events we've already seen so that we can
    // ignore the first few.
    let mut unfocused_events_seen = 0;

    let mut closed = false;
    while !closed {
        // for (hint, rw) in &render_windows {
            // let (w, h) = rw.display.get_framebuffer_dimensions();
            // let text = glium_text::TextDisplay::new(&rw.text_system, &rw.font, &hint);
            // let text_width = text.get_width();
            // println!("{} {}", w, h);

            // #[cfg_attr(rustfmt, rustfmt_skip)]
            // let matrix:[[f32; 4]; 4] = cgmath::Matrix4::new(
            //     2.0 / text_width, 0.0, 0.0, 0.0,
            //     0.0, 2.0 * (w as f32) / (h as f32) / text_width, 0.0, 0.0,
            //     0.0, 0.0, 1.0, 0.0,
            //     -1.0, -1.0, 0.0, 1.0f32,
            //     ).into();

            // let mut target = rw.display.draw();
            // target.clear_color(0.0, 0.0, 1.0, 1.0);
            // glium_text::draw(
            //     &text,
            //     &rw.text_system,
            //     &mut target,
            //     matrix,
            //     (1.0, 1.0, 0.0, 1.0),
            // ).unwrap();
            // target.finish().unwrap();
        // }
        events_loop.poll_events(|event| match event {
            glutin::Event::WindowEvent { event, .. } => match event {
                glutin::WindowEvent::Focused(false) => {
                    unfocused_events_seen += 1;
                    if render_windows.len() == unfocused_events_seen {
                        closed = true;
                    }
                },
                glutin::WindowEvent::CloseRequested => closed = true,
                glutin::WindowEvent::KeyboardInput {
                    input:
                        glutin::KeyboardInput {
                            virtual_keycode: Some(virtual_code),
                            state,
                            ..
                        },
                    ..
                } => match (virtual_code, state) {
                    (glutin::VirtualKeyCode::Escape, _) => closed = true,
                    _ => {
                        println!("{:?}", virtual_code);

                        // So this is probably fairly hacky but what am I to do!
                        // I've got to match the enum by variant name and this is the only way I
                        // see.
                        let key_str = format!("{:?}", virtual_code);
                        // if let Some(rw) = &render_windows.get(&key_str.to_lowercase()) {
                        //     wm::focus_window(&rw.window);
                        // }
                        closed = true;
                    }
                },
                _ => (),
            },
            _ => {}
        });
        // fps.tick();
    }
}

#[cfg(not(any(feature = "i3", feature = "add_some_other_wm_here")))]
fn main() {
    eprintln!(
        "You need to enable to enabe support for at least one window manager.\n
Currently supported:
    --features i3"
    );
}
