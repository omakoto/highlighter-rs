use std::rc::Rc;
use std::env;
use std::cmp::max;

use super::*;

const FG_BG_PLACE_HOLDER: char = '#';

#[derive(Debug,Copy,Clone,PartialEq,Eq)]
pub enum Color {
    /// Color not specified; take over previous color.
    None,
    /// Index color, 0-7
    Console(u8),
    /// 256 * 256 * 256 full color.
    Rgb(u8, u8, u8),
}

impl Color {
    pub fn with_index(i: u8) -> Color {
        if i > 7 {
            panic!("Invalid color index {}", i);
        }
        return Color::Console(i);
    }

    pub fn with_xterm_color(rgb: u16) -> Color {
        return Color::Rgb(Color::color_6_to_256((rgb / 100) as u8),
                          Color::color_6_to_256(((rgb / 10) % 10) as u8),
                          Color::color_6_to_256((rgb % 10) as u8));
    }

    pub fn with_rgb(r: u8, g: u8, b: u8) -> Color {
        return Color::Rgb(r, g, b);
    }

    pub fn color_6_to_256(color6: u8) -> u8 {
        return ((color6 as i32) * 255 / 5) as u8;
    }

    pub fn color_256_to_6(color256: u8) -> u8 {
        return ((color256 as i32) * 5 / 255) as u8;
    }

    #[test]
    fn test_color_6_256() {
        assert_eq!(0, Color::color_6_to_256(0));
        assert_eq!(255, Color::color_6_to_256(5));
        assert_eq!(0, Color::color_256_to_6(Color::color_6_to_256(0)));
        assert_eq!(1, Color::color_256_to_6(Color::color_6_to_256(1)));
        assert_eq!(4, Color::color_256_to_6(Color::color_6_to_256(4)));
        assert_eq!(5, Color::color_256_to_6(Color::color_6_to_256(5)));
    }
}

bitflags! {
    pub flags Attribute: u32 {
        const ATTR_NONE      = 0,
        const ATTR_INTENSE   = 1 << 0,
        const ATTR_ITALIC    = 1 << 1,
        const ATTR_UNDERLINE = 1 << 2,
        const ATTR_STRIKE    = 1 << 3,
        const ATTR_FAINT     = 1 << 4,
    }
}

impl Attribute {
    fn to_ansi_code(&self, term: Term) -> String {
        if *self == ATTR_NONE || term == Term::Dumb {
            return String::new();
        }
        let mut ret = term.csi_start().to_string();
        let mut first = true;

        {
            let mut add = |attr: Attribute, ch: char| {
                if (*self & attr) != ATTR_NONE {
                    if first {
                        first = false;
                    } else {
                        ret.push(';');
                    }
                    ret.push(ch);
                }
            };
            add(ATTR_INTENSE, '1');
            add(ATTR_ITALIC, '3');
            add(ATTR_UNDERLINE, '4');
            add(ATTR_STRIKE, '9');
            add(ATTR_FAINT, '2');
        }
        ret.push_str(term.csi_end());
        ret
    }
}

#[test]
fn test_attr_to_ansi_code() {
    assert_eq!("", ATTR_NONE.to_ansi_code(Term::Xterm));
    assert_eq!("\x1b[1m".to_string(), ATTR_INTENSE.to_ansi_code(Term::Xterm));
    assert_eq!("\x1b[3m".to_string(), ATTR_ITALIC.to_ansi_code(Term::Xterm));
    assert_eq!("\x1b[4m".to_string(), ATTR_UNDERLINE.to_ansi_code(Term::Xterm));
    assert_eq!("\x1b[9m".to_string(), ATTR_STRIKE.to_ansi_code(Term::Xterm));
    assert_eq!("\x1b[2m".to_string(), ATTR_FAINT.to_ansi_code(Term::Xterm));
    assert_eq!("\x1b[1;3;9m".to_string(),
               (ATTR_INTENSE | ATTR_ITALIC | ATTR_STRIKE).to_ansi_code(Term::Xterm));
}

#[derive(Debug,Clone,PartialEq,Eq)]
pub struct Colors {
    attrs: Attribute, // whether bold or not.  only meaningful when used for FG.

    fg: Color,
    bg: Color,

    fg_code: String, // ANSI color code for foreground
    bg_code: String, // ANSI color code for background.
}

lazy_static! {
    pub static ref NO_COLORS: Colors = Colors::new_empty();
}

impl Colors {
    pub fn new_empty() -> Colors {
        Colors {
            attrs: ATTR_NONE,
            fg: Color::None,
            bg: Color::None,
            fg_code: "".to_string(),
            bg_code: "".to_string(),
        }
    }

    pub fn with_colors(fg: Color, bg: Color, attrs: Attribute, term: Term) -> Colors {
        let mut fg_code = String::new();
        let mut bg_code = String::new();
        if fg != Color::None {
            fg_code.push_str(&attrs.to_ansi_code(term));
            fg_code.push_str(term.csi_start());
            fg_code.push_str(&(color_to_ansi_code(&fg, term).replace(FG_BG_PLACE_HOLDER, "3")));
            fg_code.push_str(term.csi_end());
        }
        if bg != Color::None {
            bg_code.push_str(term.csi_start());
            bg_code.push_str(&(color_to_ansi_code(&bg, term).replace(FG_BG_PLACE_HOLDER, "4")));
            bg_code.push_str(term.csi_end());
        };
        Colors {
            attrs: attrs,
            fg: fg,
            bg: bg,
            fg_code: fg_code,
            bg_code: bg_code,
        }
    }

    pub fn fg_code(&self) -> &String {
        &self.fg_code
    }

    pub fn bg_code(&self) -> &String {
        &self.bg_code
    }

    pub fn fg(&self) -> Color {
        self.fg
    }

    pub fn bg(&self) -> Color {
        self.bg
    }

    pub fn get_color(&self, fg: bool) -> Color {
        if fg { self.fg } else { self.bg }
    }

    pub fn fg_eq(&self, other: &Colors) -> bool {
        (self.attrs == other.attrs) && (self.fg == other.fg)
    }

    pub fn bg_eq(&self, other: &Colors) -> bool {
        (self.bg == other.bg)
    }
}

fn color_rgb_to_xterm(r: u8, g: u8, b: u8) -> u8 {
    let r6 = Color::color_256_to_6(r);
    let g6 = Color::color_256_to_6(g);
    let b6 = Color::color_256_to_6(b);
    return (16 + r6 * 36 + g6 * 6 + b6) as u8;
}

#[test]
fn test_color_rgb_to_xterm() {
    assert_eq!(16, color_rgb_to_xterm(0, 0, 0));
    assert_eq!(16 + 36 * 5, color_rgb_to_xterm(255, 0, 0));
    assert_eq!(16 + 36 * 5 + 6 * 5, color_rgb_to_xterm(255, 255, 0));
    assert_eq!(16 + 36 * 5 + 6 * 5 + 5, color_rgb_to_xterm(255, 255, 255));
}

fn color_rgb_to_3bit(r: u8, g: u8, b: u8) -> u8 {
    let max = max(r, max(g, b));
    if max == 0 {
        return 0;
    }
    let half = max / 2;
    let r_on = r >= half;
    let g_on = g >= half;
    let b_on = b >= half;

    return (if r_on { 1 } else { 0 } + if g_on { 2 } else { 0 } + if b_on { 4 } else { 0 }) as u8;
}

#[test]
fn test_color_rgb_to_3bit() {
    assert_eq!(0, color_rgb_to_3bit(0, 0, 0));
    assert_eq!(1, color_rgb_to_3bit(255, 0, 0));
    assert_eq!(2, color_rgb_to_3bit(0, 255, 0));
    assert_eq!(4, color_rgb_to_3bit(0, 0, 255));
}

fn color_to_ansi_code(color: &Color, term: Term) -> String {
    if term == Term::Dumb {
        return String::new();
    }
    return match color {
            &Color::None => String::new(),

            &Color::Console(index) => format!("#{}", (('0' as u8) + (index as u8)) as char),

            &Color::Rgb(r, g, b) => {
                match term {
                    Term::Rgb => format!("#8;2;{};{};{}", r, g, b),
                    Term::Xterm => format!("#8;5;{}", color_rgb_to_xterm(r, g, b)),
                    Term::Console => format!("#{}", color_rgb_to_3bit(r, g, b)),
                    _ => panic!(), // won't happen
                }
            }

        }
        .to_string();
}
