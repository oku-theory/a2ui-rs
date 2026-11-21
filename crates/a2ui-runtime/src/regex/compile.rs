//! Compiles a pattern's tree into a flat instruction list for the matching machine.
//!
//! Counted repeats expand inline, so a program's states are (instruction, position) pairs
//! with no counter. The tree is walked on an explicit stack of tasks, so its depth costs heap,
//! not stack. Code is written straight into the program: a repeat writes its body once and then
//! copies that range, its targets moved; a jump whose target comes later is written once that
//! target is known. A program past [`MAX_PROGRAM`] instructions stops the compilation as soon as
//! it gets there, so the work stays bounded however large the counts.

use super::ast::Node;
use super::charset::CharSet;
use super::{MAX_PROGRAM, Rule};
use crate::error::Error;

/// One instruction; jump targets are indices into the program.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Inst {
    /// Consumes this code point.
    Char(char),
    /// Consumes a code point of this set.
    Set(CharSet),
    /// Consumes any code point but a line terminator.
    Any,
    /// Continues at both targets, the preferred one first.
    Split(usize, usize),
    /// Continues at the target.
    Jmp(usize),
    /// Continues at the next instruction when the assertion holds.
    Assert(Assertion),
    /// Continues at `end` when the body at `start..end` matches here, or does not when
    /// `negated`; the body's last instruction is its own [`Inst::Match`].
    Look {
        negated: bool,
        start: usize,
        end: usize,
    },
    /// The program, or the lookahead body, matches.
    Match,
}

/// A zero-width test of the position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Assertion {
    /// `^`.
    Start,
    /// `$`.
    End,
    /// `\b` (`true`) or `\B` (`false`).
    Boundary(bool),
}

/// A compiled pattern: its instructions, starting at 0 and ending in [`Inst::Match`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Program {
    pub(super) insts: Vec<Inst>,
}

/// Compiles `node` to a program of at most [`MAX_PROGRAM`] instructions.
pub(super) fn compile(node: &Node) -> Result<Program, Error> {
    let mut code = Vec::new();
    let mut tasks = vec![Task::Enter(node)];
    while let Some(task) = tasks.pop() {
        run(task, &mut code, &mut tasks)?;
        check(code.len())?;
    }
    code.push(Inst::Match);
    check(code.len())?;
    Ok(Program { insts: code })
}

/// A step of the walk; each carries the positions of the code it completes.
enum Task<'a> {
    /// Writes a node's code.
    Enter(&'a Node),
    /// Writes the remaining branches of an alternation; `jmps` await its end.
    Branches { rest: &'a [Node], jmps: Vec<usize> },
    /// Ends a branch behind the split at `split`.
    BranchEnd {
        split: usize,
        rest: &'a [Node],
        jmps: Vec<usize>,
    },
    /// Ends the lookahead at `at` with the body's own match.
    LookEnd { negated: bool, at: usize },
    /// Completes the repeat whose body was written from `body`, after a split at `start`
    /// when `min` is 0.
    RepeatEnd {
        start: usize,
        body: usize,
        min: u32,
        max: Option<u32>,
        lazy: bool,
    },
}

/// Runs `task`, writing into `code` and pushing the tasks it leaves.
fn run<'a>(task: Task<'a>, code: &mut Vec<Inst>, tasks: &mut Vec<Task<'a>>) -> Result<(), Error> {
    match task {
        Task::Enter(node) => enter(node, code, tasks),
        Task::Branches { rest, jmps } => match rest {
            [] => {
                let end = code.len();
                for jmp in jmps {
                    code[jmp] = Inst::Jmp(end);
                }
            }
            [only] => {
                tasks.push(Task::Branches { rest: &[], jmps });
                tasks.push(Task::Enter(only));
            }
            [first, rest @ ..] => {
                let split = code.len();
                code.push(Inst::Split(split + 1, split + 1));
                tasks.push(Task::BranchEnd { split, rest, jmps });
                tasks.push(Task::Enter(first));
            }
        },
        Task::BranchEnd {
            split,
            rest,
            mut jmps,
        } => {
            jmps.push(code.len());
            code.push(Inst::Jmp(code.len()));
            code[split] = Inst::Split(split + 1, code.len());
            tasks.push(Task::Branches { rest, jmps });
        }
        Task::LookEnd { negated, at } => {
            code.push(Inst::Match);
            code[at] = Inst::Look {
                negated,
                start: at + 1,
                end: code.len(),
            };
        }
        Task::RepeatEnd {
            start,
            body,
            min,
            max,
            lazy,
        } => repeat(code, start, body, min, max, lazy)?,
    }
    Ok(())
}

/// Writes the code of a leaf, or the tasks that write a node with children.
fn enter<'a>(node: &'a Node, code: &mut Vec<Inst>, tasks: &mut Vec<Task<'a>>) {
    let inst = match node {
        Node::Empty => return,
        Node::Char(c) => Inst::Char(*c),
        Node::Any => Inst::Any,
        Node::Class { negated, items } => Inst::Set(if *negated {
            items.negate()
        } else {
            items.clone()
        }),
        Node::Start => Inst::Assert(Assertion::Start),
        Node::End => Inst::Assert(Assertion::End),
        Node::WordBoundary(word) => Inst::Assert(Assertion::Boundary(*word)),
        Node::Group(inner) => return tasks.push(Task::Enter(inner)),
        Node::Concat(items) => return tasks.extend(items.iter().rev().map(Task::Enter)),
        Node::Alt(branches) => {
            return tasks.push(Task::Branches {
                rest: branches,
                jmps: Vec::new(),
            });
        }
        Node::Look { negated, node } => {
            let at = code.len();
            code.push(Inst::Look {
                negated: *negated,
                start: at + 1,
                end: at + 1,
            });
            tasks.push(Task::LookEnd {
                negated: *negated,
                at,
            });
            return tasks.push(Task::Enter(node));
        }
        Node::Repeat {
            node,
            min,
            max,
            lazy,
        } => {
            if *max == Some(0) {
                return;
            }
            let start = code.len();
            if *min == 0 {
                // Its exit is written once the body's length is known.
                code.push(Inst::Split(start + 1, start + 1));
            }
            tasks.push(Task::RepeatEnd {
                start,
                body: code.len(),
                min: *min,
                max: *max,
                lazy: *lazy,
            });
            return tasks.push(Task::Enter(node));
        }
    };
    code.push(inst);
}

/// Completes a repeat whose body, written once at `body..`, is its first copy.
///
/// `{n}` is n copies; `{n,m}` adds m - n copies, each behind a split to the exit; `{n,}` with n
/// at least 1 loops back over its last copy, and `{0,}` jumps back to its leading split. A body
/// of no instruction matches the empty string only, as any repeat of it does.
fn repeat(
    code: &mut Vec<Inst>,
    start: usize,
    body: usize,
    min: u32,
    max: Option<u32>,
    lazy: bool,
) -> Result<(), Error> {
    let len = code.len() - body;
    if len == 0 {
        code.truncate(start);
        return Ok(());
    }
    let split = |enter: usize, exit: usize| {
        if lazy {
            Inst::Split(exit, enter)
        } else {
            Inst::Split(enter, exit)
        }
    };
    if min == 0 {
        if max.is_none() {
            code.push(Inst::Jmp(start));
            code[start] = split(start + 1, code.len());
            return Ok(());
        }
    } else {
        for _ in 1..min {
            copy(code, body, len)?;
        }
    }
    let Some(max) = max else {
        let last = code.len() - len;
        let at = code.len();
        code.push(split(last, at + 1));
        return Ok(());
    };
    let mut splits = if min == 0 { vec![start] } else { Vec::new() };
    for _ in min.max(1)..max {
        splits.push(code.len());
        code.push(Inst::Split(code.len() + 1, code.len() + 1));
        copy(code, body, len)?;
    }
    let exit = code.len();
    for at in splits {
        code[at] = split(at + 1, exit);
    }
    Ok(())
}

/// Appends a copy of `code[from..from + len]`, its targets moved to the copy.
fn copy(code: &mut Vec<Inst>, from: usize, len: usize) -> Result<(), Error> {
    let to = code.len();
    check(to + len)?;
    code.extend_from_within(from..from + len);
    let shift = to - from;
    for inst in &mut code[to..] {
        match inst {
            Inst::Split(a, b) => {
                *a += shift;
                *b += shift;
            }
            Inst::Jmp(target) => *target += shift,
            Inst::Look { start, end, .. } => {
                *start += shift;
                *end += shift;
            }
            Inst::Char(_) | Inst::Set(_) | Inst::Any | Inst::Assert(_) | Inst::Match => {}
        }
    }
    Ok(())
}

/// Refuses a program, or a part of one, longer than [`MAX_PROGRAM`] instructions.
fn check(len: usize) -> Result<(), Error> {
    if len > MAX_PROGRAM {
        return Err(
            Rule::PatternTooComplex.refuse("the compiled pattern is longer than 4096 instructions")
        );
    }
    Ok(())
}

#[cfg(test)]
#[path = "compile_tests.rs"]
mod tests;
