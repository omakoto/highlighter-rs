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

fn get_app<'a, 'b>() -> App<'a, 'b> {
    App::new("Hilighter")
        .version("0.1")
        .author("Makoto Onuki <makoto.onuki@gmail.com>")
        .about("Regex based text highlighter")
        .arg(Arg::with_name("rulefile")
            .short("r")
            .long("rule")
            .takes_value(true)
            .required_unless("bashcomp")
            .help("Specify rule file"))
        .arg(Arg::with_name("autoflush")
            .short("f")
            .long("auto-flush")
            .help("Auto-flush stdout"))
        .arg(Arg::with_name("multithread")
            .short("m")
            .long("multi-thread")
            .help("Utilize multi-cores"))
        .arg(Arg::with_name("bashcomp")
            .long("bash-completion")
            .help("Print bash completion script"))
        .arg(Arg::with_name("width")
            .short("w")
            .long("width")
            .default_value("80")
            .min_values(1)
            .takes_value(true)
            .help("Set width for pre/post lines"))
        .arg(Arg::with_name("files")
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
    if matches.is_present("bashcomp") {
        get_app().gen_completions_to("hl", Shell::Bash, &mut io::stdout());
        std::process::exit(0);
    }

    let args: Vec<String> = env::args().collect();
    let program = &args[0];

    let width = value_t!(matches, "width", usize).unwrap();
    let auto_flush = matches.is_present("autoflush");
    let rule_file = matches.value_of("rulefile").unwrap();
    let mut files: Vec<String> = vec![];
    if let Some(arg_files) = matches.values_of("files") {
        for f in arg_files {
            files.push(f.to_string());
        }
    }

    // Detect the terminal
    let term = Term::detect();
    debug!("Detected terminal: {:?}", &term);

    let mut parser = RuleParser::new(term, width);
    let rules = match parser.parse(&rule_file.to_string()) { // TODO Fix the arg type
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
