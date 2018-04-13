use std::{u32, slice, fmt};

use handle_map::{Handle, HandleMap};
use symbol::Symbol;

pub struct Function {
    pub blocks: HandleMap<Block, BlockData>,
    pub values: HandleMap<Value, Inst>,

    pub return_def: Value,
}

pub const ENTRY: Block = Block(0);
pub const EXIT: Block = Block(1);

impl Function {
    pub fn new() -> Self {
        let blocks = HandleMap::new();
        let mut values = HandleMap::new();

        let value = Constant::Real(0.0);
        let return_def = values.push(Inst::Immediate { value });

        let mut function = Function { blocks, values, return_def };

        // entry and exit blocks
        function.make_block();
        function.make_block();

        function
    }

    pub fn terminator(&self, block: Block) -> Value {
        *self.blocks[block].instructions.last()
            .expect("empty block")
    }

    pub fn successors(&self, block: Block) -> &[Block] {
        let value = self.terminator(block);
        match self.values[value] {
            Inst::Jump { ref target, .. } => slice::from_ref(target),
            Inst::Branch { ref targets, .. } => targets,
            Inst::Return { .. } | Inst::ScopeError { .. } => &[],

            _ => panic!("corrupt block"),
        }
    }

    pub fn defs(&self, value: Value) -> Option<Value> {
        use self::Inst::*;
        match self.values[value] {
            Immediate { .. } | Unary { .. } | Binary { .. } |
            Argument | Lookup { .. } |
            Project { .. } |
            LoadScope { .. } |
            Write { .. } |
            LoadField { .. } | LoadFieldDefault { .. } |
            Call { .. } => Some(value),

            Alias(_) |
            DeclareGlobal { .. } |
            // With instruction results are aggregates which can't be register allocated.
            With { .. } |
            StoreScope { .. } |
            Read { .. } |
            ScopeError { .. } |
            StoreField { .. } | StoreIndex { .. } |
            Release { .. } |
            Return { .. } |
            Jump { .. } | Branch { .. } => None,
        }
    }

    pub fn internal_defs(&self, value: Value) -> &[Value] {
        use self::Inst::*;
        match self.values[value] {
            Call { ref parameters, .. } => parameters,
            _ => &[],
        }
    }

    pub fn uses(&self, value: Value) -> &[Value] {
        self.values[value].arguments()
    }

    pub fn emit_instruction(&mut self, block: Block, inst: Inst) -> Value {
        let value = self.values.push(inst);
        self.blocks[block].instructions.push(value);
        value
    }

    pub fn emit_argument(&mut self, block: Block) -> Value {
        let value = self.values.push(Inst::Argument);
        self.blocks[block].arguments.push(value);
        value
    }

    pub fn make_block(&mut self) -> Block {
        let block = BlockData {
            arguments: vec![],
            instructions: vec![],
        };

        self.blocks.push(block)
    }
}

pub struct BlockData {
    pub arguments: Vec<Value>,
    pub instructions: Vec<Value>,
}

/// An SSA instruction.
///
/// Some of these instructions have less-than-ideal field grouping- this is so that all "used
/// values" are stored in contiguous arrays, which enables more uniform interfaces elsewhere.
///
/// TODO: Give instructions types- value, row, iterator, pointer.
///       Recombine some/all shapes; combine some `Binary` variants.
#[derive(PartialEq, Debug)]
pub enum Inst {
    /// A placeholder for a value that has been replaced.
    ///
    /// Aliases must not exist in blocks, and must be removed before codegen. They should also be
    /// removed between or as part of the passes that generate them.
    Alias(Value),

    Immediate { value: Constant },
    Unary { op: Unary, arg: Value },
    Binary { op: Binary, args: [Value; 2] },

    /// A placeholder for an argument to a basic block.
    Argument,
    DeclareGlobal { symbol: Symbol },
    Lookup { symbol: Symbol },

    /// Build an iterator over a scope, producing an aggregate.
    With { arg: Value },
    /// Obtain a scalar value from an aggregate.
    Project { arg: Value, index: u8 },
    LoadScope { scope: f64 },
    StoreScope { scope: f64, arg: Value },

    /// Mark a value as read at this point, error on arg == false.
    Read { symbol: Symbol, arg: Value },
    /// `args` contains `[value, array]`. If array is a scalar, return `value`.
    Write { args: [Value; 2] },
    /// Error that the scope `arg` does not exist.
    ScopeError { arg: Value },

    LoadField { entity: Value, field: Symbol },
    LoadFieldDefault { entity: Value, field: Symbol },

    /// `args` contains `[value, scope]`
    StoreField { args: [Value; 2], field: Symbol },
    /// `args` contains `[value, row, j]`
    StoreIndex { args: [Value; 3] },

    Release { arg: Value },

    Call { symbol: Symbol, prototype: Prototype, args: Vec<Value>, parameters: Vec<Value> },
    Return { arg: Value },

    Jump { target: Block, args: Vec<Value> },
    /// `args` contains `[condition, arg_lens[0].., arg_lens[1]..]`
    Branch { targets: [Block; 2], arg_lens: [u32; 2], args: Vec<Value> },
}

impl Inst {
    pub fn arguments(&self) -> &[Value] {
        use self::Inst::*;
        match *self {
            Unary { ref arg, .. } => slice::from_ref(arg),
            Binary { ref args, .. } => args,

            With { ref arg } => slice::from_ref(arg),

            StoreScope { ref arg, .. } => slice::from_ref(arg),

            Read { ref arg, .. } => slice::from_ref(arg),
            Write { ref args } => args,
            ScopeError { ref arg } => slice::from_ref(arg),

            LoadField { ref entity, .. } => slice::from_ref(entity),
            LoadFieldDefault { ref entity, .. } => slice::from_ref(entity),

            StoreField { ref args, .. } => args,
            StoreIndex { ref args, .. } => args,

            Release { ref arg, .. } => slice::from_ref(arg),

            Call { ref args, .. } => &args[..],
            Return { ref arg, .. } => slice::from_ref(arg),

            Jump { ref args, .. } => &args[..],
            Branch { ref args, .. } => &args[..],

            Alias(..) |
            Immediate { .. } |
            Argument | DeclareGlobal { .. } | Lookup { .. } |
            // Project instruction arguments are aggregates that can't be register allocated.
            Project { .. } |
            LoadScope { .. } => &[],
        }
    }

    pub fn arguments_mut(&mut self) -> &mut [Value] {
        use self::Inst::*;
        match *self {
            Unary { ref mut arg, .. } => slice::from_ref_mut(arg),
            Binary { ref mut args, .. } => args,

            With { ref mut arg } => slice::from_ref_mut(arg),

            StoreScope { ref mut arg, .. } => slice::from_ref_mut(arg),

            Read { ref mut arg, .. } => slice::from_ref_mut(arg),
            Write { ref mut args, .. } => args,
            ScopeError { ref mut arg, .. } => slice::from_ref_mut(arg),

            LoadField { ref mut entity, .. } => slice::from_ref_mut(entity),
            LoadFieldDefault { ref mut entity, .. } => slice::from_ref_mut(entity),

            StoreField { ref mut args, .. } => args,
            StoreIndex { ref mut args, .. } => args,

            Release { ref mut arg, .. } => slice::from_ref_mut(arg),

            Call { ref mut args, .. } => &mut args[..],
            Return { ref mut arg, .. } => slice::from_ref_mut(arg),

            Jump { ref mut args, .. } => &mut args[..],
            Branch { ref mut args, .. } => &mut args[..],

            Alias(..) |
            Immediate { .. } |
            Argument | DeclareGlobal { .. } | Lookup { .. } |
            // Project instruction arguments are aggregates that can't be register allocated.
            Project { .. } |
            LoadScope { .. } => &mut [],
        }
    }
}

#[derive(Copy, Clone, PartialEq, Debug)]
pub enum Constant {
    Real(f64),
    String(Symbol),
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Unary {
    Negate,
    Invert,
    BitInvert,

    LoadPointer,
    NextPointer,
    ExistsEntity,

    ToArray,
    ToScalar,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Binary {
    Lt,
    Le,
    Eq,
    Ne,
    Ge,
    Gt,

    Add,
    Subtract,
    Multiply,
    Divide,
    Div,
    Mod,

    And,
    Or,
    Xor,

    BitAnd,
    BitOr,
    BitXor,
    ShiftLeft,
    ShiftRight,

    LoadRow,
    LoadIndex,

    StoreRow,

    NePointer,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Prototype {
    Script,
    Function,
}

/// Implement Handle for a tuple struct containing a u32
macro_rules! derive_handle {
    ($handle: ident) => {
        impl Handle for $handle {
            fn new(index: usize) -> Self {
                assert!(index < u32::MAX as usize);
                $handle(index as u32)
            }

            fn index(self) -> usize {
                self.0 as usize
            }
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct Block(u32);
derive_handle!(Block);

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct Value(u32);
derive_handle!(Value);

impl fmt::Debug for Function {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for block in self.blocks.keys() {
            write!(f, "{:?}", block)?;
            {
                let mut arguments = f.debug_tuple("");
                for &argument in &self.blocks[block].arguments {
                    arguments.field(&argument);
                }
                arguments.finish()?;
            }
            writeln!(f)?;

            for &value in &self.blocks[block].instructions {
                write!(f, "  ")?;
                if let Some(def) = self.defs(value) {
                    write!(f, "{:?} <- ", def)?;
                }
                writeln!(f, "{:?}", self.values[value])?;
            }
        }

        Ok(())
    }
}
