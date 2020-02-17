use std::{mem, ptr, slice, cmp, fmt, error};
use std::convert::TryFrom;

use crate::symbol::Symbol;
use crate::vm::{self, code};

/// A single thread of GML execution.
pub struct Thread {
    returns: Vec<(Symbol, usize, usize)>,
    stack: Vec<Register>,

    self_entity: vm::Entity,
    other_entity: vm::Entity,
}

extern {
    type Engine;
}

/// A stack slot for the VM.
///
/// Each variant should be the same size: 64 bits.
#[derive(Copy, Clone)]
pub union Register {
    /// A language-level value.
    pub value: vm::Value,
    /// An intermediate result when working with arrays.
    pub row: vm::Row,

    /// An entity id, resolved from an instance or other scope id.
    pub entity: vm::Entity,
    /// A pointer into an array of entity ids.
    pub iterator: ptr::NonNull<vm::Entity>,
}

pub struct Error {
    pub symbol: Symbol,
    pub instruction: usize,
    pub kind: ErrorKind,
}

#[derive(Debug)]
pub enum ErrorKind {
    /// Unary type error.
    TypeUnary(code::Op, vm::Type),
    /// Binary type error.
    TypeBinary(code::Op, vm::Type, vm::Type),
    /// Division by zero.
    DivideByZero,
    /// Function call arity mismatch.
    Arity(usize),
    /// Scope does not exit.
    Scope(i32),
    /// Name in entity does not exit.
    Name(Symbol),
    /// Variable is read-only.
    Write(Symbol),
    /// Array index out of bounds.
    Bounds(i32),
    /// Error from a library.
    Other(Box<dyn error::Error>),
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}+{}:{:?}", self.symbol, self.instruction, self.kind)
    }
}

impl fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use ErrorKind::*;
        match *self {
            TypeUnary(_, _) => write!(f, "wrong type of arguments to unary operator"),
            TypeBinary(_, _, _) => write!(f, "wrong type of arguments to binary operator"),
            DivideByZero => write!(f, "division by 0"),
            Arity(_) => write!(f, "wrong number of arguments to function or script"),
            Scope(_) => write!(f, "scope does not exist"),
            Name(symbol) => write!(f, "unknown variable {}", symbol),
            Write(symbol) => write!(f, "cannot assign to the variable {}", symbol),
            Bounds(_) => write!(f, "array index out of bounds"),
            Other(ref error) => error.fmt(f),
        }
    }
}

pub const SELF: i32 = -1;
pub const OTHER: i32 = -2;
pub const ALL: i32 = -3;
pub const NOONE: i32 = -4;
pub const GLOBAL: i32 = -5;
// -6?
pub const LOCAL: i32 = -7;

impl Thread {
    pub fn new() -> Self {
        Self {
            returns: vec![],
            stack: vec![],

            self_entity: vm::Entity(0),
            other_entity: vm::Entity(0),
        }
    }

    pub fn set_self(&mut self, entity: vm::Entity) {
        self.self_entity = entity;
    }

    pub fn set_other(&mut self, entity: vm::Entity) {
        self.other_entity = entity;
    }

    pub fn execute<E: vm::world::Api>(
        &mut self,
        engine: &mut E, resources: &vm::Resources<E>,
        symbol: Symbol, arguments: &[vm::Value]
    ) -> Result<vm::Value, Error> {
        let world = E::receivers(engine) as *mut _;
        let engine = unsafe { &mut *(engine as *mut _ as *mut Engine) };
        let resources = unsafe { &*(resources as *const _ as *const vm::Resources<Engine>) };
        self.execute_internal(engine, world, resources, symbol, arguments)
    }

    fn execute_internal<'a>(
        &mut self,
        engine: &'a mut Engine, world: *mut vm::World, resources: &vm::Resources<Engine>,
        symbol: Symbol, arguments: &[vm::Value]
    ) -> Result<vm::Value, Error> {
        let mut symbol = symbol;
        let mut function = &resources.scripts[&symbol];
        let mut instruction = 0;
        let mut reg_base = self.stack.len();

        let default = Register { value: vm::Value::from(0.0) };
        self.stack.resize(reg_base + function.locals as usize, default);

        let arg_len = cmp::min(function.params as usize, arguments.len());
        let arguments = unsafe { mem::transmute::<&[vm::Value], &[Register]>(arguments) };
        self.stack[reg_base..reg_base + arg_len].copy_from_slice(&arguments[0..arg_len]);

        // Enforce that `world` is treated as a reborrow of `engine`.
        fn constrain<F: for<'a> Fn(&'a mut Engine) -> &'a mut vm::World>(f: F) -> F { f }
        let world = constrain(move |_| unsafe { &mut *world });

        loop {
            let registers = &mut self.stack[reg_base..];

            match function.instructions[instruction].decode() {
                (code::Op::Imm, t, constant, _) => {
                    registers[t].value = function.constants[constant];
                }

                (code::Op::Move, t, s, _) => {
                    registers[t] = registers[s];
                }

                (op @ code::Op::Neg, t, a, _) => {
                    let a = unsafe { registers[a].value };
                    registers[t].value = match a.data() {
                        vm::Data::Real(a) => Ok(vm::Value::from(-a)),
                        a => {
                            let kind = ErrorKind::TypeUnary(op, a.ty());
                            Err(Error { symbol, instruction, kind })
                        }
                    }?;
                }

                (op @ code::Op::Not, t, a, _) => {
                    let a = unsafe { registers[a].value };
                    registers[t].value = match a.data() {
                        vm::Data::Real(a) => {
                            let a = Self::to_bool(a);
                            Ok(vm::Value::from(!a))
                        }
                        a => {
                            let kind = ErrorKind::TypeUnary(op, a.ty());
                            Err(Error { symbol, instruction, kind })
                        }
                    }?;
                }

                (op @ code::Op::BitNot, t, a, _) => {
                    let a = unsafe { registers[a].value };
                    registers[t].value = match a.data() {
                        vm::Data::Real(a) => {
                            let a = Self::to_i32(a);
                            Ok(vm::Value::from(!a))
                        }
                        a => {
                            let kind = ErrorKind::TypeUnary(op, a.ty());
                            Err(Error { symbol, instruction, kind })
                        }
                    }?;
                }

                (op @ code::Op::Lt, t, a, b) => {
                    let a = unsafe { registers[a].value };
                    let b = unsafe { registers[b].value };
                    registers[t].value = match (a.data(), b.data()) {
                        (vm::Data::Real(a), vm::Data::Real(b)) => Ok(vm::Value::from(a < b)),
                        (vm::Data::String(a), vm::Data::String(b)) => Ok(vm::Value::from(a < b)),
                        (a, b) => {
                            let kind = ErrorKind::TypeBinary(op, a.ty(), b.ty());
                            Err(Error { symbol, instruction, kind })
                        }
                    }?;
                }

                (op @ code::Op::Le, t, a, b) => {
                    let a = unsafe { registers[a].value };
                    let b = unsafe { registers[b].value };
                    registers[t].value = match (a.data(), b.data()) {
                        (vm::Data::Real(a), vm::Data::Real(b)) => Ok(vm::Value::from(a <= b)),
                        (vm::Data::String(a), vm::Data::String(b)) => Ok(vm::Value::from(a <= b)),
                        (a, b) => {
                            let kind = ErrorKind::TypeBinary(op, a.ty(), b.ty());
                            Err(Error { symbol, instruction, kind })
                        }
                    }?;
                }

                (code::Op::Eq, t, a, b) => {
                    let a = unsafe { registers[a].value };
                    let b = unsafe { registers[b].value };
                    registers[t].value = vm::Value::from(a == b);
                }

                (code::Op::Ne, t, a, b) => {
                    let a = unsafe { registers[a].value };
                    let b = unsafe { registers[b].value };
                    registers[t].value = vm::Value::from(a != b);
                }

                (op @ code::Op::Ge, t, a, b) => {
                    let a = unsafe { registers[a].value };
                    let b = unsafe { registers[b].value };
                    registers[t].value = match (a.data(), b.data()) {
                        (vm::Data::Real(a), vm::Data::Real(b)) => Ok(vm::Value::from(a >= b)),
                        (vm::Data::String(a), vm::Data::String(b)) => Ok(vm::Value::from(a >= b)),
                        (a, b) => {
                            let kind = ErrorKind::TypeBinary(op, a.ty(), b.ty());
                            Err(Error { symbol, instruction, kind })
                        }
                    }?;
                }

                (op @ code::Op::Gt, t, a, b) => {
                    let a = unsafe { registers[a].value };
                    let b = unsafe { registers[b].value };
                    registers[t].value = match (a.data(), b.data()) {
                        (vm::Data::Real(a), vm::Data::Real(b)) => Ok(vm::Value::from(a > b)),
                        (vm::Data::String(a), vm::Data::String(b)) => Ok(vm::Value::from(a > b)),
                        (a, b) => {
                            let kind = ErrorKind::TypeBinary(op, a.ty(), b.ty());
                            Err(Error { symbol, instruction, kind })
                        }
                    }?;
                }

                (op @ code::Op::Add, t, a, b) => {
                    let a = unsafe { registers[a].value };
                    let b = unsafe { registers[b].value };
                    registers[t].value = match (a.data(), b.data()) {
                        (vm::Data::Real(a), vm::Data::Real(b)) => Ok(vm::Value::from(a + b)),
                        (vm::Data::String(a), vm::Data::String(b)) =>
                            Ok(vm::Value::from(Symbol::intern(&[a, b].concat()))),
                        (a, b) => {
                            let kind = ErrorKind::TypeBinary(op, a.ty(), b.ty());
                            Err(Error { symbol, instruction, kind })
                        }
                    }?;
                }

                (op @ code::Op::Sub, t, a, b) => {
                    let a = unsafe { registers[a].value };
                    let b = unsafe { registers[b].value };
                    registers[t].value = match (a.data(), b.data()) {
                        (vm::Data::Real(a), vm::Data::Real(b)) => Ok(vm::Value::from(a - b)),
                        (a, b) => {
                            let kind = ErrorKind::TypeBinary(op, a.ty(), b.ty());
                            Err(Error { symbol, instruction, kind })
                        }
                    }?;
                }

                (op @ code::Op::Mul, t, a, b) => {
                    let a = unsafe { registers[a].value };
                    let b = unsafe { registers[b].value };
                    registers[t].value = match (a.data(), b.data()) {
                        (vm::Data::Real(a), vm::Data::Real(b)) => Ok(vm::Value::from(a * b)),
                        (vm::Data::Real(a), vm::Data::String(b)) => {
                            let b: &str = &b;
                            let t = b.repeat(a as usize);
                            Ok(vm::Value::from(Symbol::intern(&t)))
                        }
                        (a, b) => {
                            let kind = ErrorKind::TypeBinary(op, a.ty(), b.ty());
                            Err(Error { symbol, instruction, kind })
                        }
                    }?;
                }

                (op @ code::Op::Div, t, a, b) => {
                    let a = unsafe { registers[a].value };
                    let b = unsafe { registers[b].value };
                    registers[t].value = match (a.data(), b.data()) {
                        (vm::Data::Real(a), vm::Data::Real(b)) => {
                            if b == 0.0 {
                                let kind = ErrorKind::DivideByZero;
                                return Err(Error { symbol, instruction, kind });
                            }
                            Ok(vm::Value::from(a / b))
                        }
                        (a, b) => {
                            let kind = ErrorKind::TypeBinary(op, a.ty(), b.ty());
                            Err(Error { symbol, instruction, kind })
                        }
                    }?;
                }

                (op @ code::Op::IntDiv, t, a, b) => {
                    let a = unsafe { registers[a].value };
                    let b = unsafe { registers[b].value };
                    registers[t].value = match (a.data(), b.data()) {
                        (vm::Data::Real(a), vm::Data::Real(b)) => {
                            if b == 0.0 {
                                let kind = ErrorKind::DivideByZero;
                                return Err(Error { symbol, instruction, kind });
                            }
                            let t = a / b;
                            Ok(vm::Value::from(t as i32))
                        }
                        (a, b) => {
                            let kind = ErrorKind::TypeBinary(op, a.ty(), b.ty());
                            Err(Error { symbol, instruction, kind })
                        }
                    }?;
                }

                (op @ code::Op::Mod, t, a, b) => {
                    let a = unsafe { registers[a].value };
                    let b = unsafe { registers[b].value };
                    registers[t].value = match (a.data(), b.data()) {
                        (vm::Data::Real(a), vm::Data::Real(b)) => {
                            if b == 0.0 {
                                let kind = ErrorKind::DivideByZero;
                                return Err(Error { symbol, instruction, kind });
                            }
                            Ok(vm::Value::from(a % b))
                        }
                        (a, b) => {
                            let kind = ErrorKind::TypeBinary(op, a.ty(), b.ty());
                            Err(Error { symbol, instruction, kind })
                        }
                    }?;
                }

                (op @ code::Op::And, t, a, b) => {
                    let a = unsafe { registers[a].value };
                    let b = unsafe { registers[b].value };
                    registers[t].value = match (a.data(), b.data()) {
                        (vm::Data::Real(a), vm::Data::Real(b)) => {
                            let a = Self::to_bool(a);
                            let b = Self::to_bool(b);
                            Ok(vm::Value::from(a && b))
                        }
                        (a, b) => {
                            let kind = ErrorKind::TypeBinary(op, a.ty(), b.ty());
                            Err(Error { symbol, instruction, kind })
                        }
                    }?;
                }

                (op @ code::Op::Or, t, a, b) => {
                    let a = unsafe { registers[a].value };
                    let b = unsafe { registers[b].value };
                    registers[t].value = match (a.data(), b.data()) {
                        (vm::Data::Real(a), vm::Data::Real(b)) => {
                            let a = Self::to_bool(a);
                            let b = Self::to_bool(b);
                            Ok(vm::Value::from(a || b))
                        }
                        (a, b) => {
                            let kind = ErrorKind::TypeBinary(op, a.ty(), b.ty());
                            Err(Error { symbol, instruction, kind })
                        }
                    }?;
                }

                (op @ code::Op::Xor, t, a, b) => {
                    let a = unsafe { registers[a].value };
                    let b = unsafe { registers[b].value };
                    registers[t].value = match (a.data(), b.data()) {
                        (vm::Data::Real(a), vm::Data::Real(b)) => {
                            let a = Self::to_bool(a);
                            let b = Self::to_bool(b);
                            Ok(vm::Value::from(a != b))
                        }
                        (a, b) => {
                            let kind = ErrorKind::TypeBinary(op, a.ty(), b.ty());
                            Err(Error { symbol, instruction, kind })
                        }
                    }?;
                }

                (op @ code::Op::BitAnd, t, a, b) => {
                    let a = unsafe { registers[a].value };
                    let b = unsafe { registers[b].value };
                    registers[t].value = match (a.data(), b.data()) {
                        (vm::Data::Real(a), vm::Data::Real(b)) => {
                            let a = Self::to_i32(a);
                            let b = Self::to_i32(b);
                            Ok(vm::Value::from(a & b))
                        }
                        (a, b) => {
                            let kind = ErrorKind::TypeBinary(op, a.ty(), b.ty());
                            Err(Error { symbol, instruction, kind })
                        }
                    }?;
                }

                (op @ code::Op::BitOr, t, a, b) => {
                    let a = unsafe { registers[a].value };
                    let b = unsafe { registers[b].value };
                    registers[t].value = match (a.data(), b.data()) {
                        (vm::Data::Real(a), vm::Data::Real(b)) => {
                            let a = Self::to_i32(a);
                            let b = Self::to_i32(b);
                            Ok(vm::Value::from(a | b))
                        }
                        (a, b) => {
                            let kind = ErrorKind::TypeBinary(op, a.ty(), b.ty());
                            Err(Error { symbol, instruction, kind })
                        }
                    }?;
                }

                (op @ code::Op::BitXor, t, a, b) => {
                    let a = unsafe { registers[a].value };
                    let b = unsafe { registers[b].value };
                    registers[t].value = match (a.data(), b.data()) {
                        (vm::Data::Real(a), vm::Data::Real(b)) => {
                            let a = Self::to_i32(a);
                            let b = Self::to_i32(b);
                            Ok(vm::Value::from(a ^ b))
                        }
                        (a, b) => {
                            let kind = ErrorKind::TypeBinary(op, a.ty(), b.ty());
                            Err(Error { symbol, instruction, kind })
                        }
                    }?;
                }

                (op @ code::Op::ShiftLeft, t, a, b) => {
                    let a = unsafe { registers[a].value };
                    let b = unsafe { registers[b].value };
                    registers[t].value = match (a.data(), b.data()) {
                        (vm::Data::Real(a), vm::Data::Real(b)) => {
                            let a = Self::to_i32(a);
                            let b = Self::to_i32(b);
                            Ok(vm::Value::from(a << b))
                        }
                        (a, b) => {
                            let kind = ErrorKind::TypeBinary(op, a.ty(), b.ty());
                            Err(Error { symbol, instruction, kind })
                        }
                    }?;
                }

                (op @ code::Op::ShiftRight, t, a, b) => {
                    let a = unsafe { registers[a].value };
                    let b = unsafe { registers[b].value };
                    registers[t].value = match (a.data(), b.data()) {
                        (vm::Data::Real(a), vm::Data::Real(b)) => {
                            let a = Self::to_i32(a);
                            let b = Self::to_i32(b);
                            Ok(vm::Value::from(a >> b))
                        }
                        (a, b) => {
                            let kind = ErrorKind::TypeBinary(op, a.ty(), b.ty());
                            Err(Error { symbol, instruction, kind })
                        }
                    }?;
                }

                (code::Op::DeclareGlobal, name, _, _) => {
                    let name = Self::get_string(function.constants[name]);
                    world(engine).globals.insert(name);

                    let instance = &mut world(engine).members[vm::world::GLOBAL];
                    instance.entry(name).or_insert(vm::Value::from(0.0));
                }

                (code::Op::Lookup, t, name, _) => {
                    let name = Self::get_string(function.constants[name]);
                    registers[t].entity = if world(engine).globals.contains(&name) {
                        vm::world::GLOBAL
                    } else {
                        self.self_entity
                    };
                }

                // TODO: Replace these with `self`/`other` arguments and locals passed to `Lookup`.

                (code::Op::LoadScope, t, scope, _) => {
                    registers[t].entity = match scope as i8 as i32 {
                        SELF => self.self_entity,
                        OTHER => self.other_entity,
                        GLOBAL => vm::world::GLOBAL,
                        scope => {
                            let kind = ErrorKind::Scope(scope);
                            return Err(Error { symbol, instruction, kind });
                        }
                    };
                }

                (code::Op::StoreScope, s, scope, _) => {
                    let s = unsafe { registers[s].entity };
                    match scope as i8 as i32 {
                        SELF => self.self_entity = s,
                        OTHER => self.other_entity = s,
                        scope => {
                            let kind = ErrorKind::Scope(scope);
                            return Err(Error { symbol, instruction, kind });
                        }
                    }
                }

                (op @ code::Op::With, ptr, end, scope) => {
                    let scope = unsafe { registers[scope].value };
                    let scope = match scope.data() {
                        vm::Data::Real(scope) => Ok(Self::to_i32(scope)),
                        scope => {
                            let kind = ErrorKind::TypeUnary(op, scope.ty());
                            Err(Error { symbol, instruction, kind })
                        }
                    }?;

                    let slice = match scope {
                        SELF => slice::from_ref(&self.self_entity),
                        OTHER => slice::from_ref(&self.other_entity),
                        ALL => world(engine).instances.values(),
                        NOONE => &[],
                        GLOBAL => slice::from_ref(&vm::world::GLOBAL),
                        LOCAL => &[], // TODO: error
                        object if (0..=100_000).contains(&object) =>
                            &world(engine).objects[&object][..],
                        instance if (100_001..).contains(&instance) =>
                            slice::from_ref(&world(engine).instances[instance]),
                        _ => &[], // TODO: error
                    };
                    unsafe {
                        let first = slice.as_ptr() as *mut vm::Entity;
                        let last = first.offset(slice.len() as isize);
                        registers[ptr].iterator = ptr::NonNull::new_unchecked(first);
                        registers[end].iterator = ptr::NonNull::new_unchecked(last);
                    }
                }

                (code::Op::LoadPointer, t, ptr, _) => {
                    let ptr = unsafe { registers[ptr].iterator };
                    registers[t].entity = unsafe { *ptr.as_ptr() };
                }

                (code::Op::NextPointer, t, ptr, _) => {
                    let ptr = unsafe { registers[ptr].iterator };
                    registers[t].iterator = unsafe {
                        ptr::NonNull::new_unchecked(ptr.as_ptr().offset(1))
                    };
                }

                (code::Op::NePointer, t, a, b) => {
                    let a = unsafe { registers[a].iterator };
                    let b = unsafe { registers[b].iterator };
                    registers[t].value = vm::Value::from(a != b);
                }

                (code::Op::ExistsEntity, t, entity, _) => {
                    let entity = unsafe { registers[entity].entity };
                    let exists = world(engine).members.contains_key(entity);
                    registers[t].value = vm::Value::from(exists);
                }

                (op @ code::Op::Read, a, local, _) => {
                    let a = unsafe { registers[a].value };
                    match a.data() {
                        vm::Data::Real(a) => {
                            let a = Self::to_bool(a);
                            if !a {
                                let local = Self::get_string(function.constants[local]);
                                let kind = ErrorKind::Name(local);
                                return Err(Error { symbol, instruction, kind });
                            }
                        }
                        a => {
                            let kind = ErrorKind::TypeUnary(op, a.ty());
                            return Err(Error { symbol, instruction, kind });
                        }
                    }
                }

                (code::Op::Write, t, a, b) => {
                    let a = unsafe { registers[a].value };
                    let b = unsafe { registers[b].value };
                    registers[t].value = match b.data() {
                        vm::Data::Array(b) => {
                            let _ = b.store(0, 0, a);
                            vm::Value::from(b)
                        }
                        _ => a
                    };
                }

                (op @ code::Op::ScopeError, scope, _, _) => {
                    let scope = unsafe { registers[scope].value };
                    match scope.data() {
                        vm::Data::Real(scope) => {
                            let scope = Self::to_i32(scope);
                            let kind = ErrorKind::Scope(scope);
                            return Err(Error { symbol, instruction, kind });
                        }
                        scope => {
                            let kind = ErrorKind::TypeUnary(op, scope.ty());
                            return Err(Error { symbol, instruction, kind });
                        }
                    }
                }

                (code::Op::ToArray, t, a, _) => {
                    let a = unsafe { registers[a].value };
                    registers[t].value = match a.data() {
                        vm::Data::Array(_) => Ok(a),
                        _ => Ok(vm::Value::from(vm::Array::from_scalar(a))),
                    }?;
                }

                (code::Op::ToScalar, t, a, _) => {
                    let a = unsafe { registers[a].value };
                    registers[t].value = match a.data() {
                        vm::Data::Array(array) => {
                             array.load(0, 0)
                                .map_err(|_| {
                                    let kind = ErrorKind::Bounds(0);
                                    Error { symbol, instruction, kind }
                                })?
                        }
                        _ => a,
                    };
                }

                (code::Op::Release, a, _, _) => {
                    let a = unsafe { registers[a].value };
                    unsafe { a.release() };
                }

                (code::Op::LoadField, t, entity, field) => {
                    let entity = unsafe { registers[entity].entity };
                    let field = Self::get_string(function.constants[field]);
                    let instance = &world(engine).members[entity];
                    registers[t].value = *instance.get(&field)
                        .ok_or_else(|| {
                            let kind = ErrorKind::Name(field);
                            Error { symbol, instruction, kind }
                        })?;
                }

                (code::Op::LoadFieldDefault, t, entity, field) => {
                    let entity = unsafe { registers[entity].entity };
                    let field = Self::get_string(function.constants[field]);
                    let instance = &world(engine).members[entity];
                    registers[t].value = *instance.get(&field)
                        .unwrap_or(&vm::Value::from(0.0));
                }

                (op @ code::Op::LoadRow, t, a, i) => {
                    let a = unsafe { registers[a].value };
                    let i = unsafe { registers[i].value };
                    registers[t].row = match (a.data(), i.data()) {
                        (vm::Data::Array(array), vm::Data::Real(i)) => {
                            let i = Self::to_i32(i);
                            let value = array.load_row(i)
                                .map_err(|_| {
                                    let kind = ErrorKind::Bounds(i);
                                    Error { symbol, instruction, kind }
                                })?;
                            Ok(value)
                        }
                        (a, i) => {
                            let kind = ErrorKind::TypeBinary(op, a.ty(), i.ty());
                            Err(Error { symbol, instruction, kind })
                        }
                    }?;
                }

                (op @ code::Op::LoadIndex, t, r, j) => {
                    let r = unsafe { registers[r].row };
                    let j = unsafe { registers[j].value };
                    registers[t].value = match j.data() {
                        vm::Data::Real(j) => {
                            let j = Self::to_i32(j);
                            let value = unsafe { r.load(j) }
                                .map_err(|_| {
                                    let kind = ErrorKind::Bounds(j);
                                    Error { symbol, instruction, kind }
                                })?;
                            Ok(value)
                        }
                        j => {
                            let kind = ErrorKind::TypeBinary(op, vm::Type::Array, j.ty());
                            Err(Error { symbol, instruction, kind })
                        }
                    }?;
                }

                (code::Op::StoreField, s, entity, field) => {
                    let s = unsafe { registers[s].value };
                    let entity = unsafe { registers[entity].entity };
                    let field = Self::get_string(function.constants[field]);
                    let instance = &mut world(engine).members[entity];
                    instance.insert(field, s);
                }

                (op @ code::Op::StoreRow, t, a, i) => {
                    let a = unsafe { registers[a].value };
                    let i = unsafe { registers[i].value };
                    registers[t].row = match (a.data(), i.data()) {
                        (vm::Data::Array(array), vm::Data::Real(i)) => {
                            let i = Self::to_i32(i);
                            let value = array.store_row(i)
                                .map_err(|_| {
                                    let kind = ErrorKind::Bounds(i);
                                    Error { symbol, instruction, kind }
                                })?;
                            Ok(value)
                        }
                        (a, i) => {
                            let kind = ErrorKind::TypeBinary(op, a.ty(), i.ty());
                            Err(Error { symbol, instruction, kind })
                        }
                    }?;
                }

                (op @ code::Op::StoreIndex, s, r, j) => {
                    let s = unsafe { registers[s].value };
                    let r = unsafe { registers[r].row };
                    let j = unsafe { registers[j].value };
                    match j.data() {
                        vm::Data::Real(j) => {
                            let j = Self::to_i32(j);
                            unsafe { r.store(j, s) }
                                .map_err(|_| {
                                    let kind = ErrorKind::Bounds(j);
                                    Error { symbol, instruction, kind }
                                })?;
                            Ok(())
                        }
                        j => {
                            let kind = ErrorKind::TypeBinary(op, vm::Type::Array, j.ty());
                            Err(Error { symbol, instruction, kind })
                        }
                    }?;
                }

                (code::Op::Call, callee, base, len) => {
                    self.returns.push((symbol, instruction + 1, reg_base));

                    symbol = Self::get_string(function.constants[callee]);
                    function = &resources.scripts[&symbol];
                    instruction = 0;
                    reg_base = reg_base + base;

                    let limit = cmp::max(function.locals as usize, len);
                    let default = Register { value: vm::Value::from(0.0) };
                    self.stack.resize(reg_base + limit, default);

                    let registers = &mut self.stack[reg_base..];
                    for arg in &mut registers[len..function.params as usize] {
                        arg.value = vm::Value::from(0.0);
                    }

                    continue;
                }

                (code::Op::CallApi, callee, base, len) => {
                    let api_symbol = Self::get_string(function.constants[callee]);
                    let function = resources.api[&api_symbol];
                    let reg_base = reg_base + base;

                    let registers = &mut self.stack[reg_base..];
                    let entity = self.self_entity;
                    let arguments = unsafe { mem::transmute::<_, &[vm::Value]>(&registers[..len]) };
                    registers[0].value = function(engine, resources, entity, arguments)
                        .map_err(|kind| Error { symbol, instruction, kind })?;
                }

                (code::Op::CallGet, get, base, _) => {
                    let symbol = Self::get_string(function.constants[get]);
                    let function = resources.get.get(&symbol)
                        .ok_or_else(|| {
                            let kind = ErrorKind::Name(symbol);
                            Error { symbol, instruction, kind }
                        })?;
                    let reg_base = reg_base + base;

                    let registers = &mut self.stack[reg_base..];
                    let entity = unsafe { registers[0].entity };
                    let i = unsafe { registers[1].value };
                    let i = i32::try_from(i).unwrap_or(0) as usize;
                    registers[0].value = function(engine, entity, i);
                }

                (code::Op::CallSet, set, base, _) => {
                    let symbol = Self::get_string(function.constants[set]);
                    let function = resources.set.get(&symbol)
                        .ok_or_else(|| {
                            let kind = ErrorKind::Write(symbol);
                            Error { symbol, instruction, kind }
                        })?;
                    let reg_base = reg_base + base;

                    let registers = &self.stack[reg_base..];
                    let value = unsafe { registers[0].value };
                    let entity = unsafe { registers[1].entity };
                    let i = unsafe { registers[2].value };
                    let i = i32::try_from(i).unwrap_or(0) as usize;
                    function(engine, entity, i, value);
                }

                (code::Op::Ret, _, _, _) => {
                    let (caller, caller_instruction, caller_base) = match self.returns.pop() {
                        Some(frame) => frame,
                        None => {
                            let value = unsafe { registers[0].value };
                            return Ok(value);
                        }
                    };

                    symbol = caller;
                    function = &resources.scripts[&symbol];
                    instruction = caller_instruction;
                    reg_base = caller_base;

                    let default = Register { value: vm::Value::from(0.0) };
                    self.stack.resize(reg_base + function.locals as usize, default);

                    continue;
                }

                (code::Op::Jump, t_low, t_high, _) => {
                    instruction = t_low | (t_high << 8);
                    continue;
                }

                (op @ code::Op::BranchFalse, a, t_low, t_high) => {
                    let a = unsafe { registers[a].value };
                    match a.data() {
                        vm::Data::Real(a) => {
                            let a = Self::to_bool(a);
                            if !a {
                                instruction = t_low | (t_high << 8);
                                continue;
                            }
                        }
                        a => {
                            let kind = ErrorKind::TypeUnary(op, a.ty());
                            return Err(Error { symbol, instruction, kind });
                        }
                    }
                }
            }

            instruction += 1;
        }
    }

    // TODO: round-to-nearest instead of truncate
    pub fn to_i32(value: f64) -> i32 {
        value as i32
    }

    // TODO: round-to-nearest instead of truncate
    pub fn to_u32(value: f64) -> u32 {
        value as u32
    }

    pub fn to_bool(value: f64) -> bool {
        Self::to_i32(value) > 0
    }

    fn get_string(value: vm::Value) -> Symbol {
        match value.data() {
            vm::Data::String(value) => value,
            _ => unreachable!("expected a string"),
        }
    }
}
