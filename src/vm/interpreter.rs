use std::{mem, cmp, iter, fmt};
use std::collections::{HashMap, HashSet};

use symbol::Symbol;
use vm::{self, code};

/// A single thread of GML execution.
pub struct State {
    functions: HashMap<Symbol, code::Function>,

    globals: Scope,
    global_declarations: HashSet<Symbol>,

    scopes: HashMap<i32, Scope>,

    returns: Vec<(Symbol, usize, usize)>,
    stack: Vec<Register>,
}

type Scope = HashMap<Symbol, vm::Value>;

#[derive(Copy, Clone)]
union Register {
    value: vm::Value,
    row: vm::Row,
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
const LOCAL: i32 = -6;

impl State {
    pub fn new() -> State {
        State {
            functions: HashMap::new(),

            globals: HashMap::new(),
            global_declarations: HashSet::new(),

            scopes: HashMap::new(),

            returns: vec![],
            stack: vec![],
        }
    }

    pub fn add_function(&mut self, symbol: Symbol, function: code::Function) {
        self.functions.insert(symbol, function);
    }

    pub fn execute(
        &mut self, mut symbol: Symbol, arguments: &[vm::Value], self_id: i32, other_id: i32,
    ) -> Result<vm::Value, Error> {
        let mut function = &self.functions[&symbol];
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
                    self.globals.entry(name).or_insert(vm::Value::from(0.0));
                    self.global_declarations.insert(name);
                }

                (code::Op::Lookup, t, name, _) => {
                    let registers = &mut self.stack[reg_base..];

                    let name = Self::get_string(function.constants[name]);
                    registers[t].value = if self.global_declarations.contains(&name) {
                        vm::Value::from(GLOBAL)
                    } else {
                        vm::Value::from(SELF)
                    };
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

                (op @ code::Op::Read, local, a, _) => {
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

                (code::Op::LoadField, t, scope, field) => {
                    let registers = &mut self.stack[reg_base..];

                    let scope = unsafe { registers[scope].value };
                    let field = Self::get_string(function.constants[field]);
                    registers[t].value = *Self::lookup(
                        &mut self.scopes, &mut self.globals, self_id, other_id, scope
                    )
                        .and_then(|scope| scope.get(&field))
                        .ok_or_else(|| {
                            let kind = ErrorKind::Name(field);
                            Error { symbol, instruction, kind }
                        })?;
                }

                (code::Op::LoadFieldDefault, t, scope, field) => {
                    let registers = &mut self.stack[reg_base..];

                    let scope = unsafe { registers[scope].value };
                    let field = Self::get_string(function.constants[field]);
                    let scope = Self::lookup(
                        &mut self.scopes, &mut self.globals, self_id, other_id, scope
                    )
                        .ok_or_else(|| {
                            let kind = ErrorKind::Name(field);
                            Error { symbol, instruction, kind }
                        })?;

                    registers[t].value = match scope.get(&field) {
                        Some(&value) => value,
                        None => vm::Value::from(0.0)
                    };
                }

                (code::Op::LoadFieldArray, t, scope, field) => {
                    let registers = &mut self.stack[reg_base..];

                    let scope = unsafe { registers[scope].value };
                    let field = Self::get_string(function.constants[field]);
                    let scope = Self::lookup(
                        &mut self.scopes, &mut self.globals, self_id, other_id, scope
                    )
                        .ok_or_else(|| {
                            let kind = ErrorKind::Name(field);
                            Error { symbol, instruction, kind }
                        })?;

                    registers[t].value = match scope.get(&field) {
                        Some(&value) => match value.data() {
                            vm::Data::Array(_) => value,
                            _ => vm::Value::from(vm::Array::from_scalar(value)),
                        }
                        None => {
                            let value = vm::Value::from(0.0);
                            vm::Value::from(vm::Array::from_scalar(value))
                        }
                    };
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

                (code::Op::StoreField, s, scope, field) => {
                    let registers = &self.stack[reg_base..];

                    let s = unsafe { registers[s].value };
                    let scope = unsafe { registers[scope].value };
                    let field = Self::get_string(function.constants[field]);
                    let scope = Self::lookup(
                        &mut self.scopes, &mut self.globals, self_id, other_id, scope
                    )
                        .ok_or_else(|| {
                            let kind = ErrorKind::Name(field);
                            Error { symbol, instruction, kind }
                        })?;
                    scope.insert(field, s);
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

                (code::Op::With, _t, _scope, _) => {
                    unimplemented!()
                }

                (code::Op::Next, _t, _iter, _) => {
                    unimplemented!()
                }

                (code::Op::Call, callee, base, len) => {
                    self.returns.push((symbol, instruction + 1, reg_base));

                    symbol = Self::get_string(function.constants[callee]);
                    function = &self.functions[&symbol];
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
                    function = &self.functions[&symbol];
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

    fn lookup<'a>(
        scopes: &'a mut HashMap<i32, Scope>, globals: &'a mut Scope, self_id: i32, other_id: i32,
        scope: vm::Value
    ) -> Option<&'a mut Scope> {
        let scope = match scope.data() {
            vm::Data::Real(scope) => Some(Self::to_i32(scope)),
            _ => None,
        }?;
        let scope = match scope {
            SELF => scopes.get_mut(&self_id),
            OTHER => scopes.get_mut(&other_id),
            ALL => None,
            NOONE => None,
            GLOBAL => Some(globals),
            LOCAL => unimplemented!(),
            scope => scopes.get_mut(&scope),
        }?;

        Some(scope)
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
