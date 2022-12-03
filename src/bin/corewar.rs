use env_logger;

use anyhow::{anyhow, Result};
use corewars::{
    core::{Core, CoreConfig},
    load_file::Warrior,
    parser,
};
use std::{
    collections::HashMap,
    fs,
    io::{self, Read},
    path::{Path, PathBuf},
};

use lazy_static::lazy_static;
use structopt::StructOpt;

lazy_static! {
    static ref IO_SENTINEL: PathBuf = PathBuf::from("-");
}
fn main() {
    env_logger::init();
    std::process::exit(
        // TODO use exitcode lib or something like that
        if let Err(err) = run() {
            eprintln!("Error: {}", err);
            -1
        } else {
            // TODO use exit codes for warnings?
            0
        },
    )
}

#[derive(Debug, StructOpt)]
#[structopt(rename_all = "kebab")]
/// Parse, assemble, and save Redcode files
struct Options {
    /// The corewars subcommand to perform
    #[structopt(subcommand)]
    command: Command,

    /// Print additional details while running
    // TODO(#26) hook this up to a log level
    #[structopt(long, short)]
    verbose: bool,
}

#[derive(Debug, StructOpt)]
enum Command {
    /// Save/print a program in "load file" format
    #[structopt(name = "dump")]
    Dump {
        /// Output file; defaults to stdout ("-")
        #[structopt(long, short, parse(from_os_str), default_value = IO_SENTINEL.to_str().unwrap())]
        output_file: PathBuf,

        /// Whether labels, expressions, macros, etc. should be resolved and
        /// expanded in the output
        #[structopt(long, short = "E")]
        no_expand: bool,

        /// Input files; use "-" to read from stdin
        #[structopt(long, short, parse(from_os_str))]
        input_file: Vec<PathBuf>,
    },

    /// Run a warrior to completion
    #[structopt(name = "run")]
    Run {
        /// The max number of cycles to run. Defaults to
        #[structopt(long, short)]
        max_cycles: Option<i32>,

        /// Input files; use "-" to read from stdin
        #[structopt(long, short, parse(from_os_str))]
        input_file: Vec<PathBuf>,
    },
}

pub fn run() -> Result<()> {
    let options = Options::from_args();

    match options.command {
        Command::Dump {
            output_file,
            no_expand,
            input_file,
        } => {
            if no_expand {
                unimplemented!()
            }
            let warriors = input_file
                .iter()
                .map(|path| parse_warrior(path.as_path()))
                .collect::<Result<Vec<Warrior>>>()?;

            if output_file == *IO_SENTINEL {
                for w in warriors {
                    println!("{}", w);
                }
            } else {
                for w in warriors {
                    //TODO use multiple files
                    fs::write(&output_file, format!("{}\n\n", w))?;
                }
            };
        }
        Command::Run {
            max_cycles,
            input_file,
        } => {
            let warriors = input_file
                .iter()
                .map(|path| parse_warrior(path.as_path()))
                .collect::<Result<Vec<Warrior>>>()?;
            let mut core = if let Some(max_cycles) = max_cycles {
                Core::new(CoreConfig {
                    max_cycles,
                    ..CoreConfig::default()
                })
            } else {
                Core::default()
            };
            let mut warrior_names = HashMap::with_capacity(warriors.len());
            for warrior in warriors.iter() {
                let id = core.load_warrior(&warrior)?;
                warrior_names.insert(id, warrior.metadata.name.to_owned());
            }

            let results = core.run();
            println!("Battle Results after {} steps", core.steps_taken());
            for (id, r) in results {
                println!(
                    "{}: {:?}",
                    warrior_names[&id]
                        .as_ref()
                        .map_or_else(|| format!("{}", id), |n| n.to_string()),
                    r
                );
            }

            if options.verbose {
                println!("Core after execution:\n{}", core);
            }
        }
    };

    Ok(())
}
fn parse_warrior(path: &Path) -> Result<Warrior> {
    let mut input = String::new();

    if path == *IO_SENTINEL {
        io::stdin().read_to_string(&mut input)?;
    } else {
        input = fs::read_to_string(path)?;
    }

    match parser::parse(input.as_str()) {
        parser::Result::Ok(warrior, warnings) => {
            print_warnings(&warnings);
            Ok(warrior)
        }
        parser::Result::Err(err, warnings) => {
            print_warnings(&warnings);
            Err(anyhow!("parse failed: {}", err))
        }
    }
}

fn print_warnings(warnings: &[parser::Warning]) {
    for warning in warnings.iter() {
        eprintln!("Warning: {}", warning);
    }
}
