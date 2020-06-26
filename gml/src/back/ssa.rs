use std::{u32, slice, fmt, ops::Range};

use crate::handle_map::{Handle, HandleMap};
use crate::symbol::Symbol;

/// A function, defined as a control flow graph with statements in SSA form.
///
/// Instructions are stored in a single large array. Their results are referred to via `Value`s,
/// which are indices into that array. Basic blocks are layered on top, as separate arrays of
/// `Value`s defining execution order within a block.
pub struct Function {
    pub blocks: HandleMap<Label, Block>,
    pub values: HandleMap<Value, Instruction>,
    // TODO: move this to register allocation
    pub return_def: Value,

    pub locations: HandleMap<Value, usize>,
}

/// A handle to a basic block.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct Label(u32);
derive_handle!(Label);

/// A handle to an instruction (and, implicitly, its result).
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct Value(u32);
derive_handle!(Value);

pub const ENTRY: Label = Label(0);
pub const EXIT: Label = Label(1);

pub struct Block {
    pub parameters: Vec<Value>,
    pub instructions: Vec<Value>,
}

/// An SSA instruction.
///
/// This defines an instruction's format- the number and types of its arguments and results.
#[derive(PartialEq, Debug)]
pub enum Instruction {
    /// A placeholder for a value that has been replaced.
    ///
    /// This IR does not track def-use chains. Instead, when uses of an instruction are to be
    /// updated, it is replaced with an `Alias` that forwards them to the new location. `Alias`es
    /// can be removed all at once after passes that create them, or read through in passes that
    /// cannot handle them.
    Alias { arg: Value },
    /// Obtain a scalar value from a tuple value.
    ///
    /// The number of results an instruction has is determined by its opcode. When an instruction
    /// has multiple results, its `Value` represents a single "tuple" result. Uses of the tuple's
    /// elements refer to `Project` instructions that are always allocated immediately following
    /// the primary instruction.
    Project { arg: Value, index: u8 },
    /// A parameter of a basic block.
    ///
    /// Instead of the traditional phi nodes, values at control flow join points are defined by
    /// `Parameter`s to basic blocks, which correspond to arguments passed by `Jump` and `Branch`
    /// instructions.
    Parameter,

    Nullary { op: Opcode },
    Unary { op: Opcode, arg: Value },
    UnaryReal { op: Opcode, real: f64 },
    UnarySymbol { op: Opcode, symbol: Symbol },
    Binary { op: Opcode, args: [Value; 2] },
    // TODO: remove along with StoreScope
    BinaryReal { op: Opcode, arg: Value, real: f64 },
    BinarySymbol { op: Opcode, arg: Value, symbol: Symbol },
    Ternary { op: Opcode, args: [Value; 3] },
    TernarySymbol { op: Opcode, args: [Value; 2], symbol: Symbol },

    // TODO: Remove `parameters` (see front::Codegen::emit_call).
    Call { op: Opcode, symbol: Symbol, args: Vec<Value>, parameters: Vec<Value> },
    // TODO: Replace `args` with `Argument` instructions?
    Jump { op: Opcode, target: Label, args: Vec<Value> },
    /// `args` contains `[condition, arg_lens[0].., arg_lens[1]..]`
    Branch { op: Opcode, targets: [Label; 2], arg_lens: [u32; 2], args: Vec<Value> },
}

/// An SSA opcode.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Opcode {
    // Unary opcodes:

    /// Materialize a constant. Can be UnaryReal or UnarySymbol.
    Constant,

    Negate,
    Invert,
    BitInvert,

    /// Ensure a value is an array. Scalars are converted to single-element arrays.
    ToArray,
    /// Ensure a value is a scalar. Arrays are converted to their first element.
    ToScalar,

    Release,
    Return,

    /// Build an iterator over a scope, producing a tuple of start and end.
    With,
    /// Release the iterator created by the most recent `With`.
    ReleaseWith,
    /// Error that a scope does not exist.
    ScopeError,
    LoadPointer,
    NextPointer,
    ExistsEntity,

    /// Define a symbol as `globalvar`.
    DeclareGlobal,
    /// Determine whether a symbol should be treated as a member of `self` or `global`.
    Lookup,
    /// Load the entity for the well-known scope `self`, `other`, or `global`.
    LoadScope,

    // Binary opcodes:

    Lt,
    Le,
    Eq,
    Ne,
    Ge,
    Gt,

    // TODO: reuse `Ne` using instruction types?
    NePointer,

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

    /// Mark a local as read; error if initialization flag is false.
    Read,
    /// Overwrite a value with a scalar, using pre-GMS semantics.
    ///
    /// Scalars are completely replaced; arrays only have their first element replaced.
    Write,

    /// Set the entity for the well-known scope `self` or `other`.
    StoreScope,

    /// Load a value from a field in a component.
    LoadField,
    /// Like `LoadField`, but produce 0 if the field does not exist.
    LoadFieldDefault,

    /// Load a row from an array; error if out of bounds.
    LoadRow,
    /// Load a row from an array; grow it if it does not exist.
    StoreRow,
    /// Load a value from a row; error if out of bounds.
    LoadIndex,

    // Ternary opcodes:

    /// Store a value into a field in a component.
    StoreField,
    /// Store a value into a row; grow it if out of bounds.
    StoreIndex,

    // N-ary opcodes:

    // TODO: combine these using instruction types?
    Call,
    CallApi,
    CallGet,
    CallSet,
    Jump,
    Branch,
}

/// A declaration of some external entity.
///
/// This contains just enough information to generate code for the caller.
// TODO: gms tracks function arity
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum Prototype {
    /// A GML script.
    Script { id: i32 },
    /// A native API function.
    Native { arity: usize, variadic: bool },
    /// A built-in member accessed via getter and setter.
    Member,
}

impl Function {
    pub fn new() -> Self {
        let blocks = HandleMap::new();
        let mut values = HandleMap::new();

        let op = Opcode::Constant;
        let return_def = values.push(Instruction::UnaryReal { op, real: 0.0 });

        let locations = HandleMap::new();

        // Create the function with fixed entry and exit labels.
        let mut function = Function { blocks, values, return_def, locations };
        function.make_block();
        function.make_block();

        function
    }

    pub fn make_block(&mut self) -> Label {
        let block = Block {
            parameters: vec![],
            instructions: vec![],
        };

        self.blocks.push(block)
    }

    pub fn emit_parameter(&mut self, block: Label) -> Value {
        let value = self.values.push(Instruction::Parameter);
        self.blocks[block].parameters.push(value);
        value
    }

    pub fn emit_instruction(&mut self, block: Label, instruction: Instruction, location: usize) ->
        Value
    {
        let value = self.values.push(instruction);
        *self.locations.ensure(value) = location;
        self.blocks[block].instructions.push(value);
        value
    }

    pub fn terminator(&self, block: Label) -> Value {
        *self.blocks[block].instructions.last()
            .expect("empty block")
    }

    pub fn successors(&self, block: Label) -> &[Label] {
        use self::Instruction::*;

        let value = self.terminator(block);
        match self.values[value] {
            Jump { ref target, .. } => slice::from_ref(target),
            Branch { ref targets, .. } => targets,
            Unary { op: Opcode::Return, .. } |
            Unary { op: Opcode::ScopeError, .. } => &[],

            _ => panic!("corrupt block"),
        }
    }

    pub fn op(&self, value: Value) -> Opcode {
        use self::Instruction::*;

        match self.values[value] {
            Alias { .. } | Project { .. } | Parameter => panic!("finding op of non-instruction"),

            Nullary { op, .. } |
            Unary { op, .. } |
            UnaryReal { op, .. } |
            UnarySymbol { op, .. } |
            Binary { op, .. } |
            BinaryReal { op, .. } |
            BinarySymbol { op, .. } |
            Ternary { op, .. } |
            TernarySymbol { op, .. } |
            Call { op, .. } |
            Jump { op, .. } |
            Branch { op, .. } => op,
        }
    }

    pub fn defs(&self, value: Value) -> ValueRange {
        use self::Instruction::*;

        let Value(start) = value;
        match self.values[value] {
            Alias { .. } | Project { .. } | Parameter => panic!("finding defs of non-instruction"),

            // Zero-valued instructions:
            Nullary { op: Opcode::ReleaseWith } |
            Unary { op: Opcode::Release, .. } |
            Unary { op: Opcode::Return, .. } |
            Unary { op: Opcode::ScopeError, .. } |
            UnarySymbol { op: Opcode::DeclareGlobal, .. } |
            BinarySymbol { op: Opcode::Read, .. } |
            BinaryReal { op: Opcode::StoreScope, .. } |
            Ternary { .. } |
            TernarySymbol { .. } |
            Call { op: Opcode::CallSet, .. } |
            Jump { .. } |
            Branch { .. } => ValueRange { range: start..start },

            // Two-valued instructions:
            Unary { op: Opcode::With, .. } => ValueRange { range: start + 1..start + 3 },

            // The common case: single-valued instructions:
            Nullary { .. } |
            Unary { .. } |
            UnaryReal { .. } |
            UnarySymbol { .. } |
            Binary { .. } |
            BinaryReal { .. } |
            BinarySymbol { .. } |
            Call { .. } => ValueRange { range: start..start + 1 },
        }
    }

    pub fn internal_defs(&self, value: Value) -> &[Value] {
        use self::Instruction::*;

        match self.values[value] {
            Call { ref parameters, .. } => parameters,
            _ => &[],
        }
    }

    /// Find the uses of an instruction.
    pub fn uses(&self, value: Value) -> &[Value] {
        use self::Instruction::*;

        match self.values[value] {
            Alias { .. } | Project { .. } | Parameter => panic!("finding uses of non-instruction"),

            Nullary { .. } => &[],
            Unary { ref arg, .. } => slice::from_ref(arg),
            UnaryReal { .. } => &[],
            UnarySymbol { .. } => &[],
            Binary { ref args, .. } => args,
            BinaryReal { ref arg, .. } => slice::from_ref(arg),
            BinarySymbol { ref arg, .. } => slice::from_ref(arg),
            Ternary { ref args, .. } => args,
            TernarySymbol { ref args, .. } => args,

            Call { ref args, .. } => &args[..],
            Jump { ref args, .. } => &args[..],
            Branch { ref args, .. } => &args[..],
        }
    }

    /// Find the uses of an instruction.
    pub fn uses_mut(&mut self, value: Value) -> &mut [Value] {
        use self::Instruction::*;

        match self.values[value] {
            Alias { .. } | Project { .. } | Parameter => panic!("finding uses of non-instruction"),

            Nullary { .. } => &mut [],
            Unary { ref mut arg, .. } => slice::from_mut(arg),
            UnaryReal { .. } => &mut [],
            UnarySymbol { .. } => &mut [],
            Binary { ref mut args, .. } => args,
            BinaryReal { ref mut arg, .. } => slice::from_mut(arg),
            BinarySymbol { ref mut arg, .. } => slice::from_mut(arg),
            Ternary { ref mut args, .. } => args,
            TernarySymbol { ref mut args, .. } => args,

            Call { ref mut args, .. } => &mut args[..],
            Jump { ref mut args, .. } => &mut args[..],
            Branch { ref mut args, .. } => &mut args[..],
        }
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct ValueRange {
    range: Range<u32>,
}

impl Iterator for ValueRange {
    type Item = Value;

    fn next(&mut self) -> Option<Self::Item> {
        let index = self.range.next()?;
        Some(Value(index))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.range.size_hint()
    }
}

impl DoubleEndedIterator for ValueRange {
    fn next_back(&mut self) -> Option<Self::Item> {
        let index = self.range.next_back()?;
        Some(Value(index))
    }
}

impl ExactSizeIterator for ValueRange {
    fn len(&self) -> usize {
        self.range.len()
    }
}

impl fmt::Debug for Function {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for block in self.blocks.keys() {
            write!(f, "b{}(", block.index())?;
            write_values(f, self.blocks[block].parameters.iter().cloned())?;
            writeln!(f, "):")?;

            for &value in &self.blocks[block].instructions {
                use crate::back::ssa::Instruction::*;

                write!(f, "    ")?;

                let defs = self.defs(value);
                write_values(f, defs.clone())?;
                if defs.len() > 0 {
                    write!(f, " <- ")?;
                }

                write!(f, "{:?} ", self.op(value))?;

                if let Call { symbol, ref args, .. } = self.values[value] {
                    write!(f, "{}(", symbol)?;
                    write_values(f, args.iter().cloned())?;
                    writeln!(f, ")")?;
                    continue;
                }
                if let Jump { target, ref args, .. } = self.values[value] {
                    write!(f, "b{}(", target.index())?;
                    write_values(f, args.iter().cloned())?;
                    writeln!(f, ")")?;
                    continue;
                }
                if let Branch {
                    targets: [true_block, false_block],
                    arg_lens: [true_args, false_args],
                    ref args,
                    ..
                } = self.values[value] {
                    write!(f, "v{}", args[0].index())?;

                    let true_start = 1;
                    let true_end = true_start + true_args as usize;
                    write!(f, ", b{}(", true_block.index())?;
                    write_values(f, args[true_start..true_end].iter().cloned())?;
                    write!(f, ")")?;

                    let false_start = true_end;
                    let false_end = false_start + false_args as usize;
                    write!(f, ", b{}(", false_block.index())?;
                    write_values(f, args[false_start..false_end].iter().cloned())?;
                    write!(f, ")")?;

                    writeln!(f)?;
                    continue;
                }

                write_values(f, self.uses(value).iter().cloned())?;
                match self.values[value] {
                    UnaryReal { real, .. } => write!(f, "{}", real)?,
                    UnarySymbol { symbol, .. } => write!(f, "{}", symbol)?,
                    BinaryReal { real, .. } => write!(f, ", {}", real)?,
                    BinarySymbol { symbol, .. } => write!(f, ", {}", symbol)?,
                    TernarySymbol { symbol, .. } => write!(f, ", {}", symbol)?,
                    _ => (),
                }
                writeln!(f)?;
            }
        }

        Ok(())
    }
}

fn write_values<I>(f: &mut fmt::Formatter<'_>, values: I) -> fmt::Result where
    I: IntoIterator<Item = Value>
{
    let mut has_values = false;
    for value in values {
        if has_values {
            write!(f, ", ")?;
        }
        write!(f, "v{}", value.index())?;
        has_values = true;
    }
    Ok(())
}
