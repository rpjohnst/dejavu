use std::mem;
use std::collections::HashMap;

use bitvec::BitVec;
use entity::{Entity, EntityMap};
use back::{ssa, ControlFlow};

pub struct Builder {
    pub function: ssa::Function,
    pub control_flow: ControlFlow,

    local: u32,

    current_defs: EntityMap<ssa::Block, HashMap<Local, ssa::Value>>,
    current_args: EntityMap<ssa::Block, Vec<(Local, ssa::Value)>>,
    sealed: BitVec,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub struct Local(u32);

/// A utility type for tracking whether block argument values are unique.
enum ZeroOneMany<T> {
    Zero,
    One(T),
    Many,
}

impl Builder {
    pub fn new() -> Self {
        Builder {
            function: ssa::Function::new(),
            control_flow: ControlFlow::with_capacity(0),

            local: 0,

            current_defs: EntityMap::new(),
            current_args: EntityMap::new(),
            sealed: BitVec::new(),
        }
    }

    pub fn insert_edge(&mut self, pred: ssa::Block, succ: ssa::Block) {
        self.control_flow.insert(pred, succ)
    }

    pub fn emit_local(&mut self) -> Local {
        let local = Local(self.local);
        self.local += 1;
        local
    }

    pub fn read_local(&mut self, block: ssa::Block, local: Local) -> ssa::Value {
        // TODO: factor out `ensure` call with NLL
        if let Some(&def) = self.current_defs.ensure(block).get(&local) {
            return def;
        }

        self.control_flow.pred.ensure(block);
        let pred_len = self.control_flow.pred[block].len();

        let value;
        if !self.sealed.get(block.index()) {
            value = self.function.emit_argument(block);

            let args = self.current_args.ensure(block);
            args.push((local, value));
        } else if pred_len == 0 {
            value = ssa::Value::new(0);
        } else if pred_len == 1 {
            let pred = self.control_flow.pred[block][0];
            value = self.read_local(pred, local);
        } else {
            let argument = self.function.emit_argument(block);
            self.write_local(block, local, argument);
            value = self.read_predecessors(block, local, argument);
        }

        self.write_local(block, local, value);
        value
    }

    pub fn write_local(&mut self, block: ssa::Block, local: Local, value: ssa::Value) {
        let defs = self.current_defs.ensure(block);
        defs.insert(local, value);
    }

    pub fn read_predecessors(
        &mut self, block: ssa::Block, local: Local, argument: ssa::Value
    ) -> ssa::Value {
        use self::ZeroOneMany::*;

        self.control_flow.pred.ensure(block);
        let pred_len = self.control_flow.pred[block].len();

        let mut arguments = Vec::with_capacity(pred_len);
        let mut unique = Zero;
        for i in 0..pred_len {
            let pred = self.control_flow.pred[block][i];
            let value = self.read_local(pred, local);

            unique = match unique {
                Zero if value == argument => Zero,
                Zero => One(value),

                One(unique) if value == argument || value == unique => One(unique),
                One(_) => Many,

                Many => Many,
            };
            arguments.push((pred, value));
        }

        match unique {
            Zero => {
                let args = &mut self.function.blocks[block].arguments;
                let index = args.iter().position(|&arg| arg == argument)
                    .expect("corrupt function");
                args.remove(index);

                // backend codegen will emit an error on use of this value
                self.function.values[argument] = ssa::Inst::Undef;
                argument
            }

            One(unique) => {
                let args = &mut self.function.blocks[block].arguments;
                let index = args.iter().position(|&arg| arg == argument)
                    .expect("corrupt function");
                args.remove(index);

                // pre-existing uses of this argument will be updated after SSA is complete
                self.function.values[argument] = ssa::Inst::Alias(unique);
                unique
            }

            Many => {
                for (pred, value) in arguments {
                    let jump = self.function.terminator(pred);
                    match self.function.values[jump] {
                        ssa::Inst::Jump { ref mut args, .. } => {
                            args.push(value);
                        }

                        ssa::Inst::Branch {
                            targets: [true_block, false_block],
                            arg_lens: [ref mut true_args, ref mut false_args],
                            ref mut args
                        } => {
                            if block == true_block {
                                args.insert(1 + *true_args, value);
                                *true_args += 1;
                            }
                            if block == false_block {
                                args.insert(1 + *true_args + *false_args, value);
                                *false_args += 1;
                            }
                        }

                        _ => unreachable!("corrupt function")
                    }
                }

                argument
            }
        }
    }

    pub fn seal_block(&mut self, block: ssa::Block) {
        // TODO: factor out `ensure` call with NLL
        let arguments = mem::replace(self.current_args.ensure(block), Vec::default());
        for (local, argument) in arguments {
            self.read_predecessors(block, local, argument);
        }
        self.sealed.set(block.index());
    }

    pub fn finish(mut self) -> ssa::Function {
        // resolve any aliases left during SSA construction
        for block in self.function.blocks.keys() {
            for &value in &self.function.blocks[block].instructions {
                Self::replace_aliases(&mut self.function.values, value);
            }
        }

        self.function
    }

    fn replace_aliases(values: &mut EntityMap<ssa::Value, ssa::Inst>, value: ssa::Value) {
        // TODO: Cretonne doesn't run afoul of the borrow checker here because it happens to store
        // values and instructions separately. This also enables multi-value instructions, and
        // keeps aliases out of the instruction store.
        //
        // Is this worth the extra indirection?
        for i in 0..values[value].arguments().len() {
            let arg = values[value].arguments()[i];
            let resolved = Self::resolve_alias(values, arg);
            if resolved != arg {
                values[value].arguments_mut()[i] = resolved;
            }
        }
    }

    fn resolve_alias(values: &EntityMap<ssa::Value, ssa::Inst>, value: ssa::Value) -> ssa::Value {
        let mut v = value;
        let mut i = values.len();
        while let ssa::Inst::Alias(original) = values[v] {
            v = original;

            i -= 1;
            if i == 0 {
                panic!("alias loop")
            }
        }
        v
    }
}
