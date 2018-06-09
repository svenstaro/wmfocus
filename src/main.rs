#[macro_use]
extern crate clap;
extern crate glium;

use glium::{glutin, Surface};

use clap::{App, Arg};
use glutin::os::unix::{WindowBuilderExt, XWindowType};
use glutin::{Event, KeyboardInput, VirtualKeyCode, WindowEvent};
use std::collections::HashMap;

pub struct AppConfig {
    pub font: String,
}

#[cfg(feature = "i3")]
extern crate i3ipc;

#[cfg(feature = "i3")]
mod wm_i3;

#[cfg(feature = "i3")]
use wm_i3 as wm;

fn is_xft_font(v: String) -> Result<(), String> {
    let _ = v;
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
                .validator(is_xft_font)
                .help("Use this XFT font"),
        )
        .get_matches();

    let font = if matches.is_present("font") {
        value_t!(matches, "font", String).unwrap()
    } else {
        "hans".to_string()
    };

    AppConfig { font }
}

#[cfg(any(feature = "i3", feature = "add_some_other_wm_here"))]
fn main() {
    parse_args();
    wm::thing();

    let mut events_loop = glutin::EventsLoop::new();

    let mut displays = HashMap::new();
    for i in 0..3 {
        let window = glutin::WindowBuilder::new()
            .with_decorations(false)
            .with_always_on_top(true)
            .with_x11_window_type(XWindowType::Dialog)
            .with_transparency(true)
            .with_dimensions(50, 50);
        let context = glutin::ContextBuilder::new();
        let display = glium::Display::new(window, context, &events_loop).unwrap();
        display.gl_window().set_position(20 + 300 * i, 20 + 30);
        displays.insert(i, display);
    }

    let mut closed = false;
    while !closed {
		for (i, display) in &displays {
			let mut target = display.draw();
			target.clear_color(0.0 + 3.0 / (i + 1) as f32, 0.0, 1.0, 0.5);
			target.finish().unwrap();
		}

        events_loop.poll_events(|event| {
            println!("{:?}", event);

            match event {
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
            }
        });
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
