#[macro_use]
extern crate clap;
extern crate cgmath;
extern crate font_loader;
extern crate fps_clock;
extern crate glium;
extern crate glium_text_rusttype as glium_text;
extern crate itertools;

use font_loader::system_fonts;
use glium::{glutin, Surface};
use std::collections::HashMap;
use std::error::Error;

use clap::{App, Arg};
use itertools::Itertools;
use glutin::os::unix::{WindowBuilderExt, XWindowType};
use glutin::{Event, KeyboardInput, VirtualKeyCode, WindowEvent};

#[derive(Debug)]
pub struct Window {
    id: i64,
    pos: (i32, i32),
}

pub struct RenderWindow {
    text_system: glium_text::TextSystem,
    font: glium_text::FontTexture,
    display: glium::Display,
}

#[derive(Debug)]
pub struct AppConfig {
    pub font_family: String,
    pub font_size: u32,
}

static HINT_CHARS: &'static str = "sadfjklewcmpgh";

#[cfg(feature = "i3")]
extern crate i3ipc;

#[cfg(feature = "i3")]
mod wm_i3;

#[cfg(feature = "i3")]
use wm_i3 as wm;

fn is_truetype_font(f: String) -> Result<(), String> {
    let v: Vec<_> = f.split(":").collect();
    let (family, size) = (v.get(0), v.get(1));
    if family.is_none() || size.is_none() {
        return Err("From font format".to_string());
    }
    if let Err(e) = size.unwrap().parse::<u32>() {
        return Err(e.description().to_string());
    }
    Ok(())
}

pub fn parse_args() -> AppConfig {
    let matches = App::new(crate_name!())
        .version(crate_version!())
        .author(crate_authors!())
        .about(crate_description!())
        .arg(
            Arg::with_name("font")
            .short("f")
            .long("font")
            .takes_value(true)
            .validator(is_truetype_font)
            .default_value("DejaVu Sans Mono:72")
            .help("Use a specific TrueType font with this format: family:size"),
            )
        .get_matches();

    let font = value_t!(matches, "font", String).unwrap();
    let v: Vec<_> = font.split(":").collect();
    let (font_family, font_size) = (
        v.get(0).unwrap().to_string(),
        v.get(1).unwrap().parse::<u32>().unwrap(),
        );
    AppConfig {
        font_family,
        font_size,
    }
}

/// Given a list of `current_hints` and a bunch of `hint_chars`, this finds a unique combination
/// of characters that doesn't yet exist in `current_hints`. `max_count` is the maximum possible
/// number of hints we need.
fn get_next_hint(current_hints: Vec<&String>, hint_chars: &str, max_count: usize) -> String {
    // Figure out which size we need.
    let mut size_required = 1;
    while hint_chars.len().pow(size_required) < max_count {
        size_required += 1;
    }
    let mut ret = hint_chars.chars().next().expect("No hint_chars found").to_string();
    let it = std::iter::repeat(hint_chars.chars().rev()).take(size_required as usize).multi_cartesian_product();
    for c in it {
        let folded = c.into_iter().collect();
        if !current_hints.contains(&&folded) {
            ret = folded;
        }
    }
    println!("generated {}", ret);
    ret
}

#[cfg(any(feature = "i3", feature = "add_some_other_wm_here"))]
fn main() {
    let app_config = parse_args();
    let font_family_property = system_fonts::FontPropertyBuilder::new()
        .family(&app_config.font_family)
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

    let windows = wm::get_windows();

    // Limit FPS to preserve performance
    let mut fps = fps_clock::FpsClock::new(30);

    let mut events_loop = glutin::EventsLoop::new();
    let mut render_windows = HashMap::new();
    for window in &windows {
        let gwindow = glutin::WindowBuilder::new()
            .with_decorations(false)
            .with_always_on_top(true)
            .with_x11_window_type(XWindowType::Dialog)
            .with_transparency(true);
        let context = glutin::ContextBuilder::new();
        let display = glium::Display::new(gwindow, context, &events_loop).unwrap();
        display.gl_window().set_position(window.pos.0, window.pos.1);

        let text_system = glium_text::TextSystem::new(&display);
        let font = glium_text::FontTexture::new(
            &display,
            loaded_font.as_slice(),
            app_config.font_size,
            HINT_CHARS.chars(),
            ).expect("Error loading font");
        let render_window = RenderWindow {
            text_system,
            font,
            display,
        };
        let hint = get_next_hint(render_windows.keys().collect(), HINT_CHARS, windows.len());
        render_windows.insert(hint.clone(), render_window);
        let rw = &render_windows[&hint];
        let text = glium_text::TextDisplay::new(&rw.text_system, &rw.font, &hint);
        let (text_width, text_height) = (text.get_width(), text.get_height());
        let ratio = text_height / text_width;
        let window_width = 320;
        let window_height = (window_width as f32 * ratio) as u32;
        // println!(
        //     "text_width {} text_height {}, ratio {}, window_width {} window_height {}",
        //     text_width, text_height, ratio, window_width, window_height
        //     );
        rw.display
            .gl_window()
            .set_inner_size(window_width, window_height);
    }

    let mut closed = false;
    while !closed {
        for (hint, rw) in &render_windows {
            let (w, h) = rw.display.get_framebuffer_dimensions();
            let text = glium_text::TextDisplay::new(&rw.text_system, &rw.font, &hint);
            let text_width = text.get_width();
            println!("{} {}", w, h);

            #[cfg_attr(rustfmt, rustfmt_skip)]
            let matrix:[[f32; 4]; 4] = cgmath::Matrix4::new(
                2.0 / text_width, 0.0, 0.0, 0.0,
                0.0, 2.0 * (w as f32) / (h as f32) / text_width, 0.0, 0.0,
                0.0, 0.0, 1.0, 0.0,
                -1.0, -1.0, 0.0, 1.0f32,
                ).into();

            let mut target = rw.display.draw();
            target.clear_color(0.0, 0.0, 1.0, 1.0);
            glium_text::draw(
                &text,
                &rw.text_system,
                &mut target,
                matrix,
                (1.0, 1.0, 0.0, 1.0),
                ).unwrap();
            target.finish().unwrap();
        }
        events_loop.poll_events(|event| match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => closed = true,
                WindowEvent::KeyboardInput {
                    input:
                        KeyboardInput {
                            virtual_keycode: Some(virtual_code),
                            state,
                            ..
                        },
                        ..
                } => match (virtual_code, state) {
                    (VirtualKeyCode::Escape, _) => closed = true,
                    _ => {
                        println!("{:?}", virtual_code);
                        closed = true;
                    }
                },
                        _ => (),
            },
            _ => {}
        });
        fps.tick();
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
