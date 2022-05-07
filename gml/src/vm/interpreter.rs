use std::{mem, ptr, iter, slice, cmp, fmt, error};
use std::convert::TryFrom;
use std::ops::{self, Range};

use crate::symbol::Symbol;
use crate::rc_vec::RcVec;
use crate::Function;
use crate::vm::{self, world, code};
use crate::vm::{World, Assets, Entity, Value, ValueRef, Data, Array, ArrayRef};
use crate::vm::{to_i32, to_bool};

/// A single thread of GML execution.
pub struct Thread {
    calls: Vec<(Function, usize, usize, usize)>,
    withs: Vec<RcVec<Entity>>,
    owned: Vec<Value>,
    stack: Vec<Register>,

    self_entity: Entity,
    other_entity: Entity,
}

/// A 64-bit stack slot for the VM.
#[repr(C)]
union Register {
    /// An uninitialized register.
    uninit: (),

    /// A language-level value, borrowed either from `thread.owned` or a scope.
    /// (Borrows from scopes must not live across operations that might drop the value.)
    value: ValueRef<'static>,
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

pub type Result<T> = std::result::Result<T, Box<Error>>;

pub struct Error {
    pub frames: Vec<ErrorFrame>,
    pub kind: ErrorKind,
}

pub struct ErrorFrame {
    pub function: Function,
    pub instruction: usize,
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

impl Error {
    pub fn type_unary(op: code::Op, a: Value) -> Box<Error> {
        Box::from(ErrorKind::TypeUnary(op, a))
    }
    pub fn type_binary(op: code::Op, a: Value, b: Value) -> Box<Error> {
        Box::from(ErrorKind::TypeBinary(op, a, b))
    }
    pub fn divide_by_zero() -> Box<Error> { Box::from(ErrorKind::DivideByZero) }
    pub fn arity(arity: usize) -> Box<Error> { Box::from(ErrorKind::Arity(arity)) }
    pub fn scope(scope: i32) -> Box<Error> { Box::from(ErrorKind::Scope(scope)) }
    pub fn name(name: Symbol) -> Box<Error> { Box::from(ErrorKind::Name(name)) }
    pub fn write(name: Symbol) -> Box<Error> { Box::from(ErrorKind::Write(name)) }
    pub fn bounds(index: i32) -> Box<Error> { Box::from(ErrorKind::Bounds(index)) }
}

impl<E: 'static + error::Error> From<E> for Box<Error> {
    fn from(error: E) -> Box<Error> {
        let kind = ErrorKind::Other(Box::new(error));
        Self::from(kind)
    }
}

impl From<ErrorKind> for Box<Error> {
    fn from(kind: ErrorKind) -> Box<Error> {
        Box::new(Error { frames: Vec::default(), kind })
    }
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "error: {}", self.kind)?;
        for &ErrorFrame { ref function, instruction } in self.frames.iter() {
            writeln!(f, "  {:?}+{}", function, instruction)?;
        }
        Ok(())
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

/// Only push a new array result onto the owned-value stack.
pub const PUSH_ARRAY: i32 = 0;
/// Push any result onto the owned-value stack.
pub const PUSH_ANY: i32 = 1;

impl Default for Thread {
    fn default() -> Self {
        Thread {
            calls: Vec::default(),
            withs: Vec::default(),
            owned: Vec::default(),
            stack: Vec::default(),

            self_entity: Entity::NULL,
            other_entity: Entity::NULL,
        }
    }
}

pub struct SelfGuard<'a> {
    thread: &'a mut Thread,
    other: Entity,
}

impl ops::Deref for SelfGuard<'_> {
    type Target = Thread;
    fn deref(&self) -> &Thread { self.thread }
}

impl ops::DerefMut for SelfGuard<'_> {
    fn deref_mut(&mut self) -> &mut Thread { self.thread }
}

impl Drop for SelfGuard<'_> {
    fn drop(&mut self) { self.thread.self_entity = self.other; }
}

impl Thread {
    pub fn self_entity(&self) -> Entity { self.self_entity }

    pub fn with(&mut self, entity: Entity) -> SelfGuard<'_> {
        let other = mem::replace(&mut self.self_entity, entity);
        SelfGuard { thread: self, other }
    }

    /// Obtain the arguments to an API call.
    ///
    /// Safety: This function must be called from an API function,
    /// with the `Range` passed in from `Thread::execute`,
    /// before the API function does anything else with the `Thread`.
    pub unsafe fn arguments(&self, arguments: Range<usize>) -> &[Value] {
        mem::transmute(&self.stack[arguments])
    }

    pub fn execute<W: for<'r> vm::Project<'r, (&'r mut World, &'r mut Assets<W>)>>(
        &mut self, cx: &mut W, f: Function, args: Vec<Value>
    ) -> Result<Value> {
        let cx: &mut dyn vm::Project<(&mut World, &mut Assets<W>)> = cx;
        let cx = unsafe { &mut *(cx as *mut _ as *mut _) };
        execute_internal(self, cx, f, args)
    }
}

fn get_string(value: ValueRef<'_>) -> Symbol {
    match value.decode() {
        Data::String(value) => value,
        _ => unreachable!("expected a string"),
    }
}

// Opaque type to erase the runner-side container for `vm::World` and `vm::Assets`.
extern { type W; }

fn execute_internal(
    thread: &mut Thread, cx: &mut dyn for<'r> vm::Project<'r, (&'r mut World, &'r mut Assets<W>)>,
    function: Function, arguments: Vec<Value>
) -> Result<Value> {
    let (mut world, mut assets) = cx.fields();

    // Erase the lifetime of a `ValueRef` for use in a `Register`.
    unsafe fn erase_ref(r: ValueRef<'_>) -> ValueRef<'static> { mem::transmute(r) }

    // Thread state to restore on error:
    let orig_calls = thread.calls.len();
    let orig_withs = thread.withs.len();
    let orig_owned = thread.owned.len();
    let orig_stack = thread.stack.len();

    // Thread state not stored in `thread`:
    let mut function = function;
    let mut code = &assets.code[&function];
    let mut instruction = 0;
    let mut reg_base = thread.stack.len();

    // Don't initialize locals, the compiler handles that.
    thread.stack.resize_with(reg_base + code.locals as usize, Register::default);

    // Move the arguments onto the stack and initialize any additional parameters to 0.0.
    thread.owned.extend(arguments);
    let registers = thread.stack[reg_base..][..code.params as usize].iter_mut();
    let arguments = thread.owned.iter()
        .map(Value::borrow)
        .chain(iter::repeat_with(ValueRef::default));
    for (reg, arg) in Iterator::zip(registers, arguments) {
        reg.value = unsafe { erase_ref(arg) };
    }

    let mut error = loop {
        let registers = &mut thread.stack[reg_base..];

        match code.instructions[instruction].decode() {
            (code::Op::Const, t, constant, _) => {
                // Safety: Immediates must be reals or strings, which are never freed.
                registers[t].value = unsafe { erase_ref(code.constants[constant].borrow()) };
            }

            (code::Op::GlobalConst, t, constant, _) => {
                // TODO: do GMS arrays co-exist with non-macro constants in any version?
                // Safety: Constants must be reals or strings, which are never freed.
                registers[t].value = unsafe { erase_ref(world.constants[constant].borrow()) };
            }

            (code::Op::Move, t, s, _) => {
                // Like a Rust move, this leaves the source place uninitialized.
                registers[t] = mem::take(&mut registers[s]);
            }

            (op @ code::Op::Neg, t, a, _) => {
                let a = unsafe { registers[a].value };
                registers[t].value = match a.decode() {
                    Data::Real(a) => ValueRef::from(-a),
                    _ => break Error::type_unary(op, a.clone()),
                };
            }

            (op @ code::Op::Not, t, a, _) => {
                let a = unsafe { registers[a].value };
                registers[t].value = match a.decode() {
                    Data::Real(a) => ValueRef::from(!to_bool(a)),
                    _ => break Error::type_unary(op, a.clone()),
                };
            }

            (op @ code::Op::BitNot, t, a, _) => {
                let a = unsafe { registers[a].value };
                registers[t].value = match a.decode() {
                    Data::Real(a) => ValueRef::from(!to_i32(a)),
                    _ => break Error::type_unary(op, a.clone()),
                };
            }

            (op @ code::Op::Lt, t, a, b) => {
                let a = unsafe { registers[a].value };
                let b = unsafe { registers[b].value };
                registers[t].value = match (a.decode(), b.decode()) {
                    (Data::Real(a), Data::Real(b)) => ValueRef::from(a < b),
                    (Data::String(a), Data::String(b)) => ValueRef::from(a < b),
                    _ => break Error::type_binary(op, a.clone(), b.clone()),
                };
            }

            (op @ code::Op::Le, t, a, b) => {
                let a = unsafe { registers[a].value };
                let b = unsafe { registers[b].value };
                registers[t].value = match (a.decode(), b.decode()) {
                    (Data::Real(a), Data::Real(b)) => ValueRef::from(a <= b),
                    (Data::String(a), Data::String(b)) => ValueRef::from(a <= b),
                    _ => break Error::type_binary(op, a.clone(), b.clone()),
                };
            }

            (code::Op::Eq, t, a, b) => {
                let a = unsafe { registers[a].value };
                let b = unsafe { registers[b].value };
                registers[t].value = ValueRef::from(a == b);
            }

            (code::Op::Ne, t, a, b) => {
                let a = unsafe { registers[a].value };
                let b = unsafe { registers[b].value };
                registers[t].value = ValueRef::from(a != b);
            }

            (op @ code::Op::Ge, t, a, b) => {
                let a = unsafe { registers[a].value };
                let b = unsafe { registers[b].value };
                registers[t].value = match (a.decode(), b.decode()) {
                    (Data::Real(a), Data::Real(b)) => ValueRef::from(a >= b),
                    (Data::String(a), Data::String(b)) => ValueRef::from(a >= b),
                    _ => break Error::type_binary(op, a.clone(), b.clone()),
                };
            }

            (op @ code::Op::Gt, t, a, b) => {
                let a = unsafe { registers[a].value };
                let b = unsafe { registers[b].value };
                registers[t].value = match (a.decode(), b.decode()) {
                    (Data::Real(a), Data::Real(b)) => ValueRef::from(a > b),
                    (Data::String(a), Data::String(b)) => ValueRef::from(a > b),
                    _ => break Error::type_binary(op, a.clone(), b.clone()),
                };
            }

            (op @ code::Op::Add, t, a, b) => {
                let a = unsafe { registers[a].value };
                let b = unsafe { registers[b].value };
                registers[t].value = match (a.decode(), b.decode()) {
                    (Data::Real(a), Data::Real(b)) => ValueRef::from(a + b),
                    (Data::String(a), Data::String(b)) =>
                        ValueRef::from(Symbol::intern(&[a, b].concat())),
                    _ => break Error::type_binary(op, a.clone(), b.clone()),
                };
            }

            (op @ code::Op::Sub, t, a, b) => {
                let a = unsafe { registers[a].value };
                let b = unsafe { registers[b].value };
                registers[t].value = match (a.decode(), b.decode()) {
                    (Data::Real(a), Data::Real(b)) => ValueRef::from(a - b),
                    _ => break Error::type_binary(op, a.clone(), b.clone()),
                };
            }

            (op @ code::Op::Mul, t, a, b) => {
                let a = unsafe { registers[a].value };
                let b = unsafe { registers[b].value };
                registers[t].value = match (a.decode(), b.decode()) {
                    (Data::Real(a), Data::Real(b)) => ValueRef::from(a * b),
                    (Data::Real(a), Data::String(b)) =>
                        ValueRef::from(Symbol::intern(&b.repeat(a as usize))),
                    _ => break Error::type_binary(op, a.clone(), b.clone()),
                };
            }

            (op @ code::Op::Div, t, a, b) => {
                let a = unsafe { registers[a].value };
                let b = unsafe { registers[b].value };
                registers[t].value = match (a.decode(), b.decode()) {
                    (Data::Real(_), Data::Real(b)) if b == 0.0 => break Error::divide_by_zero(),
                    (Data::Real(a), Data::Real(b)) => ValueRef::from(a / b),
                    _ => break Error::type_binary(op, a.clone(), b.clone()),
                };
            }

            (op @ code::Op::IntDiv, t, a, b) => {
                let a = unsafe { registers[a].value };
                let b = unsafe { registers[b].value };
                registers[t].value = match (a.decode(), b.decode()) {
                    (Data::Real(_), Data::Real(b)) if b == 0.0 => break Error::divide_by_zero(),
                    (Data::Real(a), Data::Real(b)) => ValueRef::from(to_i32(a / b)),
                    _ => break Error::type_binary(op, a.clone(), b.clone()),
                };
            }

            (op @ code::Op::Mod, t, a, b) => {
                let a = unsafe { registers[a].value };
                let b = unsafe { registers[b].value };
                registers[t].value = match (a.decode(), b.decode()) {
                    (Data::Real(_), Data::Real(b)) if b == 0.0 => break Error::divide_by_zero(),
                    (Data::Real(a), Data::Real(b)) => ValueRef::from(a % b),
                    _ => break Error::type_binary(op, a.clone(), b.clone()),
                };
            }

            (op @ code::Op::And, t, a, b) => {
                let a = unsafe { registers[a].value };
                let b = unsafe { registers[b].value };
                registers[t].value = match (a.decode(), b.decode()) {
                    (Data::Real(a), Data::Real(b)) => ValueRef::from(to_bool(a) && to_bool(b)),
                    _ => break Error::type_binary(op, a.clone(), b.clone()),
                };
            }

            (op @ code::Op::Or, t, a, b) => {
                let a = unsafe { registers[a].value };
                let b = unsafe { registers[b].value };
                registers[t].value = match (a.decode(), b.decode()) {
                    (Data::Real(a), Data::Real(b)) => ValueRef::from(to_bool(a) || to_bool(b)),
                    _ => break Error::type_binary(op, a.clone(), b.clone()),
                };
            }

            (op @ code::Op::Xor, t, a, b) => {
                let a = unsafe { registers[a].value };
                let b = unsafe { registers[b].value };
                registers[t].value = match (a.decode(), b.decode()) {
                    (Data::Real(a), Data::Real(b)) => ValueRef::from(to_bool(a) != to_bool(b)),
                    _ => break Error::type_binary(op, a.clone(), b.clone()),
                };
            }

            (op @ code::Op::BitAnd, t, a, b) => {
                let a = unsafe { registers[a].value };
                let b = unsafe { registers[b].value };
                registers[t].value = match (a.decode(), b.decode()) {
                    (Data::Real(a), Data::Real(b)) => ValueRef::from(to_i32(a) & to_i32(b)),
                    _ => break Error::type_binary(op, a.clone(), b.clone()),
                };
            }

            (op @ code::Op::BitOr, t, a, b) => {
                let a = unsafe { registers[a].value };
                let b = unsafe { registers[b].value };
                registers[t].value = match (a.decode(), b.decode()) {
                    (Data::Real(a), Data::Real(b)) => ValueRef::from(to_i32(a) | to_i32(b)),
                    _ => break Error::type_binary(op, a.clone(), b.clone()),
                };
            }

            (op @ code::Op::BitXor, t, a, b) => {
                let a = unsafe { registers[a].value };
                let b = unsafe { registers[b].value };
                registers[t].value = match (a.decode(), b.decode()) {
                    (Data::Real(a), Data::Real(b)) => ValueRef::from(to_i32(a) ^ to_i32(b)),
                    _ => break Error::type_binary(op, a.clone(), b.clone()),
                };
            }

            (op @ code::Op::ShiftLeft, t, a, b) => {
                let a = unsafe { registers[a].value };
                let b = unsafe { registers[b].value };
                registers[t].value = match (a.decode(), b.decode()) {
                    (Data::Real(a), Data::Real(b)) => ValueRef::from(to_i32(a) << to_i32(b)),
                    _ => break Error::type_binary(op, a.clone(), b.clone()),
                };
            }

            (op @ code::Op::ShiftRight, t, a, b) => {
                let a = unsafe { registers[a].value };
                let b = unsafe { registers[b].value };
                registers[t].value = match (a.decode(), b.decode()) {
                    (Data::Real(a), Data::Real(b)) => ValueRef::from(to_i32(a) >> to_i32(b)),
                    _ => break Error::type_binary(op, a.clone(), b.clone()),
                };
            }

            (code::Op::DeclareGlobal, name, _, _) => {
                let name = get_string(code.constants[name].borrow());
                world.globals.insert(name);

                let instance = &mut world.members[world::GLOBAL];
                instance.entry(name).or_insert(Value::from(0.0));
            }

            (code::Op::Lookup, t, name, _) => {
                let name = get_string(code.constants[name].borrow());
                registers[t].entity = if world.globals.contains(&name) {
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
                    scope => break Error::scope(scope),
                };
            }

            (code::Op::StoreScope, s, scope, _) => {
                let s = unsafe { registers[s].entity };
                match scope as i8 as i32 {
                    SELF => thread.self_entity = s,
                    OTHER => thread.other_entity = s,
                    scope => break Error::scope(scope),
                }
            }

            (op @ code::Op::With, ptr, end, scope) => {
                let scope = unsafe { registers[scope].value };
                let scope = match scope.decode() {
                    Data::Real(scope) => to_i32(scope),
                    _ => break Error::type_unary(op, scope.clone()),
                };

                let mut values = RcVec::default();
                let slice = match scope {
                    SELF => slice::from_ref(&thread.self_entity),
                    OTHER => slice::from_ref(&thread.other_entity),
                    ALL => {
                        values = world.instances.values().clone();
                        &values[..]
                    }
                    NOONE => &[],
                    GLOBAL => slice::from_ref(&world::GLOBAL),
                    LOCAL => &[], // TODO: error
                    object if (0..=100_000).contains(&object) => {
                        values = world.objects[&object].clone();
                        &values[..]
                    }
                    instance if (100_001..).contains(&instance) =>
                        slice::from_ref(&world.instances[instance]),
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
                registers[t].value = ValueRef::from(a != b);
            }

            (code::Op::ExistsEntity, t, entity, _) => {
                let entity = unsafe { registers[entity].entity };
                let exists = world.members.contains_key(entity);
                registers[t].value = ValueRef::from(exists);
            }

            // Despite its name, this opcode does not actually read any language-level values.
            // Instead it checks whether a local variable is initialized *before* it is read.
            (op @ code::Op::Read, a, local, _) => {
                let a = unsafe { registers[a].value };
                match a.decode() {
                    Data::Real(a) => if !to_bool(a) {
                        let local = get_string(code.constants[local].borrow());
                        break Error::name(local);
                    }
                    _ => break Error::type_unary(op, a.clone()),
                }
            }

            // This is a particularly inscrutable opcode, designed to handle pre-GM:S unindexed
            // writes to array variables: it overwrites `b` or `b[0, 0]` with `a` and moves
            // the result to `t`.
            (code::Op::Write, t, a, b) => {
                let a = unsafe { registers[a].value };
                let b = unsafe { registers[b].value };
                registers[t].value = match b.decode() {
                    Data::Array(array) => {
                        array.set_jagged(0, 0, a.clone());
                        b
                    }
                    _ => a,
                };
            }

            (op @ code::Op::ScopeError, scope, _, _) => {
                let scope = unsafe { registers[scope].value };
                match scope.decode() {
                    Data::Real(scope) => break Error::scope(to_i32(scope)),
                    _ => break Error::type_unary(op, scope.clone()),
                }
            }

            (code::Op::ToArray, t, a, p) => {
                let a = unsafe { registers[a].value };
                registers[t].value = match a.decode() {
                    Data::Array(_) => {
                        if p as i32 == PUSH_ANY {
                            thread.owned.push(a.clone());
                        }
                        a
                    }
                    _ => {
                        let array = Value::from(Array::from_scalar(a.clone()));
                        let value = unsafe { erase_ref(array.borrow()) };
                        thread.owned.push(array);
                        value
                    }
                };
            }

            (op @ code::Op::ToScalar, t, a, _) => {
                let a = unsafe { registers[a].value };
                registers[t].value = match a.decode() {
                    Data::Array(array) => match array.get_jagged(0, 0) {
                        // The result of this operation *must* be a scalar to preserve GML
                        // semantics. This check guards against problems when mixing versions.
                        Some(a) => match a.borrow().decode() {
                            Data::Array(_) => break Error::type_unary(op, a),
                            // Because `a` is a scalar, this does not actually leak anything.
                            _ => a.leak(),
                        }
                        None => break Error::bounds(0),
                    }
                    _ => a,
                };
            }

            (code::Op::ReleaseOwned, _, _, _) => {
                thread.owned.pop();
            }

            (code::Op::LoadField, t, entity, field) => {
                let entity = unsafe { registers[entity].entity };
                let field = get_string(code.constants[field].borrow());
                let instance = &world.members[entity];
                registers[t].value = match instance.get(&field) {
                    Some(value) => unsafe { erase_ref(value.borrow()) },
                    None => break Error::name(field),
                };
            }

            (code::Op::LoadFieldDefault, t, entity, field) => {
                let entity = unsafe { registers[entity].entity };
                let field = get_string(code.constants[field].borrow());
                let instance = &world.members[entity];
                registers[t].value = match instance.get(&field) {
                    Some(value) => unsafe { erase_ref(value.borrow()) },
                    None => ValueRef::default(),
                };
            }

            (op @ code::Op::LoadRow, t, a, i) => {
                let a = unsafe { registers[a].value };
                let i = unsafe { registers[i].value };
                registers[t].row = match (a.decode(), i.decode()) {
                    (Data::Array(array), Data::Real(i)) => match array.get_raw(to_i32(i)) {
                        // Safety: Codegen always follows this with `LoadIndex` (see `get_jagged`).
                        Some(value) => match unsafe { (*value).borrow().decode() } {
                            Data::Array(array) => array,
                            _ => break Error::type_unary(op, a.clone()),
                        }
                        None => break Error::bounds(to_i32(i)),
                    }
                    _ => break Error::type_binary(op, a.clone(), i.clone()),
                };
            }

            (op @ code::Op::LoadIndex, t, r, j) => {
                let r = unsafe { registers[r].row };
                let j = unsafe { registers[j].value };
                registers[t].value = match j.decode() {
                    Data::Real(j) => match r.get_raw(to_i32(j)) {
                        // Safety: Codegen always precedes this with `LoadRow` (see `get_jagged`).
                        Some(value) => unsafe { erase_ref((*value).borrow()) }
                        None => break Error::bounds(to_i32(j)),
                    }
                    _ => break Error::type_binary(op, Value::from(r.clone()), j.clone()),
                };
            }

            (code::Op::StoreField, s, entity, field) => {
                let s = unsafe { registers[s].value };
                let entity = unsafe { registers[entity].entity };
                let field = get_string(code.constants[field].borrow());
                let instance = &mut world.members[entity];
                instance.insert(field, s.clone());
            }

            (op @ code::Op::StoreRow, t, a, i) => {
                let a = unsafe { registers[a].value };
                let i = unsafe { registers[i].value };
                registers[t].row = match (a.decode(), i.decode()) {
                    (Data::Array(array), Data::Real(i)) => match array.set_raw_outer(to_i32(i)) {
                        // Safety: Codegen always follows this with `StoreIndex` (see `set_jagged`).
                        Some(value) => match unsafe { (*value).borrow().decode() } {
                            Data::Array(array) => array,
                            _ => break Error::type_unary(op, a.clone())
                        }
                        None => break Error::bounds(to_i32(i)),
                    }
                    _ => break Error::type_binary(op, a.clone(), i.clone()),
                };
            }

            (op @ code::Op::StoreIndex, s, r, j) => {
                let s = unsafe { registers[s].value };
                let r = unsafe { registers[r].row };
                let j = unsafe { registers[j].value };
                match j.decode() {
                    Data::Real(j) => match r.set_flat(to_i32(j), s.clone()) {
                        Some(()) => {}
                        None => break Error::bounds(to_i32(j)),
                    }
                    _ => break Error::type_binary(op, Value::from(r.clone()), j.clone()),
                }
            }

            (code::Op::Call, callee, base, len) => {
                thread.calls.push((function, instruction + 1, reg_base, thread.owned.len()));

                let id = callee as i32;
                function = Function::Script { id };
                code = &assets.code[&function];
                instruction = 0;
                reg_base = reg_base + base;

                let limit = cmp::max(code.locals as usize, len);
                thread.stack.resize_with(reg_base + limit, Register::default);

                let registers = thread.stack[reg_base..][..code.params as usize].iter_mut();
                for reg in registers.skip(len) {
                    reg.value = ValueRef::default();
                }

                continue;
            }

            (code::Op::Ret, _, _, _) => {
                let array = unsafe { registers[0].value.clone() };
                if thread.calls.len() == orig_calls {
                    thread.calls.truncate(orig_calls);
                    thread.withs.truncate(orig_withs);
                    thread.owned.truncate(orig_owned);
                    thread.stack.truncate(orig_stack);

                    return Ok(array);
                }

                let (cont, cont_instruction, cont_base, cont_owned) = thread.calls.pop().unwrap();

                function = cont;
                code = &assets.code[&function];
                instruction = cont_instruction;
                reg_base = cont_base;

                thread.stack.resize_with(reg_base + code.locals as usize, Register::default);
                thread.owned.truncate(cont_owned);
                thread.owned.push(array);

                continue;
            }

            (code::Op::CallApi, callee, base, len) => {
                let symbol = get_string(code.constants[callee].borrow());
                let api = assets.api[&symbol];
                let reg_base = reg_base + base;

                let array = unsafe {
                    let cx = &mut *(cx as *mut _ as *mut _);
                    let arguments = reg_base..reg_base + len;
                    match api(cx, thread, arguments) {
                        Ok(value) => value,
                        Err(error) => break error,
                    }
                };
                let value = unsafe { erase_ref(array.borrow()) };
                thread.owned.push(array);

                // The call above may have mutated anything reachable through `cx`.
                // Reload any invalidated borrows.
                let (w, a) = cx.fields();
                world = w;
                assets = a;
                code = &assets.code[&function];

                let registers = &mut thread.stack[reg_base..];
                registers[0].value = value;
            }

            (code::Op::CallGet, get, base, _) => {
                let symbol = get_string(code.constants[get].borrow());
                let get = match assets.get.get(&symbol) {
                    Some(&get) => get,
                    None => break Error::name(symbol),
                };
                let reg_base = reg_base + base;

                let registers = &mut thread.stack[reg_base..];
                let array = {
                    let cx = unsafe { &mut *(cx as *mut _ as *mut _) };
                    let entity = unsafe { registers[0].entity };
                    let i = unsafe { registers[1].value };
                    let i = i32::try_from(i).unwrap_or(0) as usize;
                    get(cx, entity, i)
                };
                let value = unsafe { erase_ref(array.borrow()) };
                thread.owned.push(array);

                // The call above may have mutated anything reachable through `cx`.
                // Reload any invalidated borrows.
                let (w, a) = cx.fields();
                world = w;
                assets = a;
                code = &assets.code[&function];

                registers[0].value = value;
            }

            (code::Op::CallSet, set, base, _) => {
                let symbol = get_string(code.constants[set].borrow());
                let set = match assets.set.get(&symbol) {
                    Some(&set) => set,
                    None => break Error::write(symbol),
                };
                let reg_base = reg_base + base;

                let registers = &thread.stack[reg_base..];
                {
                    let cx = unsafe { &mut *(cx as *mut _ as *mut _) };
                    let value = unsafe { registers[0].value };
                    let entity = unsafe { registers[1].entity };
                    let i = unsafe { registers[2].value };
                    let i = i32::try_from(i).unwrap_or(0) as usize;
                    set(cx, entity, i, value);
                }

                // The call above may have mutated anything reachable through `cx`.
                // Reload any invalidated borrows.
                let (w, a) = cx.fields();
                world = w;
                assets = a;
                code = &assets.code[&function];
            }

            (code::Op::Jump, t_low, t_high, _) => {
                instruction = t_low | (t_high << 8);
                continue;
            }

            (op @ code::Op::BranchFalse, a, t_low, t_high) => {
                let a = unsafe { registers[a].value };
                match a.decode() {
                    Data::Real(a) => if !to_bool(a) {
                        instruction = t_low | (t_high << 8);
                        continue;
                    }
                    _ => break Error::type_unary(op, a.clone()),
                }
            }
        }

        instruction += 1;
    };

    error.frames.push(ErrorFrame { function, instruction });
    let cont = thread.calls.iter()
        .skip(orig_calls)
        .rev()
        .map(|&(function, instruction, _, _)| ErrorFrame { function, instruction });
    error.frames.extend(cont);

    thread.calls.truncate(orig_calls);
    thread.withs.truncate(orig_withs);
    thread.owned.truncate(orig_owned);
    thread.stack.truncate(orig_stack);

    Err(error)
}
