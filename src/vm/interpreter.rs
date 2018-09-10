use std::{mem, ptr, slice, cmp, iter, fmt};

use symbol::Symbol;
use vm::{self, code};

/// A single thread of GML execution.
pub struct State {
    world: vm::World,

    self_entity: vm::Entity,
    other_entity: vm::Entity,

    returns: Vec<(Symbol, usize, usize)>,
    stack: Vec<Register>,
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

#[derive(Copy, Clone, PartialEq, Eq)]
pub struct Error {
    symbol: Symbol,
    instruction: usize,
    kind: ErrorKind,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum ErrorKind {
    TypeUnary(code::Op, vm::Type),
    TypeBinary(code::Op, vm::Type, vm::Type),
    Scope(i32),
    Name(Symbol),
    Bounds(usize),
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}+{}:{:?}", self.symbol, self.instruction, self.kind)
    }
}

const SELF: i32 = -1;
const OTHER: i32 = -2;
const ALL: i32 = -3;
const NOONE: i32 = -4;
const GLOBAL: i32 = -5;
// -6?
const LOCAL: i32 = -7;

impl State {
    pub fn new() -> State {
        State {
            world: vm::World::new(),

            self_entity: vm::Entity::default(),
            other_entity: vm::Entity::default(),

            returns: vec![],
            stack: vec![],
        }
    }

    pub fn create_instance(&mut self, id: i32) {
        let entity = self.world.create_entity();
        self.world.create_instance(id, entity);
    }

    pub fn get_instance(&self, id: i32) -> &vm::Instance {
        let entity = self.world.instances[id];
        &self.world.instance_table[&entity]
    }

    pub fn set_self(&mut self, id: i32) {
        self.self_entity = self.world.instances[id];
    }

    pub fn set_other(&mut self, id: i32) {
        self.other_entity = self.world.instances[id];
    }

    pub fn arguments(&self, arguments: vm::Arguments) -> &[vm::Value] {
        let vm::Arguments { base, limit } = arguments;
        let registers = &self.stack[base..limit];
        unsafe { mem::transmute::<&[Register], &[vm::Value]>(registers) }
    }

    pub fn execute(
        &mut self, resources: &vm::Resources, symbol: Symbol, arguments: &[vm::Value]
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

        loop {
            match function.instructions[instruction].decode() {
                (code::Op::Imm, t, constant, _) => {
                    let registers = &mut self.stack[reg_base..];

                    registers[t].value = function.constants[constant];
                }

                (code::Op::Move, t, s, _) => {
                    let registers = &mut self.stack[reg_base..];

                    registers[t] = registers[s];
                }

                (op @ code::Op::Neg, t, a, _) => {
                    let registers = &mut self.stack[reg_base..];

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
                    let registers = &mut self.stack[reg_base..];

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
                    let registers = &mut self.stack[reg_base..];

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
                    let registers = &mut self.stack[reg_base..];

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
                    let registers = &mut self.stack[reg_base..];

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
                    let registers = &mut self.stack[reg_base..];

                    let a = unsafe { registers[a].value };
                    let b = unsafe { registers[b].value };
                    registers[t].value = vm::Value::from(a == b);
                }

                (code::Op::Ne, t, a, b) => {
                    let registers = &mut self.stack[reg_base..];

                    let a = unsafe { registers[a].value };
                    let b = unsafe { registers[b].value };
                    registers[t].value = vm::Value::from(a != b);
                }

                (op @ code::Op::Ge, t, a, b) => {
                    let registers = &mut self.stack[reg_base..];

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
                    let registers = &mut self.stack[reg_base..];

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
                    let registers = &mut self.stack[reg_base..];

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
                    let registers = &mut self.stack[reg_base..];

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
                    let registers = &mut self.stack[reg_base..];

                    let a = unsafe { registers[a].value };
                    let b = unsafe { registers[b].value };
                    registers[t].value = match (a.data(), b.data()) {
                        (vm::Data::Real(a), vm::Data::Real(b)) => Ok(vm::Value::from(a * b)),
                        (vm::Data::Real(a), vm::Data::String(b)) => {
                            let b: &str = &b;
                            let t: String = iter::repeat(b).take(a as usize).collect();
                            Ok(vm::Value::from(Symbol::intern(&t)))
                        }
                        (a, b) => {
                            let kind = ErrorKind::TypeBinary(op, a.ty(), b.ty());
                            Err(Error { symbol, instruction, kind })
                        }
                    }?;
                }

                (op @ code::Op::Div, t, a, b) => {
                    let registers = &mut self.stack[reg_base..];

                    let a = unsafe { registers[a].value };
                    let b = unsafe { registers[b].value };
                    registers[t].value = match (a.data(), b.data()) {
                        (vm::Data::Real(a), vm::Data::Real(b)) => Ok(vm::Value::from(a / b)),
                        (a, b) => {
                            let kind = ErrorKind::TypeBinary(op, a.ty(), b.ty());
                            Err(Error { symbol, instruction, kind })
                        }
                    }?;
                }

                (op @ code::Op::IntDiv, t, a, b) => {
                    let registers = &mut self.stack[reg_base..];

                    let a = unsafe { registers[a].value };
                    let b = unsafe { registers[b].value };
                    registers[t].value = match (a.data(), b.data()) {
                        (vm::Data::Real(a), vm::Data::Real(b)) => {
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
                    let registers = &mut self.stack[reg_base..];

                    let a = unsafe { registers[a].value };
                    let b = unsafe { registers[b].value };
                    registers[t].value = match (a.data(), b.data()) {
                        (vm::Data::Real(a), vm::Data::Real(b)) => Ok(vm::Value::from(a % b)),
                        (a, b) => {
                            let kind = ErrorKind::TypeBinary(op, a.ty(), b.ty());
                            Err(Error { symbol, instruction, kind })
                        }
                    }?;
                }

                (op @ code::Op::And, t, a, b) => {
                    let registers = &mut self.stack[reg_base..];

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
                    let registers = &mut self.stack[reg_base..];

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
                    let registers = &mut self.stack[reg_base..];

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
                    let registers = &mut self.stack[reg_base..];

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
                    let registers = &mut self.stack[reg_base..];

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
                    let registers = &mut self.stack[reg_base..];

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
                    let registers = &mut self.stack[reg_base..];

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
                    let registers = &mut self.stack[reg_base..];

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
                    self.world.globals.insert(name);

                    let entity = vm::world::GLOBAL;
                    let component = self.world.hash_table.get_mut(&entity).unwrap();
                    component.entry(name).or_insert(vm::Value::from(0.0));
                }

                (code::Op::Lookup, t, name, _) => {
                    let registers = &mut self.stack[reg_base..];

                    let name = Self::get_string(function.constants[name]);
                    registers[t].entity = if self.world.globals.contains(&name) {
                        vm::world::GLOBAL
                    } else {
                        self.self_entity
                    };
                }

                // TODO: Replace these with `self`/`other` arguments and locals passed to `Lookup`.

                (code::Op::LoadScope, t, scope, _) => {
                    let registers = &mut self.stack[reg_base..];

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
                    let registers = &self.stack[reg_base..];

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
                    let registers = &mut self.stack[reg_base..];

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
                        ALL => self.world.instances.values(),
                        NOONE => &[],
                        GLOBAL => slice::from_ref(&vm::world::GLOBAL),
                        LOCAL => &[], // TODO: error
                        object if (0..=100_000).contains(&object) =>
                            &self.world.objects[&object][..],
                        instance if (100_001..).contains(&instance) =>
                            slice::from_ref(&self.world.instances[instance]),
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
                    let registers = &mut self.stack[reg_base..];

                    let ptr = unsafe { registers[ptr].iterator };
                    registers[t].entity = unsafe { *ptr.as_ptr() };
                }

                (code::Op::NextPointer, t, ptr, _) => {
                    let registers = &mut self.stack[reg_base..];

                    let ptr = unsafe { registers[ptr].iterator };
                    registers[t].iterator = unsafe {
                        ptr::NonNull::new_unchecked(ptr.as_ptr().offset(1))
                    };
                }

                (code::Op::NePointer, t, a, b) => {
                    let registers = &mut self.stack[reg_base..];

                    let a = unsafe { registers[a].iterator };
                    let b = unsafe { registers[b].iterator };
                    registers[t].value = vm::Value::from(a != b);
                }

                (code::Op::ExistsEntity, t, entity, _) => {
                    let registers = &mut self.stack[reg_base..];

                    let entity = unsafe { registers[entity].entity };
                    let exists = self.world.hash_table.contains_key(&entity);
                    registers[t].value = vm::Value::from(exists);
                }

                (op @ code::Op::Read, a, local, _) => {
                    let registers = &mut self.stack[reg_base..];

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
                    let registers = &mut self.stack[reg_base..];

                    let a = unsafe { registers[a].value };
                    let b = unsafe { registers[b].value };
                    registers[t].value = match b.data() {
                        vm::Data::Array(b) => {
                            b.store(0, 0, a);
                            vm::Value::from(b)
                        }
                        _ => a
                    };
                }

                (op @ code::Op::ScopeError, scope, _, _) => {
                    let registers = &self.stack[reg_base..];

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
                    let registers = &mut self.stack[reg_base..];

                    let a = unsafe { registers[a].value };
                    registers[t].value = match a.data() {
                        vm::Data::Array(_) => Ok(a),
                        _ => Ok(vm::Value::from(vm::Array::from_scalar(a))),
                    }?;
                }

                (code::Op::ToScalar, t, a, _) => {
                    let registers = &mut self.stack[reg_base..];

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
                    let registers = &mut self.stack[reg_base..];

                    let a = unsafe { registers[a].value };
                    unsafe { a.release() };
                }

                (code::Op::LoadField, t, entity, field) => {
                    let registers = &mut self.stack[reg_base..];

                    let entity = unsafe { registers[entity].entity };
                    let field = Self::get_string(function.constants[field]);
                    let component = &self.world.hash_table[&entity];
                    registers[t].value = *component.get(&field)
                        .ok_or_else(|| {
                            let kind = ErrorKind::Name(field);
                            Error { symbol, instruction, kind }
                        })?;
                }

                (code::Op::LoadFieldDefault, t, entity, field) => {
                    let registers = &mut self.stack[reg_base..];

                    let entity = unsafe { registers[entity].entity };
                    let field = Self::get_string(function.constants[field]);
                    let component = &self.world.hash_table[&entity];
                    registers[t].value = *component.get(&field)
                        .unwrap_or(&vm::Value::from(0.0));
                }

                (op @ code::Op::LoadRow, t, a, i) => {
                    let registers = &mut self.stack[reg_base..];

                    let a = unsafe { registers[a].value };
                    let i = unsafe { registers[i].value };
                    registers[t].row = match (a.data(), i.data()) {
                        (vm::Data::Array(array), vm::Data::Real(i)) => {
                            let i = Self::to_i32(i) as usize;
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
                    let registers = &mut self.stack[reg_base..];

                    let r = unsafe { registers[r].row };
                    let j = unsafe { registers[j].value };
                    registers[t].value = match j.data() {
                        vm::Data::Real(j) => {
                            let j = Self::to_i32(j) as usize;
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
                    let registers = &self.stack[reg_base..];

                    let s = unsafe { registers[s].value };
                    let entity = unsafe { registers[entity].entity };
                    let field = Self::get_string(function.constants[field]);
                    let component = self.world.hash_table.get_mut(&entity).unwrap();
                    component.insert(field, s);
                }

                (op @ code::Op::StoreRow, t, a, i) => {
                    let registers = &mut self.stack[reg_base..];

                    let a = unsafe { registers[a].value };
                    let i = unsafe { registers[i].value };
                    registers[t].row = match (a.data(), i.data()) {
                        (vm::Data::Array(array), vm::Data::Real(i)) => {
                            let i = Self::to_i32(i) as usize;
                            Ok(array.store_row(i))
                        }
                        (a, i) => {
                            let kind = ErrorKind::TypeBinary(op, a.ty(), i.ty());
                            Err(Error { symbol, instruction, kind })
                        }
                    }?;
                }

                (op @ code::Op::StoreIndex, s, r, j) => {
                    let registers = &self.stack[reg_base..];

                    let s = unsafe { registers[s].value };
                    let r = unsafe { registers[r].row };
                    let j = unsafe { registers[j].value };
                    match j.data() {
                        vm::Data::Real(j) => {
                            let j = Self::to_i32(j) as usize;
                            Ok(unsafe { r.store(j, s) })
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

                (code::Op::CallNative, callee, base, len) => {
                    let symbol = Self::get_string(function.constants[callee]);
                    let function = resources.functions[&symbol];
                    let reg_base = reg_base + base;

                    let limit = reg_base + len;

                    let arguments = vm::Arguments { base, limit };
                    let value = function(self, resources, arguments)?;

                    let registers = &mut self.stack[reg_base..];
                    registers[0].value = value;
                }

                (code::Op::CallGet, get, base, _) => {
                    let symbol = Self::get_string(function.constants[get]);
                    let function = resources.get[&symbol];
                    let reg_base = reg_base + base;

                    // TODO: simplify this with NLL
                    let entity;
                    {
                        let registers = &self.stack[reg_base..];
                        entity = unsafe { registers[0].entity };
                    }
                    let instance = &self.world.instance_table[&entity];
                    let value = function(instance);

                    let registers = &mut self.stack[reg_base..];
                    registers[0].value = value;
                }

                (code::Op::CallSet, set, base, _) => {
                    let symbol = Self::get_string(function.constants[set]);
                    let function = resources.set[&symbol];
                    let reg_base = reg_base + base;

                    // TODO: simplify this with NLL
                    let (entity, value);
                    {
                        let registers = &self.stack[reg_base..];
                        entity = unsafe { registers[0].entity };
                        value = unsafe { registers[1].value };
                    }
                    let instance = self.world.instance_table.get_mut(&entity).unwrap();
                    function(instance, value);
                }

                (code::Op::CallGetIndex, get, base, _) => {
                    let symbol = Self::get_string(function.constants[get]);
                    let function = resources.get_index[&symbol];
                    let reg_base = reg_base + base;

                    // TODO: simplify this with NLL
                    let (entity, i);
                    {
                        let registers = &self.stack[reg_base..];
                        entity = unsafe { registers[0].entity };
                        i = unsafe { registers[1].value };
                    }
                    let instance = &self.world.instance_table[&entity];
                    let i = match i.data() {
                        vm::Data::Real(i) => Self::to_i32(i) as usize,
                        _ => 0,
                    };
                    let value = function(instance, i);

                    let registers = &mut self.stack[reg_base..];
                    registers[0].value = value;
                }

                (code::Op::CallSetIndex, set, base, _) => {
                    let symbol = Self::get_string(function.constants[set]);
                    let function = resources.set_index[&symbol];
                    let reg_base = reg_base + base;

                    // TODO: simplify this with NLL
                    let (entity, i, value);
                    {
                        let registers = &self.stack[reg_base..];
                        entity = unsafe { registers[0].entity };
                        i = unsafe { registers[1].value };
                        value = unsafe { registers[2].value };
                    }
                    let instance = self.world.instance_table.get_mut(&entity).unwrap();
                    let i = match i.data() {
                        vm::Data::Real(i) => Self::to_i32(i) as usize,
                        _ => 0,
                    };
                    function(instance, i, value);
                }

                (code::Op::Ret, _, _, _) => {
                    let (caller, caller_instruction, caller_base) = match self.returns.pop() {
                        Some(frame) => frame,
                        None => {
                            let registers = &self.stack[reg_base..];
                            let value = unsafe { registers[0].value };
                            return Ok(value);
                        }
                    };

                    symbol = caller;
                    function = &resources.scripts[&symbol];
                    instruction = caller_instruction;
                    reg_base = caller_base;

                    self.stack.truncate(reg_base + function.locals as usize);

                    continue;
                }

                (code::Op::Jump, t, _, _) => {
                    instruction = t;
                    continue;
                }

                (op @ code::Op::BranchFalse, a, t, _) => {
                    let registers = &mut self.stack[reg_base..];

                    let a = unsafe { registers[a].value };
                    match a.data() {
                        vm::Data::Real(a) => {
                            let a = Self::to_bool(a);
                            if !a {
                                instruction = t;
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
