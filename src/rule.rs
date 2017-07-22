use std;
use std::iter;
use std::rc::Rc;
use std::cell::*;
use pcre::{CompileOption, Match, Pcre};

use super::*;
use term_color::*;

#[derive(Debug,Clone)]
pub struct DecorativeLine {
    /// Single mark, such as "*".
    mark: String,

    /// Colors.
    colors: Option<Colors>,

    /// Repeated marks to fill a given width, with colors.
    computed_line: String,
}

impl DecorativeLine {
    pub fn new(mark: &str,
               colors: Option<Colors>,
               term: Term,
               console_width: usize)
               -> DecorativeLine {
        let m = if mark.len() > 0 {
            mark.to_string()
        } else {
            "-".to_string()
        };
        let len = m.len();
        let mut computed = String::new();

        if let Some(ref c) = colors {
            computed.push_str(c.fg_code());
            computed.push_str(c.bg_code());
        };

        computed.push_str(
            &std::iter::repeat(m.clone()).take(console_width / len).collect::<String>());
        if colors.is_some() {
            computed.push_str(term.csi_reset());
        }

        DecorativeLine {
            mark: m,
            colors: colors,
            computed_line: computed,
        }
    }

    pub fn computed_line(&self) -> &String {
        &self.computed_line
    }
}

#[test]
fn test_build_decorative_line() {
    assert_eq!("-----".to_string(),
               *DecorativeLine::new(&"".to_string(), None, Term::Xterm, 5).computed_line());
    assert_eq!("*****".to_string(),
               *DecorativeLine::new(&"*".to_string(), None, Term::Xterm, 5).computed_line());
    assert_eq!("*-*-".to_string(),
               *DecorativeLine::new(&"*-".to_string(), None, Term::Xterm, 5).computed_line());
}

#[derive(Debug)]
struct PcreEx {
    pattern: String,
    re: Pcre,
    negate: bool,
}

impl PcreEx {
    fn compile(orig_pattern: &str) -> Result<PcreEx, RuleError> {
        let bytes = orig_pattern.as_bytes();
        let negate = bytes[0] == '!' as u8;
        let pattern = if negate {
            orig_pattern[1..].to_string()
        } else {
            orig_pattern.to_string()
        };

        let re = try!(Pcre::compile(&pattern)
            .map_err(|e| RuleError::new(&format!("Invalid regex pattern: {}", pattern))));

        Ok(PcreEx {
            pattern: orig_pattern.to_string(),
            re: re,
            negate: negate,
        })
    }

    fn test(&self, line: &str) -> bool {
        let mut ret = self.re.exec(line).is_some();
        if self.negate {
            ret = !ret;
        }
        ret
    }

    fn matches(&self, line: &str) -> Vec<(usize, usize)> {
        if self.negate {
            if self.test(line) {
                return vec![(0, line.len())];
            } else {
                return vec![];
            }
        }
        let cc = self.re.capture_count();
        let mut ret = vec![];
        for m in self.re.matches(line) {
            let mut pusher = |i| {
                if m.group_len(i) > 0 {
                    ret.push((m.group_start(i), m.group_end(i)))
                }
            };
            if cc == 0 {
                pusher(0);
            } else {
                for i in 1..(cc + 1) {
                    pusher(i);
                }
            }
        }
        ret
    }

    #[test]
    fn test_matches() {
        let mut pat1 = PcreEx::compile("abc").unwrap();
        let mut pat2 = PcreEx::compile("(a)b(c)").unwrap();

        assert_eq!(pat1.matches(&""), vec![]);
        assert_eq!(pat1.matches(&"1abc2"), vec![(1, 4)]);
        assert_eq!(pat1.matches(&"1abc2abc"), vec![(1, 4), (5, 8)]);

        assert_eq!(pat2.matches(&""), vec![]);
        assert_eq!(pat2.matches(&"1abc2"), vec![(1, 2), (3, 4)]);
        assert_eq!(pat2.matches(&"1abc2abc"),
                   vec![(1, 2), (3, 4), (5, 6), (7, 8)]);
    }
}
impl Clone for PcreEx {
    fn clone(&self) -> PcreEx {
        PcreEx::compile(&self.pattern).unwrap()
    }
}

#[derive(Debug, Clone)]
pub struct Rule {
    /// Original regex
    re: PcreEx,

    when_re: Option<PcreEx>,

    states: Vec<String>,
    next_state: Option<String>,

    stop: bool,

    match_colors: Option<Rc<Colors>>,
    line_colors: Option<Rc<Colors>>,

    pre_line: Option<DecorativeLine>,
    post_line: Option<DecorativeLine>,
}

impl Rule {
    pub fn new(pattern: &str) -> Result<Rule, RuleError> {
        if pattern.len() == 0 {
            return Err(RuleError::new("No pattern found"));
        }

        let re = try!(PcreEx::compile(pattern)
            .map_err(|e| RuleError::new(&format!("Invalid regex pattern: {}", pattern))));

        Ok(Rule {
            re: re,
            when_re: None,
            states: vec![],
            next_state: None,
            stop: false,
            match_colors: None,
            line_colors: None,
            pre_line: None,
            post_line: None,
        })
    }

    pub fn set_when(&mut self, pattern: String) -> Result<&mut Rule, RuleError> {
        let re = try!(PcreEx::compile(&pattern)
            .map_err(|e| RuleError::new(&format!("Invalid regex pattern: {}", pattern))));
        self.when_re = Some(re);
        Ok(self)
    }

    pub fn set_next_state(&mut self, state: String) -> &mut Rule {
        self.next_state = Some(state.to_string());
        self
    }

    pub fn set_states(&mut self, states: Vec<String>) -> &mut Rule {
        self.states = states.clone();
        self
    }

    pub fn set_stop(&mut self, stop: bool) -> &mut Rule {
        self.stop = stop;
        self
    }

    pub fn set_match_colors(&mut self, c: Colors) -> &mut Rule {
        self.match_colors = Some(Rc::new(c));
        self
    }

    pub fn set_line_colors(&mut self, c: Colors) -> &mut Rule {
        self.line_colors = Some(Rc::new(c));
        self
    }

    pub fn set_pre_line(&mut self, line: DecorativeLine) -> &mut Rule {
        self.pre_line = Some(line);
        self
    }

    pub fn set_post_line(&mut self, line: DecorativeLine) -> &mut Rule {
        self.post_line = Some(line);
        self
    }

    pub fn matches(&self, line: &str) -> Vec<(usize, usize)> {
        if let Some(ref re) = self.when_re {
            if !re.test(line) {
                return vec![];
            }
        }
        self.re.matches(line)
    }

    pub fn pattern(&self) -> &String {
        &self.re.pattern
    }

    pub fn match_colors(&self) -> Option<Rc<Colors>> {
        self.match_colors.as_ref().map(|x| x.clone())
    }

    pub fn line_colors(&self) -> Option<Rc<Colors>> {
        self.line_colors.as_ref().map(|x| x.clone())
    }

    pub fn states(&self) -> &Vec<String> {
        &self.states
    }

    pub fn next_state(&self) -> Option<&String> {
        self.next_state.as_ref()
    }

    pub fn pre_line(&self) -> Option<&DecorativeLine> {
        self.pre_line.as_ref()
    }

    pub fn post_line(&self) -> Option<&DecorativeLine> {
        self.post_line.as_ref()
    }

    pub fn stop(&self) -> bool {
        self.stop
    }
}

#[test]
fn test_matches_zero_width() {
    let mut pat1 = Pcre::compile("^").unwrap();

    // TODO Send a pull request to fix it.
    // assert_eq!(Rule::matches_inner(&mut pat1, &"abc"), vec![]);
}

#[test]
fn test_build_rule() {
    let mut rule = Rule::new(&String::from("xyz")).unwrap();
    rule.set_next_state("".to_string()).set_states(vec![]);
}
