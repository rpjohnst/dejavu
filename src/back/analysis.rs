use std::collections::HashSet;

use entity::{Entity, EntityMap};
use bitvec::BitVec;
use back::ssa;

/// A control flow graph for a function, as the successors and predecessors of each basic block.
pub struct ControlFlow {
    pub succ: EntityMap<ssa::Block, Vec<ssa::Block>>,
    pub pred: EntityMap<ssa::Block, Vec<ssa::Block>>,
}

impl ControlFlow {
    pub fn with_capacity(n: usize) -> Self {
        let succ: EntityMap<_, Vec<_>> = EntityMap::with_capacity(n);
        let pred: EntityMap<_, Vec<_>> = EntityMap::with_capacity(n);

        ControlFlow { succ, pred }
    }

    pub fn insert(&mut self, pred: ssa::Block, succ: ssa::Block) {
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
    pub in_: EntityMap<ssa::Block, HashSet<ssa::Value>>,
    pub out: EntityMap<ssa::Block, HashSet<ssa::Value>>,
}

impl Liveness {
    /// Compute live values at basic block boundaries.
    ///
    /// This algorithm works by propagating live values backwards from their last uses to their
    /// definitions. Each basic block is traversed backwards, and any values newly discovered to
    /// be live at its entry point are marked as live at its predecessors' terminators.
    /// Predecessors with new live values are re-added to a workset to be traversed again.
    pub fn compute(program: &ssa::Function, control_flow: &ControlFlow) -> Liveness {
        let mut in_: EntityMap<_, HashSet<_>> = EntityMap::with_capacity(program.blocks.len());
        let mut out: EntityMap<_, HashSet<_>> = EntityMap::with_capacity(program.blocks.len());

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
                    live.remove(&value);
                    live.extend(program.uses(value));
                }
                for &value in program.blocks[block].arguments.iter() {
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
