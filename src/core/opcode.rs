//! Opcode-specific logic to run during a simulation step.

use std::cell::Cell;

use crate::load_file::{Offset, Opcode};

use crate::core::modifier;
use crate::core::process;
use crate::core::Core;

#[derive(Debug)]
pub struct Executed {
    pub program_counter_offset: Option<Offset>,
    pub should_split: bool,
}

/// TODO: docstring
// TODO
#[allow(clippy::too_many_lines)]
pub fn execute(core: &mut Core, program_counter: Offset) -> Result<Executed, process::Error> {
    let instruction = core.get_offset(program_counter).clone();
    let opcode = instruction.opcode;

    // These are basically just useful constants that some opcodes need to use
    let zero = core.offset(0);
    let skip_one = core.offset(2);

    let program_counter_offset = Cell::new(None);

    let executor = modifier::Executor::new(core, program_counter);

    // For jumping opcodes, this is the relative offset they will use to make the jump
    let jump_offset = executor.a_ptr() - program_counter;

    // See docs/icws94.txt:1113 for detailed description of each opcode
    match opcode {
        // Process control/miscellaneous opcodes
        Opcode::Dat => {
            return Err(process::Error::ExecuteDat(program_counter));
        }
        Opcode::Mov => executor.run_on_instructions(|a, _b| Some(a), |a, _b| Some(a)),
        Opcode::Nop => {}

        // Infallible arithmetic
        Opcode::Add => executor.run_on_fields(|a, b| Some(a + b)),
        Opcode::Mul => executor.run_on_fields(|a, b| Some(a * b)),
        Opcode::Sub => executor.run_on_fields(|a, b| Some(b - a)),

        // Fallible arithmetic
        Opcode::Div => {
            let mut div_result = Ok(());
            executor.run_on_fields(|a, b| {
                if b.value() == 0 {
                    div_result = Err(process::Error::DivideByZero);
                    None
                } else {
                    Some(a / b)
                }
            });
            div_result?;
        }
        Opcode::Mod => {
            let mut rem_result = Ok(());
            executor.run_on_fields(|a, b| {
                if b.value() == 0 {
                    rem_result = Err(process::Error::DivideByZero);
                    None
                } else {
                    Some(a % b)
                }
            });
            rem_result?;
        }

        // Skipping control flow opcodes
        Opcode::Cmp | Opcode::Seq => {
            program_counter_offset.set(skip_one.into());
            executor.run_on_instructions(
                |a, b| {
                    if a != b {
                        program_counter_offset.set(None);
                    }
                    None
                },
                // For e.g. F and I, all fields must match
                |a, b| {
                    if a != b {
                        program_counter_offset.set(None);
                    }
                    None
                },
            );
        }
        Opcode::Slt => {
            program_counter_offset.set(skip_one.into());
            executor.run_on_fields(|a, b| {
                if a.value() >= b.value() {
                    program_counter_offset.set(None);
                }
                None
            });
        }
        Opcode::Sne => {
            let next_instruction = Some(skip_one);
            executor.run_on_instructions(
                |a, b| {
                    if a != b {
                        program_counter_offset.set(next_instruction);
                    }
                    None
                },
                |a, b| {
                    if a != b {
                        program_counter_offset.set(next_instruction);
                    }
                    None
                },
            );
        }

        // Jumping control flow opcodes
        // These subtract the current program counter since this offset will be added to it later
        Opcode::Djn => executor.run_on_fields(|_a, b| {
            let decremented = b - 1_i32;
            if decremented != zero {
                program_counter_offset.set(jump_offset.into());
            }
            Some(decremented)
        }),
        Opcode::Jmn => executor.run_on_fields(|_a, b| {
            if b != zero {
                program_counter_offset.set(jump_offset.into());
            }
            None
        }),
        Opcode::Jmp | Opcode::Spl => {
            program_counter_offset.set(jump_offset.into());
        }
        Opcode::Jmz => {
            executor.run_on_fields(|_a, b| {
                if b == zero {
                    program_counter_offset.set(jump_offset.into());
                }
                None
            });
        }

        // P-space opcodes
        Opcode::Ldp | Opcode::Stp => unimplemented!(),
    }

    Ok(Executed {
        program_counter_offset: program_counter_offset.get(),
        should_split: opcode == Opcode::Spl,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    use super::super::process::Error;
    use super::super::tests::build_core;

    use crate::load_file::{Field, Instruction, Modifier, Opcode};

    use test_case::test_case;

    mod process {

        use super::*;

        use pretty_assertions::assert_eq;

        #[test]
        fn execute_dat() {
            let mut core = build_core("dat #0, #0");
            let pc = core.offset(0);
            let err = execute(&mut core, pc).unwrap_err();
            assert_eq!(err, Error::ExecuteDat(pc));
        }

        #[test]
        fn execute_dat_with_postincrement() {
            let mut core = build_core("dat >1, >2");
            let pc = core.offset(0);

            let err = execute(&mut core, pc).unwrap_err();

            assert_eq!(err, Error::ExecuteDat(pc));
            assert_eq!(
                &core.instructions[1..=2],
                &[
                    Instruction::new(Opcode::Dat, Field::direct(0), Field::direct(1)),
                    Instruction::new(Opcode::Dat, Field::direct(0), Field::direct(1)),
                ]
            );
        }

        #[test]
        fn execute_mov() {
            let instruction = Instruction {
                opcode: Opcode::Mov,
                modifier: Modifier::I,
                a_field: Field::direct(0),
                b_field: Field::direct(1),
            };
            let mut core = build_core("mov.i $0, $1");
            let pc = core.offset(0);
            let result = execute(&mut core, pc).expect("Failed to execute");
            assert!(result.program_counter_offset.is_none());

            assert_eq!(
                &core.instructions[..4],
                &vec![
                    instruction.clone(),
                    instruction,
                    Instruction::default(),
                    Instruction::default(),
                ][..]
            );
        }

        #[test]
        fn execute_nop() {
            let mut core = build_core("nop #0, #0");
            let pc = core.offset(0);
            let result = execute(&mut core, pc).unwrap();
            assert!(result.program_counter_offset.is_none());
        }
    }

    mod infallible_arithmetic {
        use super::*;

        use super::test_case;
        use pretty_assertions::assert_eq;

        #[test_case("add", 3; "add")]
        #[test_case("sub", 1; "sub")]
        #[test_case("mul", 2; "mul")]
        fn perform_arithmetic(opcode: &str, expected_result: i32) {
            use pretty_assertions::assert_eq;

            let mut core = build_core(&format!(
                "
                {}.a $1, $2
                dat #1, #0
                dat #2, #0
                ",
                opcode
            ));

            let pc = core.offset(0);
            let result = execute(&mut core, pc).unwrap();

            assert!(result.program_counter_offset.is_none());

            assert_eq!(
                *core.get(2),
                Instruction::new(
                    Opcode::Dat,
                    Field::immediate(expected_result),
                    Field::immediate(0)
                )
            );
        }

        #[test]
        fn add_with_predecrement() {
            let mut core = build_core(
                "
                add.f $1, <1
                dat.f $1, $1
            ",
            );

            let pc = core.offset(0);
            let result = execute(&mut core, pc).unwrap();

            assert!(result.program_counter_offset.is_none());

            // The a-operand should be from before the predecrement, but the
            // b-operand should be from after it, resulting in (1+1=2, 1+0=1)
            assert_eq!(
                *core.get(1),
                Instruction::new(Opcode::Dat, Field::direct(2), Field::direct(1))
            );
        }
    }

    mod fallible_arithmetic {
        use super::*;

        use super::test_case;

        use pretty_assertions::assert_eq;

        #[test]
        fn execute_div() {
            let mut core = build_core(
                "
                div $1, $2
                dat #8, #7
                dat #4, #2
                ",
            );
            let pc = core.offset(0);
            let result = execute(&mut core, pc).unwrap();
            assert!(result.program_counter_offset.is_none());

            assert_eq!(
                *core.get(2),
                Instruction::new(Opcode::Dat, Field::immediate(2), Field::immediate(3)),
            );
        }

        #[test_case(
            Instruction::new(Opcode::Dat, Field::direct(0), Field::direct(2)),
            &Instruction::new(Opcode::Dat, Field::direct(0), Field::direct(3))
            ; "a_zero"
        )]
        #[test_case(
            Instruction::new(Opcode::Dat, Field::direct(2), Field::direct(0)),
            &Instruction::new(Opcode::Dat, Field::direct(2), Field::direct(0))
            ; "b_zero"
        )]
        #[test_case(
            Instruction::new(Opcode::Dat, Field::direct(0), Field::direct(0)),
            &Instruction::new(Opcode::Dat, Field::direct(0), Field::direct(0))
            ; "both_zero"
        )]
        fn execute_div_by_zero(divisor: Instruction, result: &Instruction) {
            use pretty_assertions::assert_eq;

            let mut core = build_core(
                "
                div.f   $1, $2
                dat     #4, #6
                ",
            );

            core.set(2, divisor);
            let pc = core.offset(0);
            let err = execute(&mut core, pc).unwrap_err();

            assert_eq!(err, Error::DivideByZero);
            assert_eq!(core.get(2), result);
        }

        #[test]
        fn execute_mod() {
            let mut core = build_core(
                "
                mod $1, $2
                dat #8, #7
                dat #4, #2
                ",
            );
            let pc = core.offset(0);
            let result = execute(&mut core, pc).unwrap();
            assert!(result.program_counter_offset.is_none());

            assert_eq!(
                *core.get(2),
                Instruction::new(Opcode::Dat, Field::immediate(0), Field::immediate(1)),
            );
        }

        #[test_case(
            Instruction::new(Opcode::Dat, Field::direct(0), Field::direct(4)),
            &Instruction::new(Opcode::Dat, Field::direct(0), Field::direct(2))
            ; "a_zero"
        )]
        #[test_case(
            Instruction::new(Opcode::Dat, Field::direct(3), Field::direct(0)),
            &Instruction::new(Opcode::Dat, Field::direct(1), Field::direct(0))
            ; "b_zero"
        )]
        #[test_case(
            Instruction::new(Opcode::Dat, Field::direct(0), Field::direct(0)),
            &Instruction::new(Opcode::Dat, Field::direct(0), Field::direct(0))
            ; "both_zero"
        )]
        fn execute_mod_by_zero(divisor: Instruction, result: &Instruction) {
            use pretty_assertions::assert_eq;

            let mut core = build_core(
                "
                mod.f   $1, $2
                dat     #4, #6
                ",
            );

            core.set(2, divisor);
            let pc = core.offset(0);
            let err = execute(&mut core, pc).unwrap_err();

            assert_eq!(err, Error::DivideByZero);
            assert_eq!(core.get(2), result);
        }
    }

    mod skipping {
        use super::*;

        use super::test_case;
        use pretty_assertions::assert_eq;

        #[test_case(
            "
            cmp.f   $1, $2
            dat     #0, #1
            dat     #0, #1
            ",
            Some(2)
            ; "cmp_equal"
        )]
        #[test_case(
            "
            seq.f   $1, $2
            dat     #0, #1
            dat     #0, #1
            ",
            Some(2)
            ; "seq_equal"
        )]
        #[test_case(
            "
            cmp.f   $1, $2
            dat     #0, #1
            dat     #1, #1
            ",
            None
            ; "cmp_unequal"
        )]
        #[test_case(
            "
            seq.f   $1, $2
            dat     #0, #1
            dat     #2, #0
            ",
            None
            ; "seq_unequal"
        )]
        #[test_case(
            "
            sne.f   $1, $2
            dat     #0, #1
            dat     #0, #1
            ",
            None
            ; "sne_equal"
        )]
        #[test_case(
            "
            sne.f   $1, $2
            dat     #0, #1
            dat     #1, #1
            ",
            Some(2)
            ; "sne_unequal"
        )]
        fn execute_skip_equality(program: &str, expected_offset: Option<i32>) {
            use pretty_assertions::assert_eq;

            let mut core = build_core(program);
            let pc = core.offset(0);
            let expected_offset = expected_offset.map(|o| core.offset(o));
            let result = execute(&mut core, pc).expect("Error executing opcode");

            assert_eq!(result.program_counter_offset, expected_offset);
        }

        #[test_case(
            "
            slt.a   $1, $2
            dat     #2, #0
            dat     #1, #0
            "
            ; "equal"
        )]
        #[test_case(
            "
            slt.a   $1, $2
            dat     #2, #0
            dat     #1, #0
            "
            ; "greater_than"
        )]
        fn execute_slt_no_skip(program: &str) {
            let mut core = build_core(program);
            let pc = core.offset(0);
            let result = execute(&mut core, pc).unwrap();
            assert!(result.program_counter_offset.is_none());
        }

        #[test]
        fn execute_slt_less_than() {
            let mut core = build_core(
                "
                slt.a   $1, $2
                dat     #1, #0
                dat     #2, #0
                ",
            );
            let pc = core.offset(0);
            let result = execute(&mut core, pc).unwrap();
            assert_eq!(result.program_counter_offset, Some(core.offset(2)));
        }
    }

    mod jumping {
        use super::*;
        use pretty_assertions::assert_eq;

        #[test]
        fn execute_djn_no_jump() {
            let mut core = build_core(
                "
                dat #0, #0
                djn.a $2, $1
                dat #1, #1
                nop #0, #0
                ",
            );
            let pc = core.offset(1);
            let result = execute(&mut core, pc).unwrap();

            assert_eq!(result.program_counter_offset, None);
            assert_eq!(
                &core.instructions[1..4],
                &vec![
                    Instruction {
                        opcode: Opcode::Djn,
                        modifier: Modifier::A,
                        a_field: Field::direct(2),
                        b_field: Field::direct(1)
                    },
                    Instruction::new(Opcode::Dat, Field::immediate(0), Field::immediate(1)),
                    Instruction::new(Opcode::Nop, Field::immediate(0), Field::immediate(0)),
                ][..]
            );
        }

        #[test]
        fn execute_djn_with_jump() {
            let mut core = build_core(
                "
                dat #0, #0
                djn.a $2, $1
                dat #3, #1
                nop #0, #0
                ",
            );
            let pc = core.offset(1);
            let result = execute(&mut core, pc).unwrap();

            assert_eq!(result.program_counter_offset, Some(core.offset(2)));
            assert_eq!(
                &core.instructions[1..4],
                &vec![
                    Instruction {
                        opcode: Opcode::Djn,
                        modifier: Modifier::A,
                        a_field: Field::direct(2),
                        b_field: Field::direct(1)
                    },
                    Instruction::new(Opcode::Dat, Field::immediate(2), Field::immediate(1)),
                    Instruction::new(Opcode::Nop, Field::immediate(0), Field::immediate(0)),
                ][..]
            );
        }

        #[test]
        fn execute_jmn_no_jump() {
            let mut core = build_core(
                "
                dat #0, #0
                jmn.a $2, $1
                dat #0, #1
                nop #0, #0
                ",
            );
            let pc = core.offset(1);
            let result = execute(&mut core, pc).unwrap();

            assert_eq!(result.program_counter_offset, None);
        }

        #[test]
        fn execute_jmn_with_jump() {
            let mut core = build_core(
                "
                dat #0, #0
                jmn.a $2, $1
                dat #1, #1
                nop #0, #0
                ",
            );
            let pc = core.offset(1);
            let result = execute(&mut core, pc).unwrap();

            assert_eq!(result.program_counter_offset, Some(core.offset(2)));
        }

        #[test]
        fn execute_jmp() {
            let mut core = build_core(
                "
                dat #0, #0
                jmp $3, #0
                ",
            );
            let pc = core.offset(1);
            let result = execute(&mut core, pc).expect("Failed to execute");

            assert_eq!(result.program_counter_offset, Some(core.offset(3)));
            assert_eq!(
                &core.instructions[1..5],
                &vec![
                    Instruction::new(Opcode::Jmp, Field::direct(3), Field::immediate(0)),
                    Instruction::default(),
                    Instruction::default(),
                    Instruction::default()
                ][..]
            );
        }

        #[test]
        fn execute_spl() {
            let mut core = build_core(
                "
                dat #0, #0
                spl $3, #0
                ",
            );
            let pc = core.offset(1);
            let result = execute(&mut core, pc).expect("Failed to execute");

            assert_eq!(result.program_counter_offset, Some(core.offset(3)));
            assert!(result.should_split);
            assert_eq!(
                &core.instructions[1..5],
                &vec![
                    Instruction::new(Opcode::Spl, Field::direct(3), Field::immediate(0)),
                    Instruction::default(),
                    Instruction::default(),
                    Instruction::default()
                ][..]
            );
        }

        #[test]
        fn execute_jmz_no_jump() {
            let mut core = build_core(
                "
                dat #0, #0
                jmz.a $2, $1
                dat #1, #1
                nop #0, #0
                ",
            );

            let pc = core.offset(1);
            let result = execute(&mut core, pc).unwrap();

            assert_eq!(result.program_counter_offset, None);
        }

        #[test]
        fn execute_jmz_with_jump() {
            let mut core = build_core(
                "
                dat #0, #0
                jmz.a $2, $1
                dat #0, #1
                nop #0, #0
                ",
            );

            let pc = core.offset(1);
            let result = execute(&mut core, pc).unwrap();

            assert_eq!(result.program_counter_offset, Some(core.offset(2)));
        }
    }
}
