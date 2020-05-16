use std::{mem, ptr, iter, slice, cmp, fmt, error};
use std::convert::TryFrom;
use std::mem::ManuallyDrop;

use crate::symbol::Symbol;
use crate::vm::{code, world};
use crate::vm::{World, Entity, Value, ValueRef, Data, to_i32, to_bool, Array, ArrayRef, Resources};

/// A single thread of GML execution.
pub struct Thread {
    returns: Vec<(Symbol, usize, usize)>,
    stack: Vec<Register>,

    self_entity: Entity,
    other_entity: Entity,
}

extern {
    type Engine;
}

/// A 64-bit stack slot for the VM.
///
/// The distinction between `value` and `value_ref` is entirely for readability, and bytecode often
/// writes a `value` but reads it as a `value_ref`. (See `Value` and `ValueRef` for justification.)
/// This enables loads to produce `ValueRef`s rather than cloning, while simultaneously using those
/// results as operands to many ops, with the usual borrowing rules upheld by codegen.
#[repr(C)]
union Register {
    /// An uninitialized register.
    uninit: (),

    /// An owned language-level value.
    value: ManuallyDrop<Value>,

    /// An intermediate result when loading from a scope or array.
    value_ref: ValueRef<'static>,
    /// An intermediate result when working with jagged arrays.
    row: ArrayRef<'static>,

    /// An entity id, resolved from an instance or other scope id.
    entity: Entity,
    /// A pointer into an array of entity ids.
    iterator: ptr::NonNull<Entity>,
}

impl Default for Register {
    fn default() -> Self { Register { uninit: () } }
}

pub struct Error {
    pub symbol: Symbol,
    pub instruction: usize,
    pub kind: ErrorKind,
}

#[derive(Debug)]
pub enum ErrorKind {
    /// Unary type error.
    TypeUnary(code::Op, Value),
    /// Binary type error.
    TypeBinary(code::Op, Value, Value),
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

impl Default for Thread {
    fn default() -> Self {
        Thread {
            returns: Vec::default(),
            stack: Vec::default(),

            self_entity: Entity(0),
            other_entity: Entity(0),
        }
    }
}

impl Thread {
    pub fn set_self(&mut self, entity: Entity) {
        self.self_entity = entity;
    }

    pub fn set_other(&mut self, entity: Entity) {
        self.other_entity = entity;
    }

    pub fn execute<E: world::Api>(
        &mut self,
        engine: &mut E, resources: &Resources<E>,
        symbol: Symbol, arguments: Vec<Value>
    ) -> Result<Value, Error> {
        let world = E::receivers(engine) as *mut _;
        let engine = unsafe { &mut *(engine as *mut _ as *mut Engine) };
        let resources = unsafe { &*(resources as *const _ as *const Resources<Engine>) };
        execute_internal(self, engine, world, resources, symbol, arguments)
    }
}

fn get_string(value: ValueRef<'_>) -> Symbol {
    match value.decode() {
        Data::String(value) => value,
        _ => unreachable!("expected a string"),
    }
}

fn execute_internal<'a>(
    thread: &mut Thread,
    engine: &'a mut Engine, world: *mut World, resources: &Resources<Engine>,
    symbol: Symbol, arguments: Vec<Value>
) -> Result<Value, Error> {
    // Enforce that `world` is treated as a reborrow of `engine`.
    fn constrain<F: for<'a> Fn(&'a mut Engine) -> &'a mut World>(f: F) -> F { f }
    let world = constrain(move |_| unsafe { &mut *world });

    // Erase the lifetime of a `ValueRef` for use in a `Register`.
    unsafe fn erase_ref(r: ValueRef<'_>) -> ValueRef<'static> { mem::transmute(r) }

    // Thread state not stored in `thread`:
    let mut symbol = symbol;
    let mut function = &resources.scripts[&symbol];
    let mut instruction = 0;
    let mut reg_base = thread.stack.len();

    // Don't initialize locals, the compiler handles that.
    thread.stack.resize_with(reg_base + function.locals as usize, Register::default);

    // Move the arguments onto the stack and initialize any additional parameters to 0.0.
    let registers = thread.stack[reg_base..][..function.params as usize].iter_mut();
    let arguments = arguments.into_iter().chain(iter::repeat_with(Value::default));
    for (reg, arg) in Iterator::zip(registers, arguments) {
        *reg = Register { value: ManuallyDrop::new(arg) };
    }

    let kind = loop {
        let registers = &mut thread.stack[reg_base..];

        match function.instructions[instruction].decode() {
            (code::Op::Imm, t, constant, _) => {
                let value = function.constants[constant].clone();
                registers[t] = Register { value: ManuallyDrop::new(value) };
            }

            (code::Op::Move, t, s, _) => {
                // Like a Rust move, this leaves the source place uninitialized.
                registers[t] = mem::take(&mut registers[s]);
            }

            (op @ code::Op::Neg, t, a, _) => {
                let a = unsafe { registers[a].value_ref };
                let value = match a.decode() {
                    Data::Real(a) => Value::from(-a),
                    _ => break ErrorKind::TypeUnary(op, a.clone()),
                };
                registers[t] = Register { value: ManuallyDrop::new(value) };
            }

            (op @ code::Op::Not, t, a, _) => {
                let a = unsafe { registers[a].value_ref };
                let value = match a.decode() {
                    Data::Real(a) => Value::from(!to_bool(a)),
                    _ => break ErrorKind::TypeUnary(op, a.clone()),
                };
                registers[t] = Register { value: ManuallyDrop::new(value) };
            }

            (op @ code::Op::BitNot, t, a, _) => {
                let a = unsafe { registers[a].value_ref };
                let value = match a.decode() {
                    Data::Real(a) => Value::from(!to_i32(a)),
                    _ => break ErrorKind::TypeUnary(op, a.clone()),
                };
                registers[t] = Register { value: ManuallyDrop::new(value) };
            }

            (op @ code::Op::Lt, t, a, b) => {
                let a = unsafe { registers[a].value_ref };
                let b = unsafe { registers[b].value_ref };
                let value = match (a.decode(), b.decode()) {
                    (Data::Real(a), Data::Real(b)) => Value::from(a < b),
                    (Data::String(a), Data::String(b)) => Value::from(a < b),
                    _ => break ErrorKind::TypeBinary(op, a.clone(), b.clone()),
                };
                registers[t] = Register { value: ManuallyDrop::new(value) };
            }

            (op @ code::Op::Le, t, a, b) => {
                let a = unsafe { registers[a].value_ref };
                let b = unsafe { registers[b].value_ref };
                let value = match (a.decode(), b.decode()) {
                    (Data::Real(a), Data::Real(b)) => Value::from(a <= b),
                    (Data::String(a), Data::String(b)) => Value::from(a <= b),
                    _ => break ErrorKind::TypeBinary(op, a.clone(), b.clone()),
                };
                registers[t] = Register { value: ManuallyDrop::new(value) };
            }

            (code::Op::Eq, t, a, b) => {
                let a = unsafe { registers[a].value_ref };
                let b = unsafe { registers[b].value_ref };
                let value = Value::from(a == b);
                registers[t] = Register { value: ManuallyDrop::new(value) };
            }

            (code::Op::Ne, t, a, b) => {
                let a = unsafe { registers[a].value_ref };
                let b = unsafe { registers[b].value_ref };
                let value = Value::from(a != b);
                registers[t] = Register { value: ManuallyDrop::new(value) };
            }

            (op @ code::Op::Ge, t, a, b) => {
                let a = unsafe { registers[a].value_ref };
                let b = unsafe { registers[b].value_ref };
                let value = match (a.decode(), b.decode()) {
                    (Data::Real(a), Data::Real(b)) => Value::from(a >= b),
                    (Data::String(a), Data::String(b)) => Value::from(a >= b),
                    _ => break ErrorKind::TypeBinary(op, a.clone(), b.clone()),
                };
                registers[t] = Register { value: ManuallyDrop::new(value) };
            }

            (op @ code::Op::Gt, t, a, b) => {
                let a = unsafe { registers[a].value_ref };
                let b = unsafe { registers[b].value_ref };
                let value = match (a.decode(), b.decode()) {
                    (Data::Real(a), Data::Real(b)) => Value::from(a > b),
                    (Data::String(a), Data::String(b)) => Value::from(a > b),
                    _ => break ErrorKind::TypeBinary(op, a.clone(), b.clone()),
                };
                registers[t] = Register { value: ManuallyDrop::new(value) };
            }

            (op @ code::Op::Add, t, a, b) => {
                let a = unsafe { registers[a].value_ref };
                let b = unsafe { registers[b].value_ref };
                let value = match (a.decode(), b.decode()) {
                    (Data::Real(a), Data::Real(b)) => Value::from(a + b),
                    (Data::String(a), Data::String(b)) =>
                        Value::from(Symbol::intern(&[a, b].concat())),
                    _ => break ErrorKind::TypeBinary(op, a.clone(), b.clone()),
                };
                registers[t] = Register { value: ManuallyDrop::new(value) };
            }

            (op @ code::Op::Sub, t, a, b) => {
                let a = unsafe { registers[a].value_ref };
                let b = unsafe { registers[b].value_ref };
                let value = match (a.decode(), b.decode()) {
                    (Data::Real(a), Data::Real(b)) => Value::from(a - b),
                    _ => break ErrorKind::TypeBinary(op, a.clone(), b.clone()),
                };
                registers[t] = Register { value: ManuallyDrop::new(value) };
            }

            (op @ code::Op::Mul, t, a, b) => {
                let a = unsafe { registers[a].value_ref };
                let b = unsafe { registers[b].value_ref };
                let value = match (a.decode(), b.decode()) {
                    (Data::Real(a), Data::Real(b)) => Value::from(a * b),
                    (Data::Real(a), Data::String(b)) =>
                        Value::from(Symbol::intern(&b.repeat(a as usize))),
                    _ => break ErrorKind::TypeBinary(op, a.clone(), b.clone()),
                };
                registers[t] = Register { value: ManuallyDrop::new(value) };
            }

            (op @ code::Op::Div, t, a, b) => {
                let a = unsafe { registers[a].value_ref };
                let b = unsafe { registers[b].value_ref };
                let value = match (a.decode(), b.decode()) {
                    (Data::Real(_), Data::Real(b)) if b == 0.0 => break ErrorKind::DivideByZero,
                    (Data::Real(a), Data::Real(b)) => Value::from(a / b),
                    _ => break ErrorKind::TypeBinary(op, a.clone(), b.clone()),
                };
                registers[t] = Register { value: ManuallyDrop::new(value) };
            }

            (op @ code::Op::IntDiv, t, a, b) => {
                let a = unsafe { registers[a].value_ref };
                let b = unsafe { registers[b].value_ref };
                let value = match (a.decode(), b.decode()) {
                    (Data::Real(_), Data::Real(b)) if b == 0.0 => break ErrorKind::DivideByZero,
                    (Data::Real(a), Data::Real(b)) => Value::from(to_i32(a / b)),
                    _ => break ErrorKind::TypeBinary(op, a.clone(), b.clone()),
                };
                registers[t] = Register { value: ManuallyDrop::new(value) };
            }

            (op @ code::Op::Mod, t, a, b) => {
                let a = unsafe { registers[a].value_ref };
                let b = unsafe { registers[b].value_ref };
                let value = match (a.decode(), b.decode()) {
                    (Data::Real(_), Data::Real(b)) if b == 0.0 => break ErrorKind::DivideByZero,
                    (Data::Real(a), Data::Real(b)) => Value::from(a % b),
                    _ => break ErrorKind::TypeBinary(op, a.clone(), b.clone()),
                };
                registers[t] = Register { value: ManuallyDrop::new(value) };
            }

            (op @ code::Op::And, t, a, b) => {
                let a = unsafe { registers[a].value_ref };
                let b = unsafe { registers[b].value_ref };
                let value = match (a.decode(), b.decode()) {
                    (Data::Real(a), Data::Real(b)) => Value::from(to_bool(a) && to_bool(b)),
                    _ => break ErrorKind::TypeBinary(op, a.clone(), b.clone()),
                };
                registers[t] = Register { value: ManuallyDrop::new(value) };
            }

            (op @ code::Op::Or, t, a, b) => {
                let a = unsafe { registers[a].value_ref };
                let b = unsafe { registers[b].value_ref };
                let value = match (a.decode(), b.decode()) {
                    (Data::Real(a), Data::Real(b)) => Value::from(to_bool(a) || to_bool(b)),
                    _ => break ErrorKind::TypeBinary(op, a.clone(), b.clone()),
                };
                registers[t] = Register { value: ManuallyDrop::new(value) };
            }

            (op @ code::Op::Xor, t, a, b) => {
                let a = unsafe { registers[a].value_ref };
                let b = unsafe { registers[b].value_ref };
                let value = match (a.decode(), b.decode()) {
                    (Data::Real(a), Data::Real(b)) => Value::from(to_bool(a) != to_bool(b)),
                    _ => break ErrorKind::TypeBinary(op, a.clone(), b.clone()),
                };
                registers[t] = Register { value: ManuallyDrop::new(value) };
            }

            (op @ code::Op::BitAnd, t, a, b) => {
                let a = unsafe { registers[a].value_ref };
                let b = unsafe { registers[b].value_ref };
                let value = match (a.decode(), b.decode()) {
                    (Data::Real(a), Data::Real(b)) => Value::from(to_i32(a) & to_i32(b)),
                    _ => break ErrorKind::TypeBinary(op, a.clone(), b.clone()),
                };
                registers[t] = Register { value: ManuallyDrop::new(value) };
            }

            (op @ code::Op::BitOr, t, a, b) => {
                let a = unsafe { registers[a].value_ref };
                let b = unsafe { registers[b].value_ref };
                let value = match (a.decode(), b.decode()) {
                    (Data::Real(a), Data::Real(b)) => Value::from(to_i32(a) | to_i32(b)),
                    _ => break ErrorKind::TypeBinary(op, a.clone(), b.clone()),
                };
                registers[t] = Register { value: ManuallyDrop::new(value) };
            }

            (op @ code::Op::BitXor, t, a, b) => {
                let a = unsafe { registers[a].value_ref };
                let b = unsafe { registers[b].value_ref };
                let value = match (a.decode(), b.decode()) {
                    (Data::Real(a), Data::Real(b)) => Value::from(to_i32(a) ^ to_i32(b)),
                    _ => break ErrorKind::TypeBinary(op, a.clone(), b.clone()),
                };
                registers[t] = Register { value: ManuallyDrop::new(value) };
            }

            (op @ code::Op::ShiftLeft, t, a, b) => {
                let a = unsafe { registers[a].value_ref };
                let b = unsafe { registers[b].value_ref };
                let value = match (a.decode(), b.decode()) {
                    (Data::Real(a), Data::Real(b)) => Value::from(to_i32(a) << to_i32(b)),
                    _ => break ErrorKind::TypeBinary(op, a.clone(), b.clone()),
                };
                registers[t] = Register { value: ManuallyDrop::new(value) };
            }

            (op @ code::Op::ShiftRight, t, a, b) => {
                let a = unsafe { registers[a].value_ref };
                let b = unsafe { registers[b].value_ref };
                let value = match (a.decode(), b.decode()) {
                    (Data::Real(a), Data::Real(b)) => Value::from(to_i32(a) >> to_i32(b)),
                    _ => break ErrorKind::TypeBinary(op, a.clone(), b.clone()),
                };
                registers[t] = Register { value: ManuallyDrop::new(value) };
            }

            (code::Op::DeclareGlobal, name, _, _) => {
                let name = get_string(function.constants[name].borrow());
                world(engine).globals.insert(name);

                let instance = &mut world(engine).members[world::GLOBAL];
                instance.entry(name).or_insert(Value::from(0.0));
            }

            (code::Op::Lookup, t, name, _) => {
                let name = get_string(function.constants[name].borrow());
                registers[t].entity = if world(engine).globals.contains(&name) {
                    world::GLOBAL
                } else {
                    thread.self_entity
                };
            }

            // TODO: Replace these with `thread`/`other` arguments and locals passed to `Lookup`.

            (code::Op::LoadScope, t, scope, _) => {
                registers[t].entity = match scope as i8 as i32 {
                    SELF => thread.self_entity,
                    OTHER => thread.other_entity,
                    GLOBAL => world::GLOBAL,
                    scope => break ErrorKind::Scope(scope),
                };
            }

            (code::Op::StoreScope, s, scope, _) => {
                let s = unsafe { registers[s].entity };
                match scope as i8 as i32 {
                    SELF => thread.self_entity = s,
                    OTHER => thread.other_entity = s,
                    scope => break ErrorKind::Scope(scope),
                }
            }

            (op @ code::Op::With, ptr, end, scope) => {
                let scope = unsafe { registers[scope].value_ref };
                let scope = match scope.decode() {
                    Data::Real(scope) => to_i32(scope),
                    _ => break ErrorKind::TypeUnary(op, scope.clone()),
                };

                let slice = match scope {
                    SELF => slice::from_ref(&thread.self_entity),
                    OTHER => slice::from_ref(&thread.other_entity),
                    ALL => world(engine).instances.values(),
                    NOONE => &[],
                    GLOBAL => slice::from_ref(&world::GLOBAL),
                    LOCAL => &[], // TODO: error
                    object if (0..=100_000).contains(&object) =>
                        &world(engine).objects[&object][..],
                    instance if (100_001..).contains(&instance) =>
                        slice::from_ref(&world(engine).instances[instance]),
                    _ => &[], // TODO: error
                };
                unsafe {
                    let first = slice.as_ptr() as *mut Entity;
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
                let value = Value::from(a != b);
                registers[t] = Register { value: ManuallyDrop::new(value) };
            }

            (code::Op::ExistsEntity, t, entity, _) => {
                let entity = unsafe { registers[entity].entity };
                let exists = world(engine).members.contains_key(entity);
                let value = Value::from(exists);
                registers[t] = Register { value: ManuallyDrop::new(value) };
            }

            // Despite its name, this opcode does not actually read any language-level values.
            // Instead it checks whether a local variable is initialized *before* it is read.
            (op @ code::Op::Read, a, local, _) => {
                let a = unsafe { registers[a].value_ref };
                match a.decode() {
                    Data::Real(a) => if !to_bool(a) {
                        let local = get_string(function.constants[local].borrow());
                        break ErrorKind::Name(local);
                    }
                    _ => break ErrorKind::TypeUnary(op, a.clone()),
                }
            }

            // This is a particularly inscrutable opcode, designed to handle pre-GM:S unindexed
            // writes to array variables: it overwrites `b` or `b[0, 0]` with `a.clone()` and moves
            // the result to `t`.
            (code::Op::Write, t, ai, bi) => {
                let a = unsafe { registers[ai].value_ref };
                let b = unsafe { registers[bi].value_ref };
                registers[t] = match b.decode() {
                    Data::Array(array) => {
                        array.set_jagged(0, 0, a.clone());
                        mem::take(&mut registers[bi])
                    }
                    _ => Register { value: ManuallyDrop::new(a.clone()) }
                };
            }

            (op @ code::Op::ScopeError, scope, _, _) => {
                let scope = unsafe { registers[scope].value_ref };
                match scope.decode() {
                    Data::Real(scope) => break ErrorKind::Scope(to_i32(scope)),
                    _ => break ErrorKind::TypeUnary(op, scope.clone()),
                }
            }

            (code::Op::ToArray, t, a, _) => {
                let a = unsafe { registers[a].value_ref };
                let value = match a.decode() {
                    Data::Array(_) => a.clone(),
                    _ => Value::from(Array::from_scalar(a.clone())),
                };
                registers[t] = Register { value: ManuallyDrop::new(value) };
            }

            (op @ code::Op::ToScalar, t, a, _) => {
                let a = unsafe { registers[a].value_ref };
                let value = match a.decode() {
                    Data::Array(array) => match array.get_jagged(0, 0) {
                        // The result of this operation *must* be a scalar to preserve GML
                        // semantics. This check guards against problems when mixing versions.
                        Some(a) => match a.borrow().decode() {
                            Data::Array(_) => break ErrorKind::TypeUnary(op, a),
                            _ => a,
                        }
                        None => break ErrorKind::Bounds(0),
                    }
                    // Because `a` is not an array, this clone is a simple copy.
                    _ => a.clone(),
                };
                registers[t] = Register { value: ManuallyDrop::new(value) };
            }

            (code::Op::Release, a, _, _) => unsafe {
                ManuallyDrop::drop(&mut registers[a].value);
            }

            (code::Op::LoadField, t, entity, field) => {
                let entity = unsafe { registers[entity].entity };
                let field = get_string(function.constants[field].borrow());
                let instance = &world(engine).members[entity];
                let value = match instance.get(&field) {
                    Some(value) => value.borrow(),
                    None => break ErrorKind::Name(field),
                };
                registers[t].value_ref = unsafe { erase_ref(value) };
            }

            (code::Op::LoadFieldDefault, t, entity, field) => {
                let entity = unsafe { registers[entity].entity };
                let field = get_string(function.constants[field].borrow());
                let instance = &world(engine).members[entity];
                let value = match instance.get(&field) {
                    Some(value) => value.borrow(),
                    None => ValueRef::default(),
                };
                registers[t].value_ref = unsafe { erase_ref(value) };
            }

            (op @ code::Op::LoadRow, t, a, i) => {
                let a = unsafe { registers[a].value_ref };
                let i = unsafe { registers[i].value_ref };
                let row = match (a.decode(), i.decode()) {
                    (Data::Array(array), Data::Real(i)) => match array.get_raw(to_i32(i)) {
                        // TODO: consider soundness here
                        Some(value) => match unsafe { (*value).borrow().decode() } {
                            Data::Array(array) => array,
                            _ => break ErrorKind::TypeUnary(op, a.clone()),
                        }
                        None => break ErrorKind::Bounds(to_i32(i)),
                    }
                    _ => break ErrorKind::TypeBinary(op, a.clone(), i.clone()),
                };
                registers[t].row = row;
            }

            (op @ code::Op::LoadIndex, t, r, j) => {
                let r = unsafe { registers[r].row };
                let j = unsafe { registers[j].value_ref };
                let value = match j.decode() {
                    Data::Real(j) => match r.get_raw(to_i32(j)) {
                        // TODO: consider soundness here
                        Some(value) => unsafe { (*value).borrow() }
                        None => break ErrorKind::Bounds(to_i32(j)),
                    }
                    _ => break ErrorKind::TypeBinary(op, Value::from(r.clone()), j.clone()),
                };
                registers[t].value_ref = unsafe { erase_ref(value) };
            }

            (code::Op::StoreField, s, entity, field) => {
                let s = unsafe { registers[s].value_ref };
                let entity = unsafe { registers[entity].entity };
                let field = get_string(function.constants[field].borrow());
                let instance = &mut world(engine).members[entity];
                instance.insert(field, s.clone());
            }

            (op @ code::Op::StoreRow, t, a, i) => {
                let a = unsafe { registers[a].value_ref };
                let i = unsafe { registers[i].value_ref };
                let row = match (a.decode(), i.decode()) {
                    (Data::Array(array), Data::Real(i)) => match array.set_raw_outer(to_i32(i)) {
                        // TODO: consider soundness here
                        Some(value) => match unsafe { (*value).borrow().decode() } {
                            Data::Array(array) => array,
                            _ => break ErrorKind::TypeUnary(op, a.clone())
                        }
                        None => break ErrorKind::Bounds(to_i32(i)),
                    }
                    _ => break ErrorKind::TypeBinary(op, a.clone(), i.clone()),
                };
                registers[t].row = row;
            }

            (op @ code::Op::StoreIndex, s, r, j) => {
                let s = unsafe { registers[s].value_ref };
                let r = unsafe { registers[r].row };
                let j = unsafe { registers[j].value_ref };
                match j.decode() {
                    Data::Real(j) => match r.set_flat(to_i32(j), s.clone()) {
                        Some(()) => {}
                        None => break ErrorKind::Bounds(to_i32(j)),
                    }
                    _ => break ErrorKind::TypeBinary(op, Value::from(r.clone()), j.clone()),
                }
            }

            (code::Op::Call, callee, base, len) => {
                thread.returns.push((symbol, instruction + 1, reg_base));

                symbol = get_string(function.constants[callee].borrow());
                function = &resources.scripts[&symbol];
                instruction = 0;
                reg_base = reg_base + base;

                let limit = cmp::max(function.locals as usize, len);
                thread.stack.resize_with(reg_base + limit, Register::default);

                let registers = thread.stack[reg_base..][..function.params as usize].iter_mut();
                for reg in registers.skip(len) {
                    *reg = Register { value: ManuallyDrop::new(Value::default()) };
                }

                continue;
            }

            (code::Op::CallApi, callee, base, len) => {
                let api_symbol = get_string(function.constants[callee].borrow());
                let function = resources.api[&api_symbol];
                let reg_base = reg_base + base;

                let registers = &mut thread.stack[reg_base..];
                let entity = thread.self_entity;
                let arguments = unsafe { mem::transmute::<_, &[Value]>(&registers[..len]) };
                let value = match function(engine, resources, entity, arguments) {
                    Ok(value) => value,
                    Err(kind) => break kind,
                };
                registers[0] = Register { value: ManuallyDrop::new(value) };
            }

            (code::Op::CallGet, get, base, _) => {
                let symbol = get_string(function.constants[get].borrow());
                let function = match resources.get.get(&symbol) {
                    Some(function) => function,
                    None => break ErrorKind::Name(symbol),
                };
                let reg_base = reg_base + base;

                let registers = &mut thread.stack[reg_base..];
                let entity = unsafe { registers[0].entity };
                let i = unsafe { registers[1].value_ref };
                let i = i32::try_from(i).unwrap_or(0) as usize;
                let value = function(engine, entity, i);
                registers[0] = Register { value: ManuallyDrop::new(value) };
            }

            (code::Op::CallSet, set, base, _) => {
                let symbol = get_string(function.constants[set].borrow());
                let function = match resources.set.get(&symbol) {
                    Some(function) => function,
                    None => break ErrorKind::Write(symbol),
                };
                let reg_base = reg_base + base;

                let registers = &thread.stack[reg_base..];
                let value = unsafe { registers[0].value_ref };
                let entity = unsafe { registers[1].entity };
                let i = unsafe { registers[2].value_ref };
                let i = i32::try_from(i).unwrap_or(0) as usize;
                function(engine, entity, i, value);
            }

            (code::Op::Ret, _, _, _) => {
                let (caller, caller_instruction, caller_base) = match thread.returns.pop() {
                    Some(frame) => frame,
                    None => {
                        let value = unsafe { registers[0].value_ref };
                        return Ok(value.clone());
                    }
                };

                symbol = caller;
                function = &resources.scripts[&symbol];
                instruction = caller_instruction;
                reg_base = caller_base;

                thread.stack.resize_with(reg_base + function.locals as usize, Register::default);

                continue;
            }

            (code::Op::Jump, t_low, t_high, _) => {
                instruction = t_low | (t_high << 8);
                continue;
            }

            (op @ code::Op::BranchFalse, a, t_low, t_high) => {
                let a = unsafe { registers[a].value_ref };
                match a.decode() {
                    Data::Real(a) => if !to_bool(a) {
                        instruction = t_low | (t_high << 8);
                        continue;
                    }
                    _ => break ErrorKind::TypeUnary(op, a.clone()),
                }
            }
        }

        instruction += 1;
    };

    Err(Error { symbol, instruction, kind })
}
