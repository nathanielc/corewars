use env_logger;

use anyhow::{anyhow, Result};
use corewars::{
    core::{Core, CoreConfig, WarriorID},
    load_file::Warrior,
    parser,
};
use log::debug;
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
        /// The number of rounds to battle. Defaults to 100.
        #[structopt(long, short = "r")]
        rounds: Option<i32>,

        /// The size of the core. Defaults to 8,000.
        #[structopt(long, short = "s")]
        core_size: Option<i32>,

        /// The maximum number of cycles to run. Defaults to 80,000.
        #[structopt(long, short = "c")]
        max_cycles: Option<i32>,

        /// The maximum number of processes. Defaults to 8,000.
        #[structopt(long, short = "p")]
        max_processes: Option<i32>,

        /// The maximum size of a warrior. Defaults to 100.
        #[structopt(long, short = "l")]
        max_warrior_length: Option<i32>,

        /// The minimum separation distance. Defaults to 100.
        #[structopt(long, short = "d")]
        min_distance: Option<i32>,

        /// The size of the P space. Defaults to 500.
        #[structopt(long, short = "S")]
        p_space: Option<i32>,

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
            rounds,
            core_size,
            max_cycles,
            max_processes,
            max_warrior_length,
            min_distance,
            p_space,
            warrior,
        } => {
            let warriors = warrior
                .iter()
                .map(|path| parse_warrior(path.as_path()))
                .collect::<Result<Vec<Warrior>>>()?;

            let mut config = CoreConfig::default();
            if let Some(core_size) = core_size {
                config.core_size = core_size;
            }
            if let Some(max_cycles) = max_cycles {
                config.max_cycles = max_cycles;
            }
            if let Some(max_processes) = max_processes {
                config.max_processes = max_processes;
            }
            if let Some(max_warrior_length) = max_warrior_length {
                config.max_warrior_length = max_warrior_length;
            }
            if let Some(min_distance) = min_distance {
                config.min_distance = min_distance;
            }
            if let Some(p_space) = p_space {
                config.p_space = p_space;
            }

            let mut scores: HashMap<WarriorID, (i32, i32, i32)> =
                HashMap::with_capacity(warriors.len());
            let mut warrior_names = HashMap::with_capacity(warriors.len());

            let rounds = rounds.unwrap_or(100);
            for _ in 0..rounds {
                let mut core = Core::new(config.clone());
                core.load_warriors(&warriors)?;
                for (id, warrior) in warriors.iter().enumerate() {
                    warrior_names.insert(id, warrior.metadata.name.to_owned());
                }
                let results = core.run();
                debug!("Battle Results after {} steps:", core.steps_taken());
                for (id, r) in results {
                    let score = scores.entry(id).or_insert((0, 0, 0));
                    match r {
                        corewars::core::BattleResult::Win => score.0 += 1,
                        corewars::core::BattleResult::Loss(_) => score.1 += 1,
                        corewars::core::BattleResult::Tie => score.2 += 1,
                    };
                    debug!(
                        "{}: {}",
                        warrior_names[&id]
                            .as_ref()
                            .map_or_else(|| format!("{}", id), |n| n.to_string()),
                        r
                    );
                }
            }
            for (id, (win, loss, tie)) in scores {
                println!(
                    "{}: {} {} {}",
                    warrior_names[&id]
                        .as_ref()
                        .map_or_else(|| format!("{}", id), |n| n.to_string()),
                    win,
                    loss,
                    tie
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
