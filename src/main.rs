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

use fileinput::FileInput;

use highlighter::*;
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

const FLAG_AUTO_FLUSH: &'static str = "auto-flush ";
const FLAG_BASHCOMP: &'static str = "bash-completion ";
const FLAG_PATTERN: &'static str = "pattern ";
const FLAG_RULEFILE: &'static str = "rulefile ";
const FLAG_WIDTH: &'static str = "width ";
const FLAG_FILES: &'static str = "files ";

fn get_app<'a, 'b>() -> App<'a, 'b> {
    App::new("Hilighter")
        .version("0.1")
        .author("Makoto Onuki <makoto.onuki@gmail.com>")
        .about("Regex based text highlighter")
        .arg(Arg::with_name(FLAG_PATTERN)
            .short("p")
            .long(FLAG_PATTERN)
            .takes_value(true)
            .multiple(true)
            .help("Specify [pattern]=[color]"))
        .arg(Arg::with_name(FLAG_RULEFILE)
            .short("r")
            .long("rule")
            .takes_value(true)
            .required_unless(FLAG_PATTERN)
            .required_unless(FLAG_BASHCOMP)
            .help("Specify rule file"))
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

// fn run_multi_threaded<T: Read+Sync>(reader: BufReader<T>, filter: Filter, writer: &Fn(&str)) {
//     let (r_tx, r_rx) = sync_channel(1);
//     let (w_tx, w_rx) = sync_channel(1);

//     // let mr = Mutex::new(reader);
//     // let mf = Mutex::new(filter);

//     let r = thread::spawn(|| {
//         for line in reader.lines() {
//             if let Ok(l) = line {
//                 if r_tx.send(l).is_err() {
//                     return;
//                 }
//             }
//         }
//     });
//     let f = thread::spawn(|| {
//         loop {
//             match r_rx.recv() {
//                 Ok(l) => filter.process(&l, |l| {
//                     if w_tx.send(l).is_err() {
//                         panic!();
//                     }
//                 }),
//                 _ => return,
//             }
//         }
//     });
//     loop {
//         match w_rx.recv() {
//             Ok(l) => writer(l),
//             _ => return,
//         }
//     }
//     r.join().unwrap();
//     f.join().unwrap();
// }

fn main() {
    env_logger::init().unwrap();

    let matches = get_app().get_matches();
    if matches.is_present(FLAG_BASHCOMP) {
        get_app().gen_completions_to("hl", Shell::Bash, &mut io::stdout());
        std::process::exit(0);
    }

    let args: Vec<String> = env::args().collect();
    let program = &args[0];

    let width = value_t!(matches, FLAG_WIDTH, usize).unwrap();
    let auto_flush = matches.is_present("autoflush");
    let mut files: Vec<String> = vec![];
    if let Some(arg) = matches.values_of(FLAG_FILES) {
        for f in arg {
            files.push(f.to_string());
        }
    }
    let mut patterns: Vec<String> = vec![];
    if let Some(arg) = matches.values_of(FLAG_PATTERN) {
        for f in arg {
            patterns.push(f.to_string());
        }
    }

    // Detect the terminal
    let term = Term::detect();
    debug!("Detected terminal: {:?}", &term);

    let mut parser = RuleParser::new(term, width);
    let parse_result = if patterns.len() > 0 {
        parser.parse_from_args(&patterns)
    } else {
        parser.parse(&(matches.value_of(FLAG_RULEFILE).unwrap()).to_string())
    };
    let rules = match parse_result {
        Err(e) => {
            error(&format!("{}", e));
            std::process::exit(1);
        }
        Ok(v) => v,
    };

    // Create the filter
    let mut filter = Filter::new(term, rules);

    // This works.
    let fileinput = FileInput::new(&files);
    let reader = BufReader::new(fileinput);

    run_single_threaded(reader, &mut filter, &move |out| {
        println!("{}", out);
        if auto_flush {
            io::stdout().flush();
        }
    });
}
