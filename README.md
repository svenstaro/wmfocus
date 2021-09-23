# wmfocus - Visually focus windows by label

[![CI](https://github.com/svenstaro/wmfocus/workflows/CI/badge.svg)](https://github.com/svenstaro/wmfocus/actions)
[![Crates.io](https://img.shields.io/crates/v/wmfocus.svg)](https://crates.io/crates/wmfocus)
[![license](http://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/svenstaro/wmfocus/blob/master/LICENSE)
[![Stars](https://img.shields.io/github/stars/svenstaro/wmfocus.svg)](https://github.com/svenstaro/wmfocus/stargazers)
[![Lines of Code](https://tokei.rs/b1/github/svenstaro/wmfocus)](https://github.com/svenstaro/wmfocus)

This tool that allows you to rapidly choose a specific window directly without having to use the mouse or directional keyboard navigation.

![Screen cast](cast.apng)

Thanks to cairo, it should work on all kinds of screens and automatically display at the correct size according to your DPI.


## Installation

<a href="https://repology.org/project/wmfocus/versions"><img align="right" src="https://repology.org/badge/vertical-allrepos/wmfocus.svg" alt="Packaging status"></a>

**On Arch Linux**: `pacman -S wmfocus`

**With Cargo**: `cargo install --features i3 wmfocus`

## Usage

Draw labels on the upper-left corner of all windows:

    wmfocus

Completely fill out windows and draw the label in the middle (try it with transparency!):

    wmfocus --fill

Use a different font (as provided by fontconfig):

    wmfocus -f "Droid Sans":100

Change up the default colors:

    wmfocus --textcolor red --textcoloralt "#eeeeee" --bgcolor "rgba(50, 50, 200, 0.5)"

wmfocus will make use of a compositor to get real transparency.

## Full help
```
wmfocus 1.2.0
Sven-Hendrik Haase <svenstaro@gmail.com>


USAGE:
    wmfocus [FLAGS] [OPTIONS]

FLAGS:
        --fill         Completely fill out windows
    -h, --help         Prints help information
    -p, --printonly    Print the window id only but don't change focus
    -V, --version      Prints version information

OPTIONS:
        --textcolor <text_color>           Text color (CSS notation) [default: #dddddd]
        --textcoloralt <text_color_alt>    Text color alternate (CSS notation) [default: #666666]
        --bgcolor <bg_color>               Background color (CSS notation) [default: rgba(30, 30, 30, 0.9)]
        --halign <horizontal_align>        Horizontal alignment of the box inside the window [default: left]  [possible
                                           values: left, center, right]
        --valign <vertical_align>          Vertical alignment of the box inside the window [default: top]  [possible
                                           values: top, center, bottom]
    -e, --exit-keys <exit-keys>...         List of keys to exit application, sequence separator is space, key separator
                                           is '+', eg Control_L+g Shift_L+f
    -f, --font <font>                      Use a specific TrueType font with this format: family:size [default: Mono:72]
    -c, --chars <hint_chars>               Define a set of possbile values to use as hint characters [default:
                                           sadfjklewcmpgh]
    -m, --margin <margin>                  Add an additional margin around the text box (value is a factor of the box
                                           size) [default: 0.2]
    -o, --offset <offset>                  Offset box from edge of window (x,y) [default: 0,0]
```

## Troubleshooting

If there's some funky stuff, you can try to track it down by running `wmfocus` with `RUST_LOG=trace`:

    RUST_LOG=trace wmfocus

This will print quite some useful debugging info.


## Compiling

You need to have recent versions of `rust`, `cargo`, `xcb-util-keysyms`, `libxkbcommon-x11` and `cairo` installed.

Then, just clone it like usual and `cargo run` to get output:

    git clone https://github.com/svenstaro/wmfocus.git
    cd wmfocus
    cargo run --features i3


## Window manager support

While this tool is window manager-independent, an implementation for your favorite window manager might not yet be available. Current support:

- i3
- sway (partial, accepting PRs)

If you want to implement support for more window managers, have a look at the [i3 implementation](https://github.com/svenstaro/wmfocus/blob/master/src/wm_i3.rs).

This tool is heavily inspired by [i3-easyfocus](https://github.com/cornerman/i3-easyfocus).


## Releasing

This is mostly a note for me on how to release this thing:

- `cargo release --dry-run`
- `cargo release`
- Release will automatically be deployed by Github Actions.
- Update Arch package.
