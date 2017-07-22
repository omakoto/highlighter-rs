extern crate env_logger;

use std::rc::Rc;

use super::*;
use term_color::*;
use rule::*;

#[derive(Debug,Clone)]
struct LayeredColors {
    colors: Rc<Colors>,
    back: Option<Box<LayeredColors>>,
}

impl LayeredColors {
    fn new() -> LayeredColors {
        LayeredColors {
            colors: Rc::new(NO_COLORS.clone()),
            back: None,
        }
    }

    fn with_colors(colors: Rc<Colors>, back: LayeredColors) -> LayeredColors {
        LayeredColors {
            colors: colors,
            back: Some(Box::new(back)),
        }
    }

    fn fg_colors(&self) -> &Colors {
        self.colors_inner(true)
    }

    fn bg_colors(&self) -> &Colors {
        self.colors_inner(false)
    }

    fn colors_inner(&self, fg: bool) -> &Colors {
        let mut cur: &LayeredColors = self;
        while (cur.colors.get_color(fg) == Color::None) && cur.back.is_some() {
            let back_ref: &Option<Box<LayeredColors>> = &cur.back;
            let back_some: &Box<LayeredColors> = back_ref.as_ref().unwrap();
            cur = back_some;
        }
        &cur.colors
    }
}

#[test]
fn test_layered_color() {
    let empty = LayeredColors::new();
    let cf1 = Colors::with_colors(Color::Console(1), Color::None, ATTR_NONE, Term::Xterm);
    let cf2 = Colors::with_colors(Color::Console(2), Color::None, ATTR_NONE, Term::Xterm);
    let cb3 = Colors::with_colors(Color::None, Color::Console(3), ATTR_NONE, Term::Xterm);
    let cb4 = Colors::with_colors(Color::None, Color::Console(4), ATTR_NONE, Term::Xterm);

    let l1 = LayeredColors::with_colors(Rc::new(cf1), empty.clone());
    assert_eq!(Color::Console(1), l1.fg_colors().fg());
    assert_eq!(Color::None, l1.bg_colors().bg());

    let l2 = LayeredColors::with_colors(Rc::new(cf2), l1.clone());
    assert_eq!(Color::Console(2), l2.fg_colors().fg());
    assert_eq!(Color::None, l1.bg_colors().bg());

    let l3 = LayeredColors::with_colors(Rc::new(cb3), l2.clone());
    assert_eq!(Color::Console(2), l3.fg_colors().fg());
    assert_eq!(Color::Console(3), l3.bg_colors().bg());
}


#[derive(Debug)]
struct Matches<'a> {
    rule: &'a Rule,
    ranges: Vec<(usize, usize)>,
}

#[derive(Debug)]
pub struct Filter {
    term: Term,
    rules: Vec<Rule>,
    state: String,
}

impl Filter {
    pub fn new(term: Term, rules: Vec<Rule>) -> Filter {
        Filter {
            term: term,
            rules: rules,
            state: String::new(),
        }
    }

    pub fn process<F>(&mut self, line: &str, out: F)
        where F: Fn(&str)
    {
        debug!("line={}", line);

        // Find matches.
        let mut matches: Vec<Matches> = vec![];
        for r in &self.rules {
            if r.states().len() > 0 {
                if !r.states().contains(&self.state) {
                    continue;
                }
            }
            let m = r.matches(line);
            if m.len() == 0 {
                continue;
            }
            debug!("  found={}, {:?}", r.pattern(), m);
            debug!("    states='{:?}'", r.states());
            if let Some(s) = r.next_state() {
                self.state = s.clone();
                debug!("    next_state='{}'", self.state);
            }
            matches.push(Matches {
                rule: &r,
                ranges: m,
            });
            if r.stop() {
                break;
            }
        }

        // Show pre lines.
        for m in &matches {
            if let Some(ref l) = m.rule.pre_line() {
                out(&l.computed_line());
            }
        }

        let mut ca = Vec::with_capacity(line.len());
        ca.resize(line.len(), LayeredColors::new());

        // From here, we apply in the reverse order.
        matches.reverse();

        // First, apply the line colors.
        for m in &matches {
            for r in &m.ranges {
                if let Some(c) = m.rule.line_colors() {
                    for i in 0..line.len() {
                        ca[i] = LayeredColors::with_colors(c.clone(), ca[i].clone());
                    }
                }
            }
        }

        // Apply match colors (in the reverse order).
        for m in &matches {
            for r in &m.ranges {
                for i in (r.0)..(r.1) {
                    if let Some(c) = m.rule.match_colors() {
                        ca[i] = LayeredColors::with_colors(c.clone(), ca[i].clone());
                    }
                }
            }
        }

        // Now build the result, char by char.
        let mut res = String::new();

        let line_bytes = line.as_bytes();

        let mut last_fg: &Colors = &NO_COLORS;
        let mut last_bg: &Colors = &NO_COLORS;

        let mut in_color = false;

        for i in 0..line.len() {
            let fg = ca[i].fg_colors();
            let bg = ca[i].bg_colors();

            if !fg.fg_eq(last_fg) || !bg.bg_eq(last_bg) {
                if in_color {
                    res.push_str(self.term.csi_reset());
                    in_color = false;
                }
                res.push_str(fg.fg_code());
                res.push_str(bg.bg_code());

                last_fg = fg;
                last_bg = bg;
                in_color = true;
            }

            res.push(line_bytes[i] as char);
        }
        if in_color {
            res.push_str(self.term.csi_reset());
            in_color = false;
        }

        out(&res);

        // Show post lines.
        for m in &matches {
            if let Some(ref l) = m.rule.post_line() {
                out(&l.computed_line());
            }
        }
    }
}
