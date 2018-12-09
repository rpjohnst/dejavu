use std::collections::HashSet;

use crate::bit_vec::BitVec;
use crate::handle_map::{Handle, HandleMap};
use crate::back::ssa;

/// A control flow graph for a function, as the successors and predecessors of each basic block.
pub struct ControlFlow {
    pub succ: HandleMap<ssa::Label, Vec<ssa::Label>>,
    pub pred: HandleMap<ssa::Label, Vec<ssa::Label>>,
}

impl ControlFlow {
    pub fn with_capacity(n: usize) -> Self {
        let succ: HandleMap<_, Vec<_>> = HandleMap::with_capacity(n);
        let pred: HandleMap<_, Vec<_>> = HandleMap::with_capacity(n);

        ControlFlow { succ, pred }
    }

    pub fn insert(&mut self, pred: ssa::Label, succ: ssa::Label) {
        self.succ.ensure(pred).push(succ);
        self.pred.ensure(succ).push(pred);
    }

    /// Computes the control flow graph of a function.
    ///
    /// A basic block's successors are easily retrieved from its terminating jump or branch. This
    /// function propagates that information to determine each block's predecessors.
    pub fn compute(program: &ssa::Function) -> Self {
        let mut control_flow = Self::with_capacity(program.blocks.len());

        for pred in program.blocks.keys() {
            for &succ in program.successors(pred) {
                control_flow.insert(pred, succ);
            }
        }

        control_flow
    }
}

/// Live value analysis.
///
/// A value is live from the point of its definition until its last use. `Liveness` tracks values
/// that live across basic block boundaries. A block can either be dominated by a live definition,
/// or it can contain a definition that dominates later blocks.
pub struct Liveness {
    pub in_: HandleMap<ssa::Label, HashSet<ssa::Value>>,
    pub out: HandleMap<ssa::Label, HashSet<ssa::Value>>,
}

impl Liveness {
    /// Compute live values at basic block boundaries.
    ///
    /// This algorithm works by propagating live values backwards from their last uses to their
    /// definitions. Each basic block is traversed backwards, and any values newly discovered to
    /// be live at its entry point are marked as live at its predecessors' terminators.
    /// Predecessors with new live values are re-added to a workset to be traversed again.
    pub fn compute(program: &ssa::Function, control_flow: &ControlFlow) -> Liveness {
        let mut in_: HandleMap<_, HashSet<_>> = HandleMap::with_capacity(program.blocks.len());
        let mut out: HandleMap<_, HashSet<_>> = HandleMap::with_capacity(program.blocks.len());

        let mut work = BitVec::new();
        for block in program.blocks.keys() {
            work.set(block.index());
        }

        let mut dirty = true;
        while dirty {
            dirty = false;

            for block in program.blocks.keys().rev() {
                if !work.clear(block.index()) {
                    continue;
                }

                let mut live: HashSet<_> = out[block].clone();
                for &value in program.blocks[block].instructions.iter().rev() {
                    for value in program.defs(value) {
                        live.remove(&value);
                    }
                    live.extend(program.uses(value));
                }
                for &value in program.blocks[block].parameters.iter() {
                    live.remove(&value);
                }
                for &value in in_[block].iter() {
                    live.remove(&value);
                }

                if live.is_empty() {
                    continue;
                }
                in_[block].extend(&live);

                for &pred in &control_flow.pred[block] {
                    let len = out[pred].len();
                    out[pred].extend(&live);
                    if out[pred].len() == len {
                        continue;
                    }

                    dirty = true;
                    work.set(pred.index());
                }
            }
        }

        Liveness { in_, out }
    }
}
