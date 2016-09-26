#[macro_use]
extern crate bitflags;
#[macro_use]
extern crate log;
#[macro_use]
extern crate lazy_static;
extern crate env_logger;
extern crate pcre;
extern crate toml;

use std::env;
use std::error::Error;
use std::fmt;
use std::io;

pub mod rule;
pub mod rule_parser;
pub mod term_color;
pub mod filter;

const CSI: &'static str = "\x1b[";
const CSI_END: &'static str = "m";
const CSI_RESET: &'static str = "\x1b[0m";
const NO_CSI: &'static str = "";

#[derive(Debug)]
pub enum RuleError {
    ParseError(String),
    // IoError(String, std::io::Error),
}

impl RuleError {
    pub fn new(description: &str) -> RuleError {
        RuleError::ParseError(description.to_string())
    }
}

// impl From<io::Error> for RuleError {
//     fn from(err: io::Error) -> RuleError {
//         RuleError::IoError("I/O error".to_string(), err)
//     }
// }

impl Error for RuleError {
    fn description(&self) -> &str {
        match self {
            &RuleError::ParseError(ref desc) => desc,
            // &RuleError::IoError(ref desc, ref err) => err.description(),
        }
    }

    fn cause(&self) -> Option<&Error> {
        match self {
            &RuleError::ParseError(ref desc) => None,
            // &RuleError::IoError(ref desc, ref err) => Some(err),
        }
    }
}

impl fmt::Display for RuleError {
    fn fmt(self: &RuleError, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.description())
    }
}

#[derive(Debug,Copy,Clone,PartialEq,Eq)]
pub enum Term {
    Dumb,
    Console,
    Xterm,
    Rgb,
}

impl Term {
    pub fn detect() -> Term {
        if let Ok(v) = env::var("TERM") {
            if v == "xterm" {
                if let Ok(v) = env::var("XTERM_FULLCOLOR") {
                    Term::Rgb
                } else {
                    Term::Xterm
                }
            } else {
                Term::Console
            }
        } else {
            Term::Dumb
        }
    }

    pub fn csi_start(&self) -> &'static str {
        if *self == Term::Dumb { NO_CSI } else { CSI }
    }

    pub fn csi_end(&self) -> &'static str {
        if *self == Term::Dumb { NO_CSI } else { CSI_END }
    }

    pub fn csi_reset(&self) -> &'static str {
        if *self == Term::Dumb { NO_CSI } else { CSI_RESET }
    }
}
