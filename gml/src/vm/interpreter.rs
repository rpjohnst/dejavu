use std::{mem, ptr, iter, slice, cmp, fmt, error};
use std::convert::TryFrom;
use std::mem::ManuallyDrop;
use std::ops::Range;

use crate::symbol::Symbol;
use crate::rc_vec::RcVec;
use crate::Function;
use crate::vm::{self, world, code};
use crate::vm::{World, Assets, Entity, Value, ValueRef, Data, Array, ArrayRef};
use crate::vm::{to_i32, to_bool};

/// A single thread of GML execution.
pub struct Thread {
    calls: Vec<(Function, usize, usize)>,
    withs: Vec<RcVec<Entity>>,
    stack: Vec<Register>,

    self_entity: Entity,
    other_entity: Entity,
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
    pub function: Function,
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
    /// Name in entity does not exist.
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
        write!(f, "{}+{}:{:?}", self.function, self.instruction, self.kind)
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
            calls: Vec::default(),
            withs: Vec::default(),
            stack: Vec::default(),

            self_entity: Entity(0),
            other_entity: Entity(0),
        }
    }
}

impl Thread {
    pub fn self_entity(&self) -> Entity { self.self_entity }
    pub fn set_self(&mut self, entity: Entity) { self.self_entity = entity; }

    pub fn set_other(&mut self, entity: Entity) { self.other_entity = entity; }

    /// Obtain the arguments to an API call.
    ///
    /// Safety: This function must be called from an API function,
    /// with the `Range` passed in from `Thread::execute`,
    /// before the API function does anything else with the `Thread`.
    pub unsafe fn arguments(&self, arguments: Range<usize>) -> &[Value] {
        mem::transmute(&self.stack[arguments])
    }

    pub fn execute<'a, W, A: 'a>(
        &mut self, world: &mut W, assets: &mut A, function: Function, arguments: Vec<Value>
    ) -> Result<Value, Error> where W: vm::Api<'a, A> {
        let engine = unsafe { &mut (
            &mut *(world as *mut _ as *mut engine::World),
            &mut *(assets as *mut _ as *mut engine::Assets),
        ) };
        let (world, assets) = vm::Api::fields(world, assets);
        let assets = assets as *mut _ as *mut _;
        execute_internal(self, engine, world, assets, function, arguments)
    }
}

fn get_string(value: ValueRef<'_>) -> Symbol {
    match value.decode() {
        Data::String(value) => value,
        _ => unreachable!("expected a string"),
    }
}

type Engine<'e> = (&'e mut engine::World, &'e mut engine::Assets);

// Opaque types to erase engine-side wrappers for `vm::World` and `vm::Assets`.
mod engine {
    extern {
        pub(super) type World;
        pub(super) type Assets;
    }
}

fn execute_internal(
    thread: &mut Thread,
    engine: &mut Engine<'_>, world: *mut World, assets: *mut Assets<engine::World, engine::Assets>,
    function: Function, arguments: Vec<Value>
) -> Result<Value, Error> {
    // Enforce that `vm::{World, Assets}` are treated as fields of `engine::{World, Assets}`.
    fn constrain<T, F: for<'e> Fn(&mut Engine<'e>) -> &'e mut T>(f: F) -> F { f }
    let world = constrain(move |_| unsafe { &mut *world });
    let assets = constrain(move |_| unsafe { &mut *assets });

    // Erase the lifetime of a `ValueRef` for use in a `Register`.
    unsafe fn erase_ref(r: ValueRef<'_>) -> ValueRef<'static> { mem::transmute(r) }

    // Thread state not stored in `thread`:
    let mut function = function;
    let mut code = match function {
        Function::Script(symbol) => &assets(engine).scripts[&symbol],
        Function::Event(event) => &assets(engine).events[&event],
    };
    let mut instruction = 0;
    let mut reg_base = thread.stack.len();

    // Don't initialize locals, the compiler handles that.
    thread.stack.resize_with(reg_base + code.locals as usize, Register::default);

    // Move the arguments onto the stack and initialize any additional parameters to 0.0.
    let registers = thread.stack[reg_base..][..code.params as usize].iter_mut();
    let arguments = arguments.into_iter().chain(iter::repeat_with(Value::default));
    for (reg, arg) in Iterator::zip(registers, arguments) {
        *reg = Register { value: ManuallyDrop::new(arg) };
    }

    let kind = loop {
        let registers = &mut thread.stack[reg_base..];

        match code.instructions[instruction].decode() {
            (code::Op::Imm, t, constant, _) => {
                let value = code.constants[constant].clone();
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
                let name = get_string(code.constants[name].borrow());
                world(engine).globals.insert(name);

                let instance = &mut world(engine).members[world::GLOBAL];
                instance.entry(name).or_insert(Value::from(0.0));
            }

            (code::Op::Lookup, t, name, _) => {
                let name = get_string(code.constants[name].borrow());
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

                let mut values = RcVec::default();
                let slice = match scope {
                    SELF => slice::from_ref(&thread.self_entity),
                    OTHER => slice::from_ref(&thread.other_entity),
                    ALL => {
                        values = world(engine).instances.values().clone();
                        &values[..]
                    }
                    NOONE => &[],
                    GLOBAL => slice::from_ref(&world::GLOBAL),
                    LOCAL => &[], // TODO: error
                    object if (0..=100_000).contains(&object) => {
                        values = world(engine).objects[&object].clone();
                        &values[..]
                    }
                    instance if (100_001..).contains(&instance) =>
                        slice::from_ref(&world(engine).instances[instance]),
                    _ => &[], // TODO: error
                };

                // Safety: There are two cases to consider here:
                // - The iterator points to a single element. It will be dereferenced only once,
                //   before any operations that can invalidate it.
                // - The iterator points into an `RcVec` of entities. The `RcVec`'s refcount is
                //   incremented above, and decremented by `ReleaseWith`. Any attempt to mutate the
                //   `RcVec` in between will make a copy of the array, and mutate the copy instead.
                unsafe {
                    let first = slice.as_ptr() as *mut Entity;
                    let last = first.offset(slice.len() as isize);
                    registers[ptr].iterator = ptr::NonNull::new_unchecked(first);
                    registers[end].iterator = ptr::NonNull::new_unchecked(last);
                }

                thread.withs.push(values);
            }

            (code::Op::ReleaseWith, _, _, _) => {
                thread.withs.pop();
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
                        let local = get_string(code.constants[local].borrow());
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
                let field = get_string(code.constants[field].borrow());
                let instance = &world(engine).members[entity];
                let value = match instance.get(&field) {
                    Some(value) => value.borrow(),
                    None => break ErrorKind::Name(field),
                };
                registers[t].value_ref = unsafe { erase_ref(value) };
            }

            (code::Op::LoadFieldDefault, t, entity, field) => {
                let entity = unsafe { registers[entity].entity };
                let field = get_string(code.constants[field].borrow());
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
                let field = get_string(code.constants[field].borrow());
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
                thread.calls.push((function, instruction + 1, reg_base));

                let id = callee as i32;
                function = Function::Script(id);
                code = &assets(engine).scripts[&id];
                instruction = 0;
                reg_base = reg_base + base;

                let limit = cmp::max(code.locals as usize, len);
                thread.stack.resize_with(reg_base + limit, Register::default);

                let registers = thread.stack[reg_base..][..code.params as usize].iter_mut();
                for reg in registers.skip(len) {
                    *reg = Register { value: ManuallyDrop::new(Value::default()) };
                }

                continue;
            }

            (code::Op::CallApi, callee, base, len) => {
                let symbol = get_string(code.constants[callee].borrow());
                let api = assets(engine).api[&symbol];
                let reg_base = reg_base + base;

                let value = unsafe {
                    let (world, assets) = engine;
                    let arguments = reg_base..reg_base + len;
                    match api(world, assets, thread, arguments) {
                        Ok(value) => value,
                        Err(kind) => break kind,
                    }
                };

                // The call above may have mutated our `vm::Assets`.
                // Reload the function body just in case. (This also keeps borrowck happy.)
                code = match function {
                    Function::Script(symbol) => &assets(engine).scripts[&symbol],
                    Function::Event(event) => &assets(engine).events[&event],
                };

                let registers = &mut thread.stack[reg_base..];
                registers[0] = Register { value: ManuallyDrop::new(value) };
            }

            (code::Op::CallGet, get, base, _) => {
                let symbol = get_string(code.constants[get].borrow());
                let get = match assets(engine).get.get(&symbol) {
                    Some(get) => get,
                    None => break ErrorKind::Name(symbol),
                };
                let reg_base = reg_base + base;

                let registers = &mut thread.stack[reg_base..];
                let value = {
                    let (world, assets) = engine;
                    let entity = unsafe { registers[0].entity };
                    let i = unsafe { registers[1].value_ref };
                    let i = i32::try_from(i).unwrap_or(0) as usize;
                    get(world, assets, entity, i)
                };

                // The call above may have mutated our `vm::Assets`.
                // Reload the function body just in case. (This also keeps borrowck happy.)
                code = match function {
                    Function::Script(symbol) => &assets(engine).scripts[&symbol],
                    Function::Event(event) => &assets(engine).events[&event],
                };

                registers[0] = Register { value: ManuallyDrop::new(value) };
            }

            (code::Op::CallSet, set, base, _) => {
                let symbol = get_string(code.constants[set].borrow());
                let set = match assets(engine).set.get(&symbol) {
                    Some(set) => set,
                    None => break ErrorKind::Write(symbol),
                };
                let reg_base = reg_base + base;

                let registers = &thread.stack[reg_base..];
                {
                    let (world, assets) = engine;
                    let value = unsafe { registers[0].value_ref };
                    let entity = unsafe { registers[1].entity };
                    let i = unsafe { registers[2].value_ref };
                    let i = i32::try_from(i).unwrap_or(0) as usize;
                    set(world, assets, entity, i, value);
                }

                // The call above may have mutated our `vm::Assets`.
                // Reload the function body just in case. (This also keeps borrowck happy.)
                code = match function {
                    Function::Script(symbol) => &assets(engine).scripts[&symbol],
                    Function::Event(event) => &assets(engine).events[&event],
                };
            }

            (code::Op::Ret, _, _, _) => {
                let (caller, caller_instruction, caller_base) = match thread.calls.pop() {
                    Some(frame) => frame,
                    None => {
                        let value = unsafe { registers[0].value_ref };
                        return Ok(value.clone());
                    }
                };

                function = caller;
                code = match function {
                    Function::Script(symbol) => &assets(engine).scripts[&symbol],
                    Function::Event(event) => &assets(engine).events[&event],
                };
                instruction = caller_instruction;
                reg_base = caller_base;

                thread.stack.resize_with(reg_base + code.locals as usize, Register::default);

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

    Err(Error { function, instruction, kind })
}
