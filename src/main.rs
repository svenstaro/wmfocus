#[macro_use]
extern crate clap;
extern crate font_loader;
extern crate fps_clock;
extern crate glium;
extern crate glium_text_rusttype as glium_text;

use font_loader::system_fonts;
use glium::{glutin, Surface, Display};
use std::error::Error;

use clap::{App, Arg};
use glutin::os::unix::{WindowBuilderExt, XWindowType};
use glutin::{Event, KeyboardInput, VirtualKeyCode, WindowEvent};
use std::collections::HashMap;

#[derive(Debug)]
pub struct Window {
    id: i64,
    pos: (i32, i32),
}

#[derive(Debug)]
pub struct AppConfig {
    pub font_family: String,
    pub font_size: u8,
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
    if let Err(e) = size.unwrap().parse::<u8>() {
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
                .default_value("DejaVu Sans Mono:14")
                .help("Use a specific TrueType font with this format: family:size"),
        )
        .get_matches();

    let font = value_t!(matches, "font", String).unwrap();
    let v: Vec<_> = font.split(":").collect();
    let (font_family, font_size) = (
        v.get(0).unwrap().to_string(),
        v.get(1).unwrap().parse::<u8>().unwrap(),
    );
    AppConfig {
        font_family,
        font_size,
    }
}

#[cfg(any(feature = "i3", feature = "add_some_other_wm_here"))]
fn main() {
    let app_config = parse_args();
    let font_family_property = system_fonts::FontPropertyBuilder::new()
        .family(&app_config.font_family)
        .build();
    let mut loaded_font = system_fonts::get(&font_family_property);
    if loaded_font.is_none() {
        eprintln!("Family not found, falling back to first Monospace font");
        let mut font_monospace_property = system_fonts::FontPropertyBuilder::new().monospace().build();
        let sysfonts = system_fonts::query_specific(&mut font_monospace_property);
        loaded_font = system_fonts::get(&font_monospace_property);
        eprintln!("Falling back to font '{font}'", font=sysfonts[0]);
    }

    let windows = wm::get_windows();

    // Limit FPS to preserve performance
    let mut fps = fps_clock::FpsClock::new(30);

    let mut events_loop = glutin::EventsLoop::new();
    let mut displays = HashMap::new();
    for window in windows {
        let gwindow = glutin::WindowBuilder::new()
            .with_decorations(false)
            .with_always_on_top(true)
            .with_x11_window_type(XWindowType::Dialog)
            .with_transparency(true)
            .with_dimensions(50, 50);
        let context = glutin::ContextBuilder::new();
        let display = glium::Display::new(gwindow, context, &events_loop).unwrap();
        let text_system = glium_text::TextSystem::new(&display);
        // let font = glium_text::FontTexture::new(
        //     &display,
        //     File::open("font.ttf").unwrap(),
        //     32,
        //     glium_text::FontTexture::ascii_character_list(),
        // ).unwrap();
        display.gl_window().set_position(window.pos.0, window.pos.1);
        displays.insert(window.id, display);
    }

    let mut closed = false;
    for (i, display) in &displays {
        let mut target = display.draw();
        target.clear_color(0.0 + 3.0 / (i + 1) as f32, 0.0, 1.0, 1.0);
        target.finish().unwrap();
    }

    while !closed {
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
