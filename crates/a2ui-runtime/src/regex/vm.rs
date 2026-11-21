//! Runs a compiled program over a value in time linear in program and value.
//!
//! The machine decides, for each position from the end of the value back to its start, which
//! instructions reach their [`Inst::Match`] from there: a consuming instruction does when its
//! code point fits and its successor did one position later, any other when one of its
//! successors does at the same position. A state (instruction, position) is thus settled once,
//! whatever the pattern, so no input can backtrack exponentially and no frame is ever stacked:
//! the work is bounded by the program's length times the value's, plus one.
//!
//! A lookahead's body is one level deeper than the lookahead and settled first at each
//! position, so a negative lookahead reads a finished answer. Instructions only move control
//! within their level; the bodies' own [`Inst::Match`] instructions end them.

use super::charset::CharSet;
use super::compile::{Assertion, Inst, Program};

/// A compiled program with the reverse edges the backward pass follows.
#[derive(Debug, Clone)]
pub(super) struct Machine {
    program: Program,
    /// The code points `.` matches.
    dot: CharSet,
    /// Each instruction's lookahead nesting depth.
    level: Vec<usize>,
    /// The instructions continuing without consuming into each instruction, `preds[i..j]`
    /// for instruction `t` where `i = pred_start[t]` and `j = pred_start[t + 1]`.
    pred_start: Vec<usize>,
    preds: Vec<usize>,
    /// The deepest level, plus one.
    levels: usize,
}

impl Machine {
    /// Prepares `program` for matching.
    pub(super) fn new(program: Program) -> Self {
        let level = levels(&program.insts);
        let levels = level.iter().max().map_or(1, |deepest| deepest + 1);
        let (pred_start, preds) = reverse_edges(&program.insts);
        Self {
            program,
            dot: CharSet::dot(),
            level,
            pred_start,
            preds,
            levels,
        }
    }

    /// The number of instructions.
    pub(super) fn len(&self) -> usize {
        self.program.insts.len()
    }

    /// Whether the program matches anywhere in `value`.
    pub(super) fn is_match(&self, value: &[char]) -> bool {
        self.search(value).0
    }

    /// Whether the program matches anywhere in `value`, and the work it did.
    ///
    /// The work counts every instruction visited at each position and every reverse edge
    /// checked, passing or not. Each instruction has at most two successors and is settled
    /// once a position, so the work is at most 3 × program × (value + 1).
    pub(super) fn search(&self, value: &[char]) -> (bool, usize) {
        let size = self.len();
        let mut next = vec![false; size + 1];
        let mut cur = vec![false; size + 1];
        // What `cur` and `next` set, so that each is cleared in kind.
        let mut marked: Vec<usize> = Vec::new();
        let mut marked_next: Vec<usize> = Vec::new();
        let mut work: Vec<Vec<usize>> = vec![Vec::new(); self.levels];
        let mut steps = 0;
        for pos in (0..=value.len()).rev() {
            std::mem::swap(&mut next, &mut cur);
            std::mem::swap(&mut marked_next, &mut marked);
            for pc in marked.drain(..) {
                cur[pc] = false;
            }
            // `cur` now is empty and `next` holds the answers one position further.
            for (pc, inst) in self.program.insts.iter().enumerate() {
                steps += 1;
                // The code point here, when the successor matches one position on.
                let ahead = value.get(pos).filter(|_| next[pc + 1]);
                let live = match inst {
                    Inst::Match => true,
                    Inst::Char(expected) => ahead == Some(expected),
                    Inst::Set(set) => ahead.is_some_and(|&c| set.contains(c)),
                    Inst::Any => ahead.is_some_and(|&c| self.dot.contains(c)),
                    // Settled below, from the instructions they continue into.
                    Inst::Split(..) | Inst::Jmp(_) | Inst::Assert(_) | Inst::Look { .. } => false,
                };
                if live {
                    cur[pc] = true;
                    marked.push(pc);
                    work[self.level[pc]].push(pc);
                }
            }
            for depth in (0..self.levels).rev() {
                while let Some(target) = work[depth].pop() {
                    let (from, to) = (self.pred_start[target], self.pred_start[target + 1]);
                    for &pred in &self.preds[from..to] {
                        steps += 1;
                        if !cur[pred] && self.passes(pred, value, pos, &cur) {
                            cur[pred] = true;
                            marked.push(pred);
                            work[depth].push(pred);
                        }
                    }
                }
            }
            if cur[0] {
                return (true, steps);
            }
        }
        (false, steps)
    }

    /// Whether the non-consuming instruction at `pc` lets control through at `pos`.
    ///
    /// A lookahead's body is settled in `cur` already, being one level deeper.
    fn passes(&self, pc: usize, value: &[char], pos: usize, cur: &[bool]) -> bool {
        match &self.program.insts[pc] {
            Inst::Split(..) | Inst::Jmp(_) => true,
            Inst::Assert(Assertion::Start) => pos == 0,
            Inst::Assert(Assertion::End) => pos == value.len(),
            Inst::Assert(Assertion::Boundary(expected)) => {
                let before = pos.checked_sub(1).is_some_and(|at| is_word(value, at));
                (before != is_word(value, pos)) == *expected
            }
            Inst::Look { negated, start, .. } => cur[*start] != *negated,
            Inst::Char(_) | Inst::Set(_) | Inst::Any | Inst::Match => false,
        }
    }
}

/// Whether the code point at `at` is an ASCII word character.
fn is_word(value: &[char], at: usize) -> bool {
    value
        .get(at)
        .is_some_and(|c| c.is_ascii_alphanumeric() || *c == '_')
}

/// Each instruction's lookahead depth: a body `start..end` is one deeper than its lookahead.
fn levels(insts: &[Inst]) -> Vec<usize> {
    let mut open: Vec<usize> = Vec::new();
    insts
        .iter()
        .enumerate()
        .map(|(pc, inst)| {
            while open.last().is_some_and(|&end| end <= pc) {
                open.pop();
            }
            let depth = open.len();
            if let Inst::Look { end, .. } = inst {
                open.push(*end);
            }
            depth
        })
        .collect()
}

/// The instructions continuing into each one without consuming, grouped by target.
fn reverse_edges(insts: &[Inst]) -> (Vec<usize>, Vec<usize>) {
    let edges: Vec<(usize, usize)> = insts
        .iter()
        .enumerate()
        .flat_map(|(pc, inst)| {
            let targets = match inst {
                Inst::Split(a, b) => [Some(*a), Some(*b)],
                Inst::Jmp(target) => [Some(*target), None],
                Inst::Assert(_) => [Some(pc + 1), None],
                Inst::Look { end, .. } => [Some(*end), None],
                Inst::Char(_) | Inst::Set(_) | Inst::Any | Inst::Match => [None, None],
            };
            targets
                .into_iter()
                .flatten()
                .map(move |target| (target, pc))
        })
        .collect();
    let mut pred_start = vec![0; insts.len() + 2];
    for &(target, _) in &edges {
        pred_start[target + 2] += 1;
    }
    for index in 2..pred_start.len() {
        pred_start[index] += pred_start[index - 1];
    }
    let mut preds = vec![0; edges.len()];
    for &(target, pc) in &edges {
        preds[pred_start[target + 1]] = pc;
        pred_start[target + 1] += 1;
    }
    pred_start.pop();
    (pred_start, preds)
}

#[cfg(test)]
#[path = "vm_tests.rs"]
mod tests;
