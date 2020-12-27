use crate::instructions::*;

use std::collections::VecDeque;

fn scan_for_trust_me(
    code: &Code,
    jmp_offsets: &mut VecDeque<usize>,
    before_idx: usize,
    after_idx: &mut usize,
) {
    // record the location of the line after the TrustMe capping the
    // choice instruction sequence to after_idx.
    loop {
        match &code[*after_idx] {
            &Line::Choice(ChoiceInstruction::DefaultRetryMeElse(offset)) |
            &Line::Choice(ChoiceInstruction::RetryMeElse(offset)) |
            &Line::IndexedChoice(IndexedChoiceInstruction::Retry(offset)) => {
                *after_idx += offset;
            }
            &Line::Choice(ChoiceInstruction::DefaultTrustMe) |
            &Line::Choice(ChoiceInstruction::TrustMe) |
            &Line::IndexedChoice(IndexedChoiceInstruction::Trust(..)) => {
                break;
            }
            _ => {
                *after_idx += 1;
            }
        }
    }

    // search the code in the range for JmpBy instructions and record their
    // offsets for future scanning.
    for (idx, instr) in code[before_idx .. *after_idx].iter().enumerate() {
        match instr {
            &Line::Control(ControlInstruction::JmpBy(_, offset, ..)) => {
                jmp_offsets.push_back(before_idx + idx + offset)
            }
            _ => {
            }
        }
    }

    *after_idx += 1;
}

fn capture_next_range(code: &Code, queue: &mut VecDeque<usize>, last_idx: &mut usize) {
    loop {
        match &code[*last_idx] {
            &Line::Choice(ChoiceInstruction::TryMeElse(offset)) |
            &Line::IndexedChoice(IndexedChoiceInstruction::Try(offset)) => {
                let before_idx = *last_idx;
                *last_idx += offset;

                scan_for_trust_me(code, queue, before_idx, last_idx);
            }
            &Line::Control(ControlInstruction::JmpBy(_, offset, _, false)) => {
                queue.push_back(*last_idx + offset);
                *last_idx += 1;
            }
            &Line::Control(ControlInstruction::JmpBy(_, offset, _, true)) => {
                queue.push_back(*last_idx + offset);
                break;
            }
            &Line::Control(ControlInstruction::Proceed) |
            &Line::Control(ControlInstruction::CallClause(_, _, _, true, _)) =>
                break,
            _ =>
                *last_idx += 1,
        };
    }
}

/* This function walks the code of a single predicate, supposed to
 * begin in code at the offset p. Each instruction is passed to the
 * walker function.
 */
pub fn walk_code(code: &Code, p: usize, mut walker: impl FnMut(&Line))
{
    let mut queue = VecDeque::from(vec![p]);

    while let Some(first_idx) = queue.pop_front() {
        let mut last_idx = first_idx;

        capture_next_range(code, &mut queue, &mut last_idx);

        for instr in &code[first_idx .. last_idx + 1] {
            walker(instr);
        }
    }
}

/* A function for code walking that might result in modification to
 * the code. Otherwise identical to walk_code.
 */
pub fn walk_code_mut(code: &mut Code, p: usize, mut walker: impl FnMut(&mut Line))
{
    let mut queue = VecDeque::from(vec![p]);

    while let Some(first_idx) = queue.pop_front() {
        let mut last_idx = first_idx;

        capture_next_range(code, &mut queue, &mut last_idx);

        for instr in &mut code[first_idx .. last_idx + 1] {
            walker(instr);
        }
    }
}
