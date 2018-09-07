#[macro_use]
extern crate clap;
extern crate font_loader;
extern crate fps_clock;
extern crate gfx;
extern crate gfx_device_gl;
extern crate gfx_glyph;
extern crate gfx_window_glutin;
extern crate glutin;
extern crate itertools;
extern crate rusttype;

use gfx::handle::{DepthStencilView, RenderTargetView};
use gfx::{format, Device};
use gfx_glyph::GlyphBrushBuilder;
use glutin::dpi::{PhysicalPosition};
use glutin::os::unix::WindowBuilderExt;
use glutin::GlContext;
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
}

pub struct RenderWindow<'a, R: gfx::Resources, C: gfx::CommandBuffer<R>, F: gfx::Factory<R>, T, G> {
    desktop_window: &'a DesktopWindow,
    glutin_window: glutin::GlWindow,
    device: gfx_device_gl::Device,
    encoder: gfx::Encoder<R, C>,
    rtv: RenderTargetView<R, T>,
    dsv: DepthStencilView<R, G>,
    glyph_brush: gfx_glyph::GlyphBrush<'a, R, F>,
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
        let hint = utils::get_next_hint(
            render_windows.keys().collect(),
            HINT_CHARS,
            desktop_windows.len(),
        );
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
        let window_builder = glutin::WindowBuilder::new()
            // .with_decorations(false)
            // .with_always_on_top(true)
            // .with_x11_window_type(XWindowType::Splash)
            .with_override_redirect(true)
            // .with_transparency(true)
            .with_dimensions((500, 500).into());

        let context = glutin::ContextBuilder::new();
        let (glutin_window, mut device, mut factory, mut rtv, mut dsv) =
            gfx_window_glutin::init::<format::Srgba8, format::Depth>(
                window_builder,
                context,
                &events_loop,
            );

        let mut glyph_brush = GlyphBrushBuilder::using_font_bytes(&app_config.loaded_font)
            .initial_cache_size((512, 512))
            // Enable depth testing with less-equal drawing and update the depth buffer
            .depth_test(gfx::preset::depth::LESS_EQUAL_WRITE)
            .build(factory.clone());

        let mut encoder = factory.create_command_buffer().into();

        let dpi = glutin_window.get_hidpi_factor();
        glutin_window.set_position(PhysicalPosition::from(desktop_window.pos).to_logical(dpi));
        println!("{:?}", glutin_window.get_position());

        let render_window = RenderWindow {
            desktop_window,
            glutin_window,
            device,
            encoder,
            rtv,
            dsv,
            glyph_brush,
            // text_system,
            // font,
            // display,
        };

        render_windows.insert(hint.clone(), render_window);
    }

    // We have to ignore the first few events because whenever a new window is created, the old one
    // is unfocused. However, we don't want close all the windows after the second one is
    // created. Therefore, we have to count how many events we've already seen so that we can
    // ignore the first few.
    let mut unfocused_events_seen = 0;

    let mut closed = false;
    while !closed {
        events_loop.poll_events(|event| match event {
            glutin::Event::WindowEvent { event, .. } => match event {
                glutin::WindowEvent::Focused(false) => {
                    unfocused_events_seen += 1;
                    if render_windows.len() == unfocused_events_seen {
                        closed = true;
                    }
                }
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
                        if let Some(rw) = &render_windows.get(&key_str.to_lowercase()) {
                            wm::focus_window(&rw.desktop_window);
                        }
                        closed = true;
                    }
                },
                _ => (),
            },
            _ => {}
        });

        for (hint, render_window) in &mut render_windows {
            render_window
                .encoder
                .clear(&render_window.rtv, [0.02, 0.02, 0.02, 1.0]);
            render_window.encoder.clear_depth(&render_window.dsv, 1.0);

            let (width, height, ..) = render_window.rtv.get_dimensions();
            let (width, height) = (f32::from(width), f32::from(height));

            render_window.glyph_brush.queue(gfx_glyph::Section {
                screen_position: (width / 2.0, 100.0),
                bounds: (width, height - 100.0),
                text: "On top",
                scale: gfx_glyph::Scale::uniform(95.0),
                color: [0.8, 0.8, 0.8, 1.0],
                font_id: gfx_glyph::FontId(0),
                layout: gfx_glyph::Layout::default().h_align(gfx_glyph::HorizontalAlign::Center),
                z: 0.2,
            });

            render_window
                .glyph_brush
                .draw_queued(
                    &mut render_window.encoder,
                    &render_window.rtv,
                    &render_window.dsv,
                ).expect("Couldn't submit draw call");

            render_window.encoder.flush(&mut render_window.device);
            render_window
                .glutin_window
                .swap_buffers()
                .expect("Failed to swap buffers");
            render_window.device.cleanup();
        }
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
