use std::error::Error;
use std::iter;
use std::str::FromStr;

use clap::{App, Arg};
use css_color_parser::Color as CssColor;
use font_loader::system_fonts;
use itertools::Itertools;

use AppConfig;

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

/// Parse app arguments.
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
                .default_value("Mono:72")
                .help("Use a specific TrueType font with this format: family:size"))
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
                .display_order(50)
                .help("Text color (CSS notation)"))
        .arg(
            Arg::with_name("bg_color")
                .long("bgcolor")
                .takes_value(true)
                .validator(is_valid_color)
                .default_value("rgba(30, 30, 30, 0.8)")
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
        .get_matches();

    let font = value_t!(matches, "font", String).unwrap();
    let v: Vec<_> = font.split(':').collect();
    let (font_family, font_size) = (v[0].to_string(), v[1].parse::<f32>().unwrap());
    let margin = value_t!(matches, "margin", f32).unwrap();
    let text_color_unparsed = value_t!(matches, "text_color", CssColor).unwrap();
    let text_color = (
        text_color_unparsed.r as f32 / 255.0,
        text_color_unparsed.g as f32 / 255.0,
        text_color_unparsed.b as f32 / 255.0,
        text_color_unparsed.a,
    );
    let bg_color_unparsed = value_t!(matches, "bg_color", CssColor).unwrap();
    let bg_color = (
        bg_color_unparsed.r as f32 / 255.0,
        bg_color_unparsed.g as f32 / 255.0,
        bg_color_unparsed.b as f32 / 255.0,
        bg_color_unparsed.a,
    );
    let horizontal_align = value_t!(matches, "horizontal_align", HorizontalAlign).unwrap();
    let vertical_align = value_t!(matches, "vertical_align", VerticalAlign).unwrap();
    let fill = value_t!(matches, "fill", bool).unwrap_or(false);

    let loaded_font = load_font(&font_family);

    println!("{:?} x {:?}", text_color, bg_color);

    AppConfig {
        font_family,
        font_size,
        loaded_font,
        margin,
        text_color,
        bg_color,
        horizontal_align,
        vertical_align,
        fill,
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
    println!("generated {}", ret);
    ret
}
