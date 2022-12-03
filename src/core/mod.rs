//! A [`Core`](Core) is a block of "memory" in which Redcode programs reside.
//! This is where all simulation of a Core Wars battle takes place.

use log::trace;
use std::{collections::HashMap, convert::TryInto};
use std::{collections::LinkedList, fmt};
use std::{
    fmt::Display,
    ops::{Index, Range},
};

use thiserror::Error as ThisError;

use crate::load_file::{Instruction, Offset, Warrior};

mod address;
mod modifier;
mod opcode;
mod process;

const DEFAULT_MAX_CYCLES: i32 = 10_000;
const DEFAULT_CORE_SIZE: i32 = 8_000;
const DEFAULT_MAX_PROCESSES: i32 = 10_000;
const DEFAULT_MAX_WARRIOR_LENGTH: i32 = 100;
const DEFAULT_MIN_DISTANCE: i32 = 1000;

/// An error occurred during loading or core creation
#[derive(ThisError, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// The warrior was longer than allowed
    #[error("warrior has too many instructions")]
    WarriorTooLong,

    /// The specified core size was larger than the allowed max
    #[error("cannot create a core with size {0}; must be less than {}", u32::MAX)]
    InvalidCoreSize(u32),

    #[error(transparent)]
    WarriorAlreadyLoaded(#[from] process::Error),
}

/// The full memory core at a given point in time
pub struct Core {
    config: CoreConfig,
    instructions: Box<[Instruction]>,
    process_queue: process::Queue,
    steps_taken: i32,
    log: LinkedList<Vec<Instruction>>,
    warriors: Vec<WarriorID>,
}

pub struct CoreConfig {
    pub core_size: i32,
    pub max_cycles: i32,
    pub max_processes: i32,
    pub max_warrior_length: i32,
    pub min_distance: i32,
}
impl Default for CoreConfig {
    fn default() -> Self {
        Self {
            core_size: DEFAULT_CORE_SIZE,
            max_cycles: DEFAULT_MAX_CYCLES,
            max_processes: DEFAULT_MAX_PROCESSES,
            max_warrior_length: DEFAULT_MAX_WARRIOR_LENGTH,
            min_distance: DEFAULT_MIN_DISTANCE,
        }
    }
}

#[derive(PartialEq, Debug)]
pub enum BattleResult {
    Win,
    Loss(process::Error),
    Tie,
}

impl Display for BattleResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BattleResult::Win => write!(f, "Win"),
            BattleResult::Loss(e) => write!(f, "Loss, {}", e),
            BattleResult::Tie => write!(f, "Tie"),
        }
    }
}

type WarriorID = usize;

enum StepResult {
    Continue(WarriorID, Option<process::Error>),
    Halt,
}

impl Core {
    /// Create a new Core with the given number of possible instructions.
    pub fn new(config: CoreConfig) -> Self {
        Self {
            instructions: vec![Instruction::default(); config.core_size as usize]
                .into_boxed_slice(),
            config,
            process_queue: process::Queue::new(),
            steps_taken: 0,
            log: LinkedList::new(),
            warriors: Vec::new(),
        }
    }

    #[must_use]
    pub fn steps_taken(&self) -> i32 {
        self.steps_taken
    }

    #[cfg(test)]
    fn program_counter(&self) -> Offset {
        self.process_queue
            .peek()
            .expect("process queue was empty")
            .offset
    }

    fn offset<T: Into<i32>>(&self, value: T) -> Offset {
        Offset::new(value.into(), self.len())
    }

    /// Get the number of instructions in the core (available to programs
    /// via the `CORESIZE` label)
    #[must_use]
    pub fn len(&self) -> u32 {
        self.instructions
            .len()
            .try_into()
            .expect("Core has > u32::MAX instructions")
    }

    /// Whether the core is empty or not (almost always `false`)
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.instructions.is_empty()
    }

    /// Get an instruction from a given index in the core
    #[must_use]
    pub fn get(&self, index: i32) -> &Instruction {
        self.get_offset(self.offset(index))
    }

    /// Get an instruction from a given offset in the core
    fn get_offset(&self, offset: Offset) -> &Instruction {
        &self.instructions[offset.value() as usize]
    }

    /// Get a mutable instruction from a given index in the core
    pub fn get_mut(&mut self, index: i32) -> &mut Instruction {
        self.get_offset_mut(self.offset(index))
    }

    /// Get a mutable from a given offset in the core
    fn get_offset_mut(&mut self, offset: Offset) -> &mut Instruction {
        &mut self.instructions[offset.value() as usize]
    }

    /// Write an instruction at a given index into the core
    #[cfg(test)]
    fn set(&mut self, index: i32, value: Instruction) {
        self.set_offset(self.offset(index), value);
    }

    /// Write an instruction at a given offset into the core
    #[cfg(test)]
    fn set_offset(&mut self, index: Offset, value: Instruction) {
        self.instructions[index.value() as usize] = value;
    }

    /// Load a [`Warrior`](Warrior) into the core starting at the front (first instruction of the core).
    /// Returns an error if the Warrior was too long to fit in the core, or had unresolved labels
    pub fn load_warrior(&mut self, warrior: &Warrior) -> Result<WarriorID, Error> {
        if warrior.len() > self.config.max_warrior_length {
            return Err(Error::WarriorTooLong);
        }

        let warrior_id = self.warriors.len() as WarriorID;
        self.warriors.push(warrior_id);

        // TODO check that all instructions are fully resolved? Or require a type
        // safe way of loading a resolved warrior perhaps

        for (i, instruction) in warrior.program.instructions.iter().enumerate() {
            self.instructions[i] = self.normalize(instruction.clone());
        }

        let origin: i32 = warrior
            .program
            .origin
            .unwrap_or(0)
            .try_into()
            .expect(format!("Warrior {:?} has invalid origin", warrior_id).as_str());

        self.process_queue
            .push(warrior_id, self.offset(origin), None);

        Ok(warrior_id)
    }

    fn normalize(&self, mut instruction: Instruction) -> Instruction {
        // NOTE: this works, but it's a bit unforgiving in terms of debugging since
        // we lose information in the process. Similarly, during parsing we lose
        // expression expansion etc.

        // Maybe it would be better to just normalize all values during execution
        // instead of during warrior loading...

        instruction
            .a_field
            .set_value(self.offset(instruction.a_field.unwrap_value()));

        instruction
            .b_field
            .set_value(self.offset(instruction.b_field.unwrap_value()));

        instruction
    }

    // Run a single cycle of simulation.
    fn step(&mut self) -> StepResult {
        self.log.push_back(self.instructions.to_vec());
        let current_process = match self.process_queue.pop() {
            Ok(cp) => cp,
            Err(_err) => return StepResult::Halt,
        };

        trace!(
            "Step{:>6} (t{:>2}): {:0>5} {}",
            self.steps_taken,
            current_process.thread,
            current_process.offset.value(),
            self.get_offset(current_process.offset),
        );
        self.steps_taken += 1;

        let result = opcode::execute(self, current_process.offset);

        match result {
            Err(err) => match err {
                process::Error::DivideByZero | process::Error::ExecuteDat(_) => {
                    if self.process_queue.thread_count(current_process.id) < 1 {
                        StepResult::Continue(current_process.id, Some(err))
                    } else {
                        // This is fine, the task terminated but the process is still alive
                        StepResult::Continue(current_process.id, None)
                    }
                }
                _ => panic!("Unexpected error {}", err),
            },
            Ok(result) => {
                // In the special case of a split, enqueue PC+1 (with same thread id)
                // before also enqueueing the other offset (new thread id)
                let new_thread_id = if result.should_split {
                    self.process_queue.push(
                        current_process.id,
                        current_process.offset + 1,
                        Some(current_process.thread),
                    );
                    None
                } else {
                    Some(current_process.thread)
                };

                // Either the opcode changed the program counter, or we should just enqueue PC+1
                let offset = result
                    .program_counter_offset
                    .unwrap_or_else(|| self.offset(1));

                self.process_queue.push(
                    current_process.id,
                    current_process.offset + offset,
                    new_thread_id,
                );

                StepResult::Continue(current_process.id, None)
            }
        }
    }

    /// Run a core to completion. Reports what happened to each warrior.
    pub fn run(&mut self) -> HashMap<WarriorID, BattleResult> {
        let mut results: HashMap<WarriorID, BattleResult> =
            HashMap::with_capacity(self.warriors.len());

        while self.steps_taken < self.config.max_cycles {
            match self.step() {
                StepResult::Continue(id, err) => {
                    if let Some(err) = err {
                        results.insert(id, BattleResult::Loss(err));
                    }
                }
                StepResult::Halt => break,
            }

            // If we have more that one warrior battling and a single survivor then stop
            if self.warriors.len() > 1 {
                let survivor_count = self.warriors.len()
                    - results
                        .iter()
                        .filter(|(_id, r)| matches!(r, BattleResult::Loss(_)))
                        .count();
                if survivor_count <= 1 {
                    break;
                }
            }
        }
        let survivor_count = self.warriors.len()
            - results
                .iter()
                .filter(|(_id, r)| matches!(r, BattleResult::Loss(_)))
                .count();
        if survivor_count > 1 {
            // Insert the winners, which all tied.
            for id in self.warriors.iter() {
                if results.get(&id).is_none() {
                    results.insert(*id, BattleResult::Tie);
                }
            }
        } else {
            // Insert the winner, which won.
            for id in self.warriors.iter() {
                if results.get(&id).is_none() {
                    results.insert(*id, BattleResult::Win);
                }
            }
        }
        // Return results mapped by name instead of id
        results
    }

    // TODO: clean up this impl a bunch
    fn format_lines<F: Fn(usize, &Instruction) -> String, G: Fn(usize, &Instruction) -> String>(
        &self,
        formatter: &mut fmt::Formatter,
        instruction_prefix: F,
        instruction_suffix: G,
    ) -> fmt::Result {
        let mut lines = Vec::new();
        let mut iter = self.instructions.iter().enumerate().peekable();

        while let Some((i, instruction)) = iter.next() {
            let add_line = |line_vec: &mut Vec<String>, j| {
                line_vec.push(
                    instruction_prefix(j, instruction)
                        + &instruction.to_string()
                        + &instruction_suffix(j, instruction),
                );
            };

            if *instruction == Instruction::default() {
                // Skip large chunks of defaulted instructions with a counter instead
                let mut skipped_count = 0;
                while let Some(&(_, inst)) = iter.peek() {
                    if inst != &Instruction::default() {
                        break;
                    }
                    skipped_count += 1;
                    iter.next();
                }

                if skipped_count > 5 {
                    add_line(&mut lines, i);
                    lines.push(format!("; {:<6}({} more)", "...", skipped_count - 2));
                    add_line(&mut lines, i + skipped_count);
                } else {
                    for _ in 0..skipped_count {
                        add_line(&mut lines, i);
                    }
                }
            } else {
                add_line(&mut lines, i);
            }
        }

        write!(formatter, "{}", lines.join("\n"))
    }
}

impl Default for Core {
    fn default() -> Self {
        Self::new(CoreConfig::default())
    }
}

impl fmt::Debug for Core {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        self.format_lines(
            formatter,
            |i, _| format!("{:0>6} ", i),
            |i, _| {
                if let Ok(process) = self.process_queue.peek() {
                    let i: u32 = i.try_into().unwrap_or_else(|_| {
                        panic!("Instruction count of warrior {:?} > u32::MAX", &process.id)
                    });

                    if i == process.offset.value() {
                        return format!("{:>8}", "; <= PC");
                    }
                }

                String::new()
            },
        )
    }
}

impl fmt::Display for Core {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        self.format_lines(formatter, |_, _| String::new(), |_, _| String::new())
    }
}

impl Index<Range<usize>> for Core {
    type Output = [Instruction];

    fn index(&self, index: Range<usize>) -> &Self::Output {
        &self.instructions[index]
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use corewars_core::load_file::{Field, Opcode, Program};

    use super::*;

    /// Create a core from a string. Public since it is used by submodules' tests as well
    pub fn build_core(program: &str) -> Core {
        let warrior = corewars_parser::parse(program).expect("Failed to parse warrior");

        let mut core = Core::new(CoreConfig {
            max_cycles: 8000,
            ..CoreConfig::default()
        });
        core.load_warrior(&warrior).expect("Failed to load warrior");
        core
    }

    #[test]
    fn new_core() {
        let mut core = Core::new(CoreConfig {
            max_cycles: 128,
            ..CoreConfig::default()
        });
        assert_eq!(core.len(), 128);
    }

    #[test]
    fn load_program() {
        let mut core = Core::new(CoreConfig {
            max_cycles: 128,
            ..CoreConfig::default()
        });

        let warrior = corewars_parser::parse(
            "
            mov $1, #1
            jmp #-1, #2
            jmp #-1, #2
            ",
        )
        .expect("Failed to parse warrior");

        core.load_warrior(&warrior).expect("Failed to load warrior");
        let expected_core_size = 128_u32;
        assert_eq!(core.len(), expected_core_size);

        let jmp_target = (expected_core_size - 1).try_into().unwrap();

        assert_eq!(
            &core.instructions[..4],
            &[
                Instruction::new(Opcode::Mov, Field::direct(1), Field::immediate(1)),
                Instruction::new(
                    Opcode::Jmp,
                    Field::immediate(jmp_target),
                    Field::immediate(2)
                ),
                Instruction::new(
                    Opcode::Jmp,
                    Field::immediate(jmp_target),
                    Field::immediate(2)
                ),
                Instruction::default(),
            ]
        );
    }

    #[test]
    fn load_program_too_long() {
        let mut core = Core::new(CoreConfig {
            max_cycles: 128,
            ..CoreConfig::default()
        });
        let warrior = Warrior {
            program: Program {
                instructions: vec![
                    Instruction::new(Opcode::Dat, Field::direct(1), Field::direct(1),);
                    255
                ],
                origin: None,
            },
            ..Warrior::default()
        };

        core.load_warrior(&warrior)
            .expect_err("Should have failed to load warrior: too long");

        assert_eq!(core.len(), 128);
    }

    #[test]
    fn wrap_program_counter_on_overflow() {
        let mut core = build_core("mov $0, $1");

        for i in 0..core.len() {
            assert_eq!(core.program_counter().value(), i);
            core.step();
        }

        assert_eq!(core.program_counter().value(), 0);
        core.step();
        assert_eq!(core.program_counter().value(), 1);
    }
}
