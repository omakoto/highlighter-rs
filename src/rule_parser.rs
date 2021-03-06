extern crate env_logger;

use std::error::Error;
use std::env;
use std::iter;
use std::cell::*;
use std::io;
use std::io::BufReader;
use std::io::prelude::*;
use std::fs::File;
use std::collections::BTreeMap;
use pcre::{CompileOption, Match, Pcre};
use toml;
use toml::Value;

use super::*;
use term_color::*;
use rule::*;

#[derive(Debug)]
struct ColorParser {
    term: Term,
    re: Pcre,
}

macro_rules! COLOR_RE {
    () => {r"(?:
            (black|red|green|yellow|blue|magenta|cyan|white)
            |
            (\d{3})
            |
            (?: ([0-9a-f]{2}) \,? ([0-9a-f]{2}) \,? ([0-9a-f]{2}))
            )"
    }
}

const COLORS_RE: &'static str = concat!(r"(?xi) ^",
                                        r" (?: ([bifus]*) \s* ",
                                        COLOR_RE!(),
                                        r")?",
                                        r"\s* (?: \/ \s* ",
                                        COLOR_RE!(),
                                        r" )? $");

impl ColorParser {
    fn new(term: Term) -> ColorParser {
        let re = match Pcre::compile(COLORS_RE) {
            Err(err) => {
                panic!("Pcre failed");
            }
            Ok(re) => re,
        };
        ColorParser {
            term: term,
            re: re,
        }
    }

    fn get_group(m: &Match, i: usize) -> String {
        if m.group_len(i) == 0 {
            String::new()
        } else {
            m.group(i).to_string()
        }
    }

    fn hex_to_u8(s: &String) -> Option<u8> {
        if s.len() == 0 {
            None
        } else {
            Some(u8::from_str_radix(s, 16).unwrap())
        }
    }

    #[test]
    fn test_hex_to_u8() {
        assert!(ColorParser::hex_to_u8(&("".to_string())).is_none());
        assert_eq!(0, ColorParser::hex_to_u8(&("0".to_string())).unwrap());
        assert_eq!(15, ColorParser::hex_to_u8(&("f".to_string())).unwrap());
        assert_eq!(254, ColorParser::hex_to_u8(&("fe".to_string())).unwrap());
    }

    fn rgb666_to_u16(s: &String) -> Option<u16> {
        if s.len() == 0 {
            None
        } else {
            Some(s.parse::<u16>().unwrap())
        }
    }

    fn to_color_index(s: &String) -> Option<u8> {
        if s.len() == 0 {
            None
        } else {
            let bytes = s.as_bytes();
            Some(match bytes[0] as char {
                'r' => 1,
                'g' => 2,
                'y' => 3,
                'm' => 5,
                'c' => 6,
                'w' => 7,
                _ => if bytes[2] as char == 'a' { 0 } else { 4 },
            })
        }
    }

    #[test]
    fn test_to_color_index() {
        assert!(ColorParser::to_color_index(&("".to_string())).is_none());
        assert_eq!(0,
                   ColorParser::to_color_index(&("black".to_string())).unwrap());
        assert_eq!(1,
                   ColorParser::to_color_index(&("red".to_string())).unwrap());
        assert_eq!(2,
                   ColorParser::to_color_index(&("green".to_string())).unwrap());
        assert_eq!(3,
                   ColorParser::to_color_index(&("yellow".to_string())).unwrap());
        assert_eq!(4,
                   ColorParser::to_color_index(&("blue".to_string())).unwrap());
        assert_eq!(5,
                   ColorParser::to_color_index(&("magenta".to_string())).unwrap());
        assert_eq!(6,
                   ColorParser::to_color_index(&("cyan".to_string())).unwrap());
        assert_eq!(7,
                   ColorParser::to_color_index(&("white".to_string())).unwrap());
    }

    fn to_color(index: Option<u8>,
                 rgb666: Option<u16>,
                 r: Option<u8>,
                 g: Option<u8>,
                 b: Option<u8>)
                 -> Color {
        if index.is_some() {
            return Color::with_index(index.unwrap());
        }
        if rgb666.is_some() {
            return Color::with_xterm_color(rgb666.unwrap());
        }
        if r.is_some() && g.is_some() && b.is_some() {
            return Color::with_rgb(r.unwrap(), g.unwrap(), b.unwrap());
        }
        Color::None
    }

    fn parse(&self, value: &str) -> Result<Colors, RuleError> {
        let m = match self.re.exec(&value) {
            None => return Err(RuleError::new(&format!("Invalid color: {}", value))),
            Some(m) => m,
        };
        let prefix = ColorParser::get_group(&m, 1);
        let fg_index = ColorParser::to_color_index(&ColorParser::get_group(&m, 2));
        let fg_rgb666 = ColorParser::rgb666_to_u16(&ColorParser::get_group(&m, 3));
        let fg_r = ColorParser::hex_to_u8(&ColorParser::get_group(&m, 4));
        let fg_g = ColorParser::hex_to_u8(&ColorParser::get_group(&m, 5));
        let fg_b = ColorParser::hex_to_u8(&ColorParser::get_group(&m, 6));

        let bg_index = ColorParser::to_color_index(&ColorParser::get_group(&m, 7));
        let bg_rgb666 = ColorParser::rgb666_to_u16(&ColorParser::get_group(&m, 8));
        let bg_r = ColorParser::hex_to_u8(&ColorParser::get_group(&m, 9));
        let bg_g = ColorParser::hex_to_u8(&ColorParser::get_group(&m, 10));
        let bg_b = ColorParser::hex_to_u8(&ColorParser::get_group(&m, 11));

        let mut attrs = ATTR_NONE;
        if prefix.contains('b') {
            attrs |= ATTR_INTENSE;
        }
        if prefix.contains('i') {
            attrs |= ATTR_INTENSE;
        }
        if prefix.contains('u') {
            attrs |= ATTR_UNDERLINE;
        }
        if prefix.contains('s') {
            attrs |= ATTR_STRIKE;
        }
        if prefix.contains('f') {
            attrs |= ATTR_FAINT;
        }
        Ok(Colors::with_colors(ColorParser::to_color(fg_index, fg_rgb666, fg_r, fg_g, fg_b),
                               ColorParser::to_color(bg_index, bg_rgb666, bg_r, bg_g, bg_b),
                               attrs,
                               self.term))
    }
}

#[test]
fn test_parse_color() {
    env_logger::init().unwrap();

    let mut parser = ColorParser::new(Term::Xterm);

    parser.parse(&"red".to_string()).unwrap();
    parser.parse(&"bred".to_string()).unwrap();
    parser.parse(&"bu 555".to_string()).unwrap();
    parser.parse(&"i 001122".to_string()).unwrap();
    parser.parse(&"ibusf 001122/blue".to_string()).unwrap();
    parser.parse(&"/00,11,fF".to_string()).unwrap();
}

pub struct RuleParser {
    color_parser: ColorParser,
    term: Term,
    output_width: usize,
    default_color_index: i32,
}

impl RuleParser {
    pub fn new(term: Term, output_width: usize) -> RuleParser {
        RuleParser {
            color_parser: ColorParser::new(term),
            term: term,
            output_width: output_width,
            default_color_index: -1,
        }
    }

    fn str_from_table<'a>(map: &'a BTreeMap<String, Value>,
                          key: &str)
                          -> Result<&'a str, RuleError> {
        match map.get(key) {
            None => Err(RuleError::new(&format!("Missing key '{}'.", key))),
            Some(v) => {
                v.as_str().ok_or(RuleError::new(&format!("Key '{}' must contain a string.", key)))
            }
        }
    }

    fn bool_from_table<'a>(map: &'a BTreeMap<String, Value>, key: &str) -> Result<bool, RuleError> {
        match map.get(key) {
            None => Err(RuleError::new(&format!("Missing key '{}'.", key))),
            Some(v) => {
                v.as_bool().ok_or(RuleError::new(&format!("Key '{}' must contain a boolean.", key)))
            }
        }
    }

    fn slice_from_table<'a>(map: &'a BTreeMap<String, Value>,
                            key: &str)
                            -> Result<Vec<String>, RuleError> {
        let mut ret = vec![];
        let mut ok = true;
        if let Some(value) = map.get(key) {
            if let Some(slice) = value.as_slice() {
                debug!("XXX {:?}", slice);
                for s in slice {
                    debug!("    {:?}", s);
                    match s.as_str() {
                        Some(v) => ret.push(v.to_string()),
                        None => {
                            ok = false;
                            break;
                        }
                    }
                }
            } else {
                ok = false;
            }
            if ok {
                return Ok(ret);
            } else {
                return Err(RuleError::new(&format!("Key '{}' must contain a string array.", key)));
            }
        }
        Err(RuleError::new(&format!("Missing key '{}'.", key)))
    }

    fn get_default_color(&mut self) -> &'static str {
        self.default_color_index += 1;
        return match self.default_color_index {
            0 => "b055@/012",
            1 => "b550@/110",
            2 => "b505@/101",
            3 => "b511@/100",
            _ => {
                self.default_color_index = -1;
                "b151@/010"
            },
        };
    }

    pub fn parse_simple_rule(&mut self, value: &str) -> Result<Rule, RuleError> {
        // Split with "="
        let pattern;
        let mut rest;
        if let Some(p) = value.rfind('=') {
            pattern = &value[0..p];
            rest = &value[p + 1..value.len()];
        } else {
            pattern = &value;
            rest = "";
        }
        if rest.len() == 0 {
            rest = self.get_default_color();
        }
        if pattern.len() == 0 {
            return Err(RuleError::new("Pattern can't be empty."));
        }

        // Split the right-hand side with "@"
        let color;
        let line_color;
        if let Some(p) = rest.find('@') {
            color = &rest[0..p];
            line_color = &rest[p + 1..rest.len()];
        } else {
            color = &rest;
            line_color = &"";
        }

        debug!("  pattern={}, color={}, line_color={}",
               pattern,
               color,
               line_color);

        let mut rule = try!(Rule::new(&pattern));

        if color.len() > 0 {
            let c = try!(self.color_parser.parse(color));
            rule.set_match_colors(c);
        }
        if line_color.len() > 0 {
            let c = try!(self.color_parser.parse(line_color));
            rule.set_line_colors(c);
        }
        if color.len() == 0 && line_color.len() == 0 {
            rule.set_match_colors(self.color_parser.parse("bred").unwrap());
        }

        debug!("rule={:?}", rule);
        Ok(rule)
    }

    fn new_decorative_line(&self, marker: &str, colors: &Option<Colors>) -> DecorativeLine {
        DecorativeLine::new(marker, colors.clone(), self.term, self.output_width)
    }

    pub fn parse_legacy(&self, filename: &str, mut rules: &mut Vec<Rule>) -> Result<(), RuleError> {
        debug!("Reading legacy rule file from {}...", filename);

        let file = BufReader::new(try!(File::open(&filename)
            .map_err(|e| RuleError::new(&format!("Unable to open file '{}'", filename)))));

        // I just wanted "add_rule()" as a closure, but couldn't because of
        // borrow checking... So I needed "expand" the closure by myself
        // to access the borrowed values in it.
        struct State {
            rule: Option<Rule>,

            pre_line: Option<String>,
            pre_line_color: Option<Colors>,
            post_line: Option<String>,
            post_line_color: Option<Colors>,

            term: Term,
            output_width: usize,
        }

        impl State {
            fn new_decorative_line(p: &RuleParser,
                                   pre_line: &Option<String>,
                                   pre_line_color: &Option<Colors>)
                                   -> DecorativeLine {
                let pl = pre_line.as_ref().unwrap_or(&String::new()).clone();
                p.new_decorative_line(&pl, pre_line_color)
            }

            fn add_rule(&mut self, p: &RuleParser, rules: &mut Vec<Rule>) {
                if self.rule.is_none() {
                    // No previous rule.
                    return;
                }
                {
                    let rule = self.rule.as_mut().unwrap();
                    if self.pre_line.is_some() || self.pre_line_color.is_some() {
                        let dl =
                            State::new_decorative_line(p, &self.pre_line, &self.pre_line_color);
                        rule.set_pre_line(dl);
                    }
                    if self.post_line.is_some() || self.post_line_color.is_some() {
                        let dl =
                            State::new_decorative_line(p, &self.post_line, &self.post_line_color);
                        rule.set_post_line(dl);
                    }

                    debug!("{:?}", rule);
                    rules.push(rule.clone());
                }
                self.rule = None;
                self.pre_line = None;
                self.pre_line_color = None;
                self.post_line = None;
                self.post_line_color = None;
            }
        }

        let mut state = State {
            rule: None,
            pre_line: None,
            pre_line_color: None,
            post_line: None,
            post_line_color: None,
            term: self.term,
            output_width: self.output_width,
        };

        let mut line_no = 0;
        for line_res in file.lines() {
            line_no += 1;

            if let Err(e) = line_res {
                return Err(RuleError::new(&format!("Error reading from '{}': {}", filename, e)));
            }
            let line = line_res.unwrap().trim().to_string();

            if line.len() == 0 || line.starts_with('/') || line.starts_with('#') {
                continue;
            }
            // Split with '='.
            let key;
            let value;
            if let Some(p) = line.find('=') {
                key = line[0..p].trim();
                value = line[p + 1..line.len()].trim();
            } else {
                key = &line;
                value = &"";
            }

            debug!("  {}={}", key, value);

            if key == "pattern" {
                state.add_rule(self, &mut rules);
                state.rule = Some(try!(Rule::new(&value)));
                continue;
            }

            if state.rule.is_none() {
                return Err(RuleError::new(&format!("Error reading from '{}': file must start \
                                                    with 'pattern' ",
                                                   filename)));
            }

            match key {
                ".when" => {
                    try!(state.rule.as_mut().unwrap().set_when(value.to_string()));
                }
                ".states" => {
                    // state.rule.set_states(value.to_string());
                    let mut vals: Vec<String> = vec![];
                    for s in value.split(",") {
                        let s = if s == "INIT" { "" } else { s.trim() };
                        vals.push(s.to_string());
                    }
                    state.rule.as_mut().unwrap().set_states(vals);
                }
                ".next_state" => {
                    let s = if value == "INIT" { "" } else { value };
                    state.rule.as_mut().unwrap().set_next_state(s.to_string());
                }
                ".color" => {
                    let c = try!(self.color_parser.parse(value));
                    state.rule.as_mut().unwrap().set_match_colors(c);
                }
                ".stop" => {
                    state.rule.as_mut().unwrap().set_stop(true);
                }
                ".line_color" => {
                    let c = try!(self.color_parser.parse(value));
                    state.rule.as_mut().unwrap().set_line_colors(c);
                }
                ".pre_line" => {
                    state.pre_line = Some(value.to_string());
                }
                ".pre_line_color" => {
                    state.pre_line_color = Some(try!(self.color_parser.parse(value)));
                }
                ".post_line" => {
                    state.post_line = Some(value.to_string());
                }
                ".post_line_color" => {
                    state.post_line_color = Some(try!(self.color_parser.parse(value)));
                }
                _ => {
                    return Err(RuleError::new(&format!("Error reading from '{}': Invalid key \
                                                        '{}'",
                                                       filename,
                                                       key)));
                }
            }
        }
        state.add_rule(self, &mut rules);
        Ok(())
    }

    pub fn parse_toml(&self, filename: &str, rules: &mut Vec<Rule>) -> Result<(), RuleError> {
        debug!("Reading toml rule file from {}...", filename);

        // Load file content.
        let mut rule = String::new();
        try!(File::open(&filename)
            .and_then(|mut f| f.read_to_string(&mut rule))
            .map_err(|e| RuleError::new(&format!("Unable to open file '{}'", filename))));

        // If the first line starts with '/', then ignore it.
        if rule.starts_with("/") {
            if let Some(first_nl) = rule.find('\n') {
                // Remove the first line, but keep the LF, to preserve line numbers.
                rule.drain(..(first_nl));
            }
        }
        // debug!("TOML: {}", rule);

        // Parse TOML.
        let mut parser = toml::Parser::new(&rule);
        let toml = match parser.parse() {
            Some(toml) => toml,
            None => {
                for err in &parser.errors {
                    let (loline, locol) = parser.to_linecol(err.lo);
                    let (hiline, hicol) = parser.to_linecol(err.hi);
                    return Err(RuleError::new(&format!("Invalid TOML: {}:{}:{}-{}:{} error: {}",
                                                       filename,
                                                       loline + 1,
                                                       locol + 1,
                                                       hiline + 1,
                                                       hicol + 1,
                                                       err.desc)));
                }
                return Err(RuleError::new("Invalid TOML: Unknown error."));
            }
        };

        // Parse the structure.
        if toml.len() == 0 {
            return Err(RuleError::new("Invalid TOML: No [[rule]] found."));
        }
        if toml.len() > 1 {
            return Err(RuleError::new("Invalid TOML: Invalid TOML: Must only contain [[rule]]s."));
        }
        let rules_array = match toml.get("rule") {
            Some(&Value::Array(ref v)) => v,
            _ => return Err(RuleError::new("Invalid TOML: No [[rule]]s found.")),
        };

        debug!("# rules={}", rules_array.len());

        debug!("Rules count={}", rules_array.len());
        for raw_rule in rules_array {
            debug!("value={:?}", raw_rule);
            let rule_table = try!(raw_rule.as_table()
                .ok_or(RuleError::new("\'rule\' key fond, but it's not a table.")));

            let pattern = try!(RuleParser::str_from_table(rule_table, "pattern"));
            let mut rule = try!(Rule::new(&pattern));

            let mut pre_line: Option<String> = None;
            let mut pre_line_color: Option<Colors> = None;
            let mut post_line: Option<String> = None;
            let mut post_line_color: Option<Colors> = None;

            for key in rule_table.keys() {
                match key.as_ref() {
                    "pattern" => (), // Already parsed.
                    k @ "when" => {
                        let p = try!(RuleParser::str_from_table(rule_table, k));
                        try!(rule.set_when(p.to_string()));
                        ()
                    }
                    k @ "color" => {
                        let c = try!(self.color_parser
                            .parse(try!(RuleParser::str_from_table(rule_table, k))));
                        rule.set_match_colors(c);
                        ()
                    }
                    k @ "line_color" => {
                        rule.set_line_colors(try!(self.color_parser
                            .parse(try!(RuleParser::str_from_table(rule_table, k)))));
                        ()
                    }
                    k @ "states" => {
                        rule.set_states(try!(RuleParser::slice_from_table(rule_table, k)));
                    }
                    k @ "next_state" => {
                        rule.set_next_state(try!(RuleParser::str_from_table(rule_table, k))
                            .to_string());
                    }
                    k @ "stop" => {
                        rule.set_stop(try!(RuleParser::bool_from_table(rule_table, k)));
                    }
                    k @ "pre_line" => {
                        pre_line = Some(try!(RuleParser::str_from_table(rule_table, k))
                            .to_string());
                        ()
                    }
                    k @ "pre_line_color" => {
                        pre_line_color = Some(try!(self.color_parser
                            .parse(try!(RuleParser::str_from_table(rule_table, k)))));
                        ()
                    }
                    k @ "post_line" => {
                        post_line = Some(try!(RuleParser::str_from_table(rule_table, k))
                            .to_string());
                        ()
                    }
                    k @ "post_line_color" => {
                        post_line_color = Some(try!(self.color_parser
                            .parse(try!(RuleParser::str_from_table(rule_table, k)))));
                        ()
                    }
                    _ => return Err(RuleError::new(&format!("Unknown key '{}'.", key))),
                }
            }
            if pre_line.is_some() || pre_line_color.is_some() {
                rule.set_pre_line(self.new_decorative_line(&pre_line.unwrap_or("".to_string()),
                                                      &pre_line_color));
            }
            if post_line.is_some() || post_line_color.is_some() {
                rule.set_post_line(self.new_decorative_line(&post_line.unwrap_or("".to_string()),
                                                            &post_line_color));
            }

            debug!("rule={:?}", rule);

            rules.push(rule);
        }

        Ok(())
    }
}

#[test]
fn test_rule_parser() {
    let p = RuleParser::new(Term::Xterm, 80);
}

#[test]
fn test_parse_simple_rule() {
    let mut p = RuleParser::new(Term::Xterm, 80);

    assert!(p.parse_simple_rule("=").is_err());

    let r = p.parse_simple_rule("a=").unwrap();
    assert_eq!("a", r.pattern());
    assert_eq!("Some(Colors { attrs: ATTR_INTENSE, fg: Rgb(255, 255, 0), \
        bg: None, fg_code: \"\\u{1b}[1m\\u{1b}[38;5;226m\", bg_code: \"\" })",
               format!("{:?}", r.match_colors()));
    assert_eq!("Some(Colors { attrs: , fg: None, bg: Rgb(51, 51, 0), \
        fg_code: \"\", bg_code: \"\\u{1b}[48;5;58m\" })",
                format!("{:?}", r.line_colors()));

    let r = p.parse_simple_rule("b").unwrap();
    assert_eq!("b", r.pattern());
    assert_eq!("Some(Colors { attrs: ATTR_INTENSE, fg: Rgb(255, 0, 255), \
        bg: None, fg_code: \"\\u{1b}[1m\\u{1b}[38;5;201m\", bg_code: \"\" })",
               format!("{:?}", r.match_colors()));
    assert_eq!("Some(Colors { attrs: , fg: None, bg: Rgb(51, 0, 51), \
        fg_code: \"\", bg_code: \"\\u{1b}[48;5;53m\" })", format!("{:?}", r.line_colors()));

    let r = p.parse_simple_rule("c").unwrap();
    assert_eq!("c", r.pattern());
    assert_eq!("Some(Colors { attrs: ATTR_INTENSE, fg: Rgb(255, 51, 51), \
        bg: None, fg_code: \"\\u{1b}[1m\\u{1b}[38;5;203m\", bg_code: \"\" })",
               format!("{:?}", r.match_colors()));
    assert_eq!("Some(Colors { attrs: , fg: None, bg: Rgb(51, 0, 0), \
        fg_code: \"\", bg_code: \"\\u{1b}[48;5;52m\" })",
                format!("{:?}", r.line_colors()));

    let r = p.parse_simple_rule("a").unwrap();
    let r = p.parse_simple_rule("a").unwrap();
    let r = p.parse_simple_rule("a").unwrap();

    let r = p.parse_simple_rule("a").unwrap();
    assert_eq!("a", r.pattern());
    assert_eq!("Some(Colors { attrs: ATTR_INTENSE, fg: Rgb(255, 0, 255), \
        bg: None, fg_code: \"\\u{1b}[1m\\u{1b}[38;5;201m\", bg_code: \"\" })",
               format!("{:?}", r.match_colors()));
    assert_eq!("Some(Colors { attrs: , fg: None, bg: Rgb(51, 0, 51), \
        fg_code: \"\", bg_code: \"\\u{1b}[48;5;53m\" })", format!("{:?}", r.line_colors()));

    let r = p.parse_simple_rule("a=333").unwrap();
    assert_eq!("a", r.pattern());
    assert_eq!("Some(Colors { attrs: , fg: Rgb(153, 153, 153), bg: None, \
        fg_code: \"\\u{1b}[38;5;145m\", bg_code: \"\" })",
               format!("{:?}", r.match_colors()));
    assert_eq!("None", format!("{:?}", r.line_colors()));

    let r = p.parse_simple_rule("a=333/red").unwrap();
    assert_eq!("a", r.pattern());
    assert_eq!("Some(Colors { attrs: , fg: Rgb(153, 153, 153), bg: Console(1), \
        fg_code: \"\\u{1b}[38;5;145m\", bg_code: \"\\u{1b}[41m\" })",
               format!("{:?}", r.match_colors()));
    assert_eq!("None", format!("{:?}", r.line_colors()));

    let r = p.parse_simple_rule("a=/red").unwrap();
    assert_eq!("a", r.pattern());
    assert_eq!("Some(Colors { attrs: , fg: None, bg: Console(1), \
        fg_code: \"\", bg_code: \"\\u{1b}[41m\" })",
               format!("{:?}", r.match_colors()));
    assert_eq!("None", format!("{:?}", r.line_colors()));

    let r = p.parse_simple_rule("a=333@444").unwrap();
    assert_eq!("a", r.pattern());
    assert_eq!("Some(Colors { attrs: , fg: Rgb(153, 153, 153), bg: None, \
        fg_code: \"\\u{1b}[38;5;145m\", bg_code: \"\" })",
               format!("{:?}", r.match_colors()));
    assert_eq!("Some(Colors { attrs: , fg: Rgb(204, 204, 204), bg: None, \
        fg_code: \"\\u{1b}[38;5;188m\", bg_code: \"\" })",
               format!("{:?}", r.line_colors()));

    let r = p.parse_simple_rule("a=@444").unwrap();
    assert_eq!("a", r.pattern());
    assert_eq!("None", format!("{:?}", r.match_colors()));
    assert_eq!("Some(Colors { attrs: , fg: Rgb(204, 204, 204), bg: None, \
        fg_code: \"\\u{1b}[38;5;188m\", bg_code: \"\" })",
               format!("{:?}", r.line_colors()));
}



// from toml: https://github.com/alexcrichton/toml-rs/blob/master/src/lib.rs
//
// Representation of a TOML value.
// #[derive(PartialEq, Clone, Debug)]
// #[allow(missing_docs)]
// pub enum Value {
// String(String),
// Integer(i64),
// Float(f64),
// Boolean(bool),
// Datetime(String),
// Array(Array),
// Table(Table),
// }
//
// Type representing a TOML array, payload of the `Value::Array` variant
// pub type Array = Vec<Value>;
//
// Type representing a TOML table, payload of the `Value::Table` variant
// pub type Table = BTreeMap<String, Value>;
//
