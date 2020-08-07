use css_color_parser::Color as CssColor;
use font_loader::system_fonts;
use log::{info, warn};
use structopt::clap::arg_enum;
use structopt::StructOpt;

arg_enum! {
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub enum HorizontalAlign {
        Left,
        Center,
        Right,
    }
}

arg_enum! {
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub enum VerticalAlign {
        Top,
        Center,
        Bottom,
}
}

/// Load a system font.
fn load_font(font_family: &str) -> Vec<u8> {
    let mut font_family_property = system_fonts::FontPropertyBuilder::new()
        .family(font_family)
        .build();
    let info = system_fonts::query_specific(&mut font_family_property);
    info!("Returned effective font is: {:?}", info);
    let (loaded_font, _) =
        if let Some((loaded_font, index)) = system_fonts::get(&font_family_property) {
            (loaded_font, index)
        } else {
            warn!("Family not found, falling back to first Monospace font");
            let mut font_monospace_property =
                system_fonts::FontPropertyBuilder::new().monospace().build();
            let sysfonts = system_fonts::query_specific(&mut font_monospace_property);
            warn!("Falling back to font '{font}'", font = sysfonts[0]);
            let (loaded_font, index) =
                system_fonts::get(&font_monospace_property).expect("Couldn't find suitable font");
            (loaded_font, index)
        };
    loaded_font
}

/// Generate a valid `FontConfig` from `f`.
/// `f` is expected to be in format `Mono:72`.
fn parse_truetype_font(f: &str) -> Result<FontConfig, String> {
    let v: Vec<_> = f.split(':').collect();
    let (family, size) = (
        v.get(0).ok_or("Wrong font format")?,
        v.get(1).ok_or("Wrong font format")?,
    );

    let loaded_font = load_font(family);
    let font_config = FontConfig {
        font_family: family.to_string(),
        font_size: size
            .parse::<f64>()
            .map_err(|_| "Couldn't parse font size".to_string())?,
        loaded_font,
    };
    Ok(font_config)
}

/// Validate coordinates and parse offset.
fn parse_offset(c: &str) -> Result<Offset, String> {
    let v: Vec<_> = c.split(',').collect();
    let (x, y) = (
        v.get(0)
            .ok_or("Wrong coordinate format, expected x,y coordinates")?,
        v.get(1)
            .ok_or("Wrong coordinate format, expected x,y coordinates")?,
    );
    let offset = Offset {
        x: x.parse::<i32>()
            .map_err(|_| "Couldn't parse x coordinate")?,
        y: y.parse::<i32>()
            .map_err(|_| "Couldn't parse y coordinate")?,
    };
    Ok(offset)
}

/// Parse a color into a tuple of floats.
fn parse_color(color_str: &str) -> Result<(f64, f64, f64, f64), String> {
    let color = color_str
        .parse::<CssColor>()
        .map_err(|_| "Invalid color format")?;
    Ok((
        f64::from(color.r) / 255.0,
        f64::from(color.g) / 255.0,
        f64::from(color.b) / 255.0,
        f64::from(color.a),
    ))
}

#[derive(Debug)]
pub struct Offset {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug)]
pub struct FontConfig {
    pub font_family: String,
    pub font_size: f64,
    pub loaded_font: Vec<u8>,
}

#[derive(StructOpt, Debug)]
#[structopt(
    name = "wmfocus",
    author,
    about,
    global_settings = &[structopt::clap::AppSettings::ColoredHelp],
)]
pub struct AppConfig {
    /// Use a specific TrueType font with this format: family:size
    #[structopt(short, long, default_value = "Mono:72", parse(try_from_str = parse_truetype_font))]
    pub font: FontConfig,

    /// Define a set of possbile values to use as hint characters
    #[structopt(short = "c", long = "chars", default_value = "sadfjklewcmpgh")]
    pub hint_chars: String,

    /// Add an additional margin around the text box (value is a factor of the box size)
    #[structopt(short, long, default_value = "0.2")]
    pub margin: f32,

    /// Text color (CSS notation)
    #[structopt(long = "textcolor", display_order = 49, default_value = "#dddddd", parse(try_from_str = parse_color))]
    pub text_color: (f64, f64, f64, f64),

    /// Text color alternate (CSS notation)
    #[structopt(long = "textcoloralt", display_order = 50, default_value = "#666666", parse(try_from_str = parse_color))]
    pub text_color_alt: (f64, f64, f64, f64),

    /// Background color (CSS notation)
    #[structopt(long = "bgcolor", display_order = 51, default_value = "rgba(30, 30, 30, 0.9)", parse(try_from_str = parse_color))]
    pub bg_color: (f64, f64, f64, f64),

    /// Horizontal alignment of the box inside the window
    #[structopt(long = "halign", display_order = 100, default_value = "left", possible_values = &HorizontalAlign::variants(), case_insensitive = true)]
    pub horizontal_align: HorizontalAlign,

    /// Vertical alignment of the box inside the window
    #[structopt(long = "valign", display_order = 101, default_value = "top", possible_values = &VerticalAlign::variants(), case_insensitive = true)]
    pub vertical_align: VerticalAlign,

    /// Completely fill out windows
    #[structopt(long, display_order = 102, conflicts_with_all(&["horizontal_align", "vertical_align", "margin", "offset"]))]
    pub fill: bool,

    /// Print the window id only but don't change focus
    #[structopt(short, long)]
    pub print_only: bool,

    /// Offset box from edge of window relative to alignment (x,y)
    #[structopt(short, long, allow_hyphen_values = true, default_value = "0,0", parse(try_from_str = parse_offset))]
    pub offset: Offset,
}

pub fn parse_args() -> AppConfig {
    let mut config = AppConfig::from_args();
    if config.fill {
        config.horizontal_align = HorizontalAlign::Center;
        config.vertical_align = VerticalAlign::Center;
    }
    config
}
