#[macro_use]
extern crate clap;

extern crate winit;

use clap::{App, Arg};
use std::collections::HashMap;
use winit::{ControlFlow, Event, WindowEvent};

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

    let mut events_loop = winit::EventsLoop::new();

    let mut windows = HashMap::new();
    for i in 0..3 {
        let window = winit::WindowBuilder::new()
            .with_decorations(false)
            .with_always_on_top(true)
            .with_transparency(true)
            .with_min_dimensions(30, 30)
            .with_max_dimensions(30, 30)
            .with_always_on_top(true)
            .build(&events_loop)
            .unwrap();
        window.set_position(20 + 300 * i, 20 + 30);
        windows.insert(window.id(), window);
    }

    events_loop.run_forever(|event| {
        println!("{:?}", event);

        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => return ControlFlow::Break,
                WindowEvent::KeyboardInput {
                    input:
                        winit::KeyboardInput {
                            virtual_keycode: Some(virtual_code),
                            state,
                            ..
                        },
                    ..
                } => match (virtual_code, state) {
                    (winit::VirtualKeyCode::Escape, _) => return ControlFlow::Break,
                    _ => {
                        println!("{:?}", virtual_code);
                        return ControlFlow::Break;
                    }
                },
                _ => (),
            },
            _ => {}
        }

        ControlFlow::Continue
    });
}

#[cfg(not(any(feature = "i3", feature = "add_some_other_wm_here")))]
fn main() {
    eprintln!(
        "You need to enable to enabe at least one WM feature.\n
Currently supported:
    --features i3"
    );
}
