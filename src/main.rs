#[macro_use]
extern crate log;
extern crate env_logger;
extern crate fileinput;
extern crate highlighter;
#[macro_use]
extern crate clap;

use clap::{Arg, App, SubCommand, Shell};
use std::cmp::max;
use std::io;
use std::io::BufReader;
use std::io::prelude::*;
use std::env;
use std::sync::mpsc::*;
use std::thread;
use std::sync::*;
use std::error::Error;

use fileinput::FileInput;

use highlighter::*;
use highlighter::rule::*;
use highlighter::term_color::*;
use highlighter::rule_parser::*;
use highlighter::filter::*;

fn error(message: &String) {
    writeln!(&mut std::io::stderr(),
             "{}: {}",
             env::args().nth(0).unwrap(),
             message);
}

// fn print_usage(program: &str, opts: &Options) {
//     error(&opts.usage(&format!("Usage: {} CONFIG [Files...]", program)));
// }

const FLAG_AUTO_FLUSH: &'static str = "auto-flush";
const FLAG_BASHCOMP: &'static str = "bash-completion";
const FLAG_SIMPLE_RULE: &'static str = "pattern";
const FLAG_RULEFILE: &'static str = "rulefile";
const FLAG_LEGACY_RULEFILE: &'static str = "legacyfile";
const FLAG_WIDTH: &'static str = "width";
const FLAG_FILES: &'static str = "files";

fn get_app<'a, 'b>() -> App<'a, 'b> {
    App::new("Hilighter")
        .version("0.1")
        .author("Makoto Onuki <makoto.onuki@gmail.com>")
        .about("Regex based text highlighter")
        .arg(Arg::with_name(FLAG_SIMPLE_RULE)
            .short("p")
            .long(FLAG_SIMPLE_RULE)
            .takes_value(true)
            .multiple(true)
            .number_of_values(1)
            .help("Add a simple rule: RE=(colors)(@colors) \
                e.g. '\\d+=500/222@/cyan'"))
        .arg(Arg::with_name(FLAG_RULEFILE)
            .short("r")
            .long(FLAG_RULEFILE)
            .takes_value(true)
            .multiple(true)
            .number_of_values(1)
            .help("Specify TOML rule file"))
        .arg(Arg::with_name(FLAG_LEGACY_RULEFILE)
            .short("c")
            .long(FLAG_LEGACY_RULEFILE)
            .takes_value(true)
            .multiple(true)
            .number_of_values(1)
            .help("Specify legacy file"))
        .arg(Arg::with_name("autoflush")
            .short("f")
            .long("auto-flush")
            .help("Auto-flush stdout"))
        .arg(Arg::with_name(FLAG_BASHCOMP)
            .long("bash-completion")
            .help("Print bash completion script"))
        .arg(Arg::with_name(FLAG_WIDTH)
            .short("w")
            .long(FLAG_WIDTH)
            .default_value("80")
            .min_values(1)
            .takes_value(true)
            .help("Set width for pre/post lines"))
        .arg(Arg::with_name(FLAG_FILES)
            .index(1)
            .required(false)
            .multiple(true)
            .help("Input files"))
}

fn run_single_threaded<T: Read>(reader: BufReader<T>, filter: &mut Filter, writer: &Fn(&str)) {
    for line in reader.lines() {
        match line {
            Err(e) => {
                error(&format!("{}", e));
                match e.kind() {
                    std::io::ErrorKind::InvalidData => continue, // OK
                    _ => return,
                }
            }
            Ok(s) => {
                filter.process(&s, writer);
            }
        }
    }
}

fn real_main() -> Result<(), String> {
    env_logger::init().unwrap();

    let matches = get_app().get_matches();
    if matches.is_present(FLAG_BASHCOMP) {
        get_app().gen_completions_to("hl", Shell::Bash, &mut io::stdout());
        return Ok(());
    }

    let args: Vec<String> = env::args().collect();
    let program = &args[0];

    let width = value_t!(matches, FLAG_WIDTH, usize).unwrap();
    let auto_flush = matches.is_present("autoflush");
    let mut files: Vec<String> = vec![];
    if let Some(args) = matches.values_of(FLAG_FILES) {
        for arg in args {
            files.push(arg.to_string());
        }
    }

    // Detect the terminal
    let term = Term::detect();
    debug!("Detected terminal: {:?}", &term);

    // Parse rules.
    let mut parser = RuleParser::new(term, width);

    let mut rules: Vec<Rule> = vec![];

    if let Some(args) = matches.values_of(FLAG_RULEFILE) {
        for arg in args {
            debug!("Loading TOML rule file {}", arg);
            try!(parser.parse_toml(arg, &mut rules).map_err(|e| e.description().to_string()));
        }
    }
    if let Some(args) = matches.values_of(FLAG_LEGACY_RULEFILE) {
        for arg in args {
            debug!("Loading legacy rule file {}", arg);
            try!(parser.parse_legacy(arg, &mut rules).map_err(|e| e.description().to_string()));
        }
    }
    if let Some(args) = matches.values_of(FLAG_SIMPLE_RULE) {
        for arg in args {
            debug!("Adding simple rule {}", arg);
            rules.push(try!(parser.parse_simple_rule(arg).map_err(|e| e.description().to_string())));
        }
    }

    if rules.len() == 0 {
        return Err("No rules specified.".to_string());
    }

    // Create the filter
    let mut filter = Filter::new(term, rules);

    // This works.
    let fileinput = FileInput::new(&files);
    let reader = BufReader::new(fileinput);

    run_single_threaded(reader,
                        &mut filter,
                        &move |out| {
                            println!("{}", out);
                            if auto_flush {
                                io::stdout().flush();
                            }
                        });
    Ok(())
}

fn main() {
    match real_main() {
        Ok(_) => return, // okay
        Err(err) => {
            error(&err);
            std::process::exit(1);
        }
    }
}
