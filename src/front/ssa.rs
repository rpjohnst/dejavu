use std::mem;
use std::collections::HashMap;

use handle_map::{Handle, HandleMap};
use bit_vec::BitVec;
use back::{ssa, ControlFlow};

pub struct Builder {
    pub function: ssa::Function,
    pub control_flow: ControlFlow,

    local: u32,

    current_defs: HandleMap<ssa::Label, HashMap<Local, ssa::Value>>,
    current_args: HandleMap<ssa::Label, Vec<(Local, ssa::Value)>>,
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

            current_defs: HandleMap::new(),
            current_args: HandleMap::new(),
            sealed: BitVec::new(),
        }
    }

    pub fn insert_edge(&mut self, pred: ssa::Label, succ: ssa::Label) {
        self.control_flow.insert(pred, succ)
    }

    pub fn emit_local(&mut self) -> Local {
        let local = Local(self.local);
        self.local += 1;
        local
    }

    pub fn read_local(&mut self, block: ssa::Label, local: Local) -> ssa::Value {
        // TODO: factor out `ensure` call with NLL
        if let Some(&def) = self.current_defs.ensure(block).get(&local) {
            return def;
        }

        self.control_flow.pred.ensure(block);
        let pred_len = self.control_flow.pred[block].len();

        let value;
        if !self.sealed.get(block.index()) {
            value = self.function.emit_parameter(block);

            let args = self.current_args.ensure(block);
            args.push((local, value));
        } else if pred_len == 0 {
            value = ssa::Value::new(0);
        } else if pred_len == 1 {
            let pred = self.control_flow.pred[block][0];
            value = self.read_local(pred, local);
        } else {
            let parameter = self.function.emit_parameter(block);
            self.write_local(block, local, parameter);
            value = self.read_predecessors(block, parameter, local);
        }

        self.write_local(block, local, value);
        value
    }

    pub fn write_local(&mut self, block: ssa::Label, local: Local, value: ssa::Value) {
        let defs = self.current_defs.ensure(block);
        defs.insert(local, value);
    }

    pub fn read_predecessors(
        &mut self, block: ssa::Label, parameter: ssa::Value, local: Local
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
                Zero if value == parameter => Zero,
                Zero => One(value),

                One(unique) if value == parameter || value == unique => One(unique),
                One(_) => Many,

                Many => Many,
            };
            arguments.push((pred, value));
        }

        match unique {
            Zero => {
                let parameters = &mut self.function.blocks[block].parameters;
                let param = parameters.pop();
                assert_eq!(param, Some(parameter));

                // this is a garbage value; uninitialized variables are checked elsewhere
                let value = ssa::Constant::Real(0.0);
                self.function.values[parameter] = ssa::Inst::Immediate { value };
                parameter
            }

            One(unique) => {
                let parameters = &mut self.function.blocks[block].parameters;
                let param = parameters.pop();
                assert_eq!(param, Some(parameter));

                // pre-existing uses of this parameter will be updated after SSA is complete
                self.function.values[parameter] = ssa::Inst::Alias(unique);
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
                                args.insert((1 + *true_args) as usize, value);
                                *true_args += 1;
                            }
                            if block == false_block {
                                args.insert((1 + *true_args + *false_args) as usize, value);
                                *false_args += 1;
                            }
                        }

                        _ => unreachable!("corrupt function")
                    }
                }

                parameter
            }
        }
    }

    pub fn seal_block(&mut self, block: ssa::Label) {
        // TODO: factor out `ensure` call with NLL
        let parameters = mem::replace(self.current_args.ensure(block), Vec::default());
        for (local, parameter) in parameters {
            self.read_predecessors(block, parameter, local);
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

    fn replace_aliases(values: &mut HandleMap<ssa::Value, ssa::Inst>, value: ssa::Value) {
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

    fn resolve_alias(values: &HandleMap<ssa::Value, ssa::Inst>, value: ssa::Value) -> ssa::Value {
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
