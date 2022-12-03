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

#[derive(Debug, StructOpt)]
#[structopt(rename_all = "kebab")]
/// Parse, assemble, and save Redcode files
struct Options {
    /// The corewars subcommand to perform
    #[structopt(subcommand)]
    command: Command,
}

#[derive(Debug, StructOpt)]
enum Command {
    /// Run a warrior to completion
    Run {
        /// The max number of cycles to run. Defaults to
        #[structopt(long, short)]
        max_cycles: Option<i32>,

        /// Input files; use "-" to read from stdin
        #[structopt(long, short, parse(from_os_str))]
        warrior: Vec<PathBuf>,
    },
}

fn main() -> Result<()> {
    env_logger::init();
    let options = Options::from_args();

    match options.command {
        Command::Run {
            max_cycles,
            warrior,
        } => {
            let warriors = warrior
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
            println!("Battle Results after {} steps:", core.steps_taken());
            for (id, r) in results {
                println!(
                    "{}: {}",
                    warrior_names[&id]
                        .as_ref()
                        .map_or_else(|| format!("{}", id), |n| n.to_string()),
                    r
                );
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
