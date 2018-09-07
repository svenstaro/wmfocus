use std::cmp::max;
use std::collections::HashMap;
use std::error::Error;

use clap::{App, Arg};
use itertools::Itertools;
use font_loader::system_fonts;
use std::iter;

use rusttype::Font;

use AppConfig;

/// Checks whether the provided fontconfig font `f` is valid.
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

/// Load a system font.
fn load_font<'a>(font_family: &str) -> Vec<u8> {
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
                .default_value("DejaVu Sans Mono:72")
                .help("Use a specific TrueType font with this format: family:size"),
        ).get_matches();

    let font = value_t!(matches, "font", String).unwrap();
    let v: Vec<_> = font.split(":").collect();
    let (font_family, font_size) = (
        v.get(0).unwrap().to_string(),
        v.get(1).unwrap().parse::<u32>().unwrap(),
    );

    let loaded_font = load_font(&font_family);

    AppConfig {
        font_family,
        font_size,
        loaded_font,
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
