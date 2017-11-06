use std::{mem, cmp, iter};
use std::collections::HashMap;

use symbol::{Symbol, keyword};
use front::{self, ast, Span, ErrorHandler};
use back::ssa;

pub struct Codegen<'e> {
    builder: front::ssa::Builder,
    errors: &'e ErrorHandler,

    /// GML `var` declarations are static and independent of control flow. All references to a
    /// `var`-declared name after its declaration in the source text are treated as local.
    locals: HashMap<Symbol, Local>,
    /// The number of script arguments that have been created so far.
    arguments: u32,
    /// The return value of the program.
    return_value: front::ssa::Local,
    /// The number of entry-block instructions initializing local variables. This is used as an
    /// insertion point so more can be inserted.
    initializers: u32,

    current_block: ssa::Block,

    current_next: Option<ssa::Block>,
    current_exit: Option<ssa::Block>,

    current_switch: Option<ssa::Value>,
    current_expr: Option<ssa::Block>,
    current_default: Option<ssa::Block>,
}

/// A location that can be read from or written to.
///
/// GML arrays are not first class values, and are instead tied to variable bindings. To accomodate
/// this, `Lvalue::Index` is replaced with `Lvalue::IndexLocal` and `Lvalue::IndexField`. In the
/// future, `Index` can be used to implement first-class arrays and data structure accessors.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
enum Lvalue {
    Local(Symbol),
    Field(ssa::Value, Symbol),
    Index(ssa::Value, [ssa::Value; 2]),
    IndexLocal(Symbol, [ssa::Value; 2]),
    IndexField(ssa::Value, Symbol, [ssa::Value; 2]),
}

#[derive(Debug)]
struct LvalueError;

/// A GML-level local variable.
#[derive(Copy, Clone)]
struct Local {
    /// Each local variable must dynamically track whether it has been initialized- a compile-time
    /// error for uninitialized uses would reject some valid GML programs.
    flag: front::ssa::Local,
    local: front::ssa::Local,
}

// TODO: deduplicate these with the ones from vm::interpreter?
const SELF: f64 = -1.0;
const OTHER: f64 = -2.0;
const ALL: f64 = -3.0;
const NOONE: f64 = -4.0;
const GLOBAL: f64 = -5.0;
const LOCAL: f64 = -6.0;

impl<'e> Codegen<'e> {
    pub fn new(errors: &'e ErrorHandler) -> Codegen<'e> {
        let mut builder = front::ssa::Builder::new();
        let return_value = builder.emit_local();

        Codegen {
            builder: builder,
            errors: errors,

            locals: HashMap::new(),
            arguments: 0,
            return_value: return_value,
            initializers: 0,

            current_block: ssa::ENTRY,

            current_next: None,
            current_exit: None,

            current_switch: None,
            current_expr: None,
            current_default: None,
        }
    }

    pub fn compile(mut self, program: &(ast::Stmt, Span)) -> ssa::Function {
        let entry_block = self.current_block;
        self.builder.seal_block(entry_block);

        let zero = self.emit_real(0.0);
        self.builder.write_local(self.current_block, self.return_value, zero);

        self.emit_statement(program);

        self.emit_jump(ssa::EXIT);
        self.builder.seal_block(ssa::EXIT);

        self.current_block = ssa::EXIT;
        let locals = mem::replace(&mut self.locals, HashMap::default());
        for (_, Local { local, .. }) in locals {
            let value = self.builder.read_local(self.current_block, local);
            self.emit_instruction(ssa::Inst::Release { arg: value });
        }
        let return_value = self.builder.read_local(self.current_block, self.return_value);
        self.emit_instruction(ssa::Inst::Return { arg: return_value });

        self.builder.finish()
    }

    fn emit_statement(&mut self, statement: &(ast::Stmt, Span)) {
        let (ref statement, statement_span) = *statement;
        match *statement {
            ast::Stmt::Assign(op, box ref lvalue, box ref rvalue) => {
                let lvalue = match self.emit_lvalue(lvalue) {
                    Ok(lvalue) => lvalue,
                    Err(LvalueError) => return,
                };

                let rvalue = if let Some(op) = op {
                    let lvalue = lvalue.clone();
                    let left = self.emit_load(lvalue);
                    let right = self.emit_rvalue(rvalue);

                    let op = ast::Binary::Op(op).into();
                    let inst = ssa::Inst::Binary { op, args: [left, right] };
                    self.emit_instruction(inst)
                } else {
                    self.emit_rvalue(rvalue)
                };

                self.emit_store(lvalue, rvalue);
            }

            ast::Stmt::Invoke(ref call) => {
                self.emit_call(call);
            }

            ast::Stmt::Declare(scope, box ref names) => {
                let names: Vec<_> = names.iter().filter_map(|&(name, name_span)| {
                    if name.is_argument() {
                        self.errors.error(name_span, "cannot redeclare a builtin variable");
                        return None;
                    }

                    Some(name)
                }).collect();

                match scope {
                    ast::Declare::Local => {
                        for symbol in names {
                            let local = self.emit_local(None);
                            self.locals.insert(symbol, local);
                        }
                    }

                    ast::Declare::Global => {
                        for symbol in names {
                            self.emit_instruction(ssa::Inst::DeclareGlobal { symbol });
                        }
                    }
                };
            }

            ast::Stmt::Block(box ref statements) => {
                for statement in statements {
                    self.emit_statement(statement);
                }
            }

            ast::Stmt::If(box ref expr, box ref true_branch, ref false_branch) => {
                let true_block = self.make_block();
                let false_block = self.make_block();
                let merge_block = if false_branch.is_some() {
                    self.make_block()
                } else {
                    false_block
                };

                let expr = self.emit_rvalue(expr);
                self.emit_branch(expr, true_block, false_block);
                self.builder.seal_block(true_block);
                self.builder.seal_block(false_block);

                self.current_block = true_block;
                self.emit_statement(true_branch);
                self.emit_jump(merge_block);

                if let Some(box ref false_branch) = *false_branch {
                    self.current_block = false_block;
                    self.emit_statement(false_branch);
                    self.emit_jump(merge_block);
                }

                self.builder.seal_block(merge_block);

                self.current_block = merge_block;
            }

            ast::Stmt::Repeat(box ref expr, box ref body) => {
                let next_block = self.make_block();
                let body_block = self.make_block();
                let exit_block = self.make_block();

                let iter = self.builder.emit_local();

                let count = self.emit_rvalue(expr);
                self.builder.write_local(self.current_block, iter, count);
                self.emit_jump(next_block);

                self.current_block = next_block;
                let count = self.builder.read_local(self.current_block, iter);
                let one = self.emit_real(1.0);
                let op = ssa::Binary::Subtract;
                let next = self.emit_instruction(ssa::Inst::Binary { op, args: [count, one] });
                self.builder.write_local(self.current_block, iter, next);
                self.emit_branch(count, body_block, exit_block);
                self.builder.seal_block(body_block);

                self.current_block = body_block;
                self.with_loop(next_block, exit_block, |self_| {
                    self_.emit_statement(body);
                });
                self.emit_jump(next_block);
                self.builder.seal_block(next_block);
                self.builder.seal_block(exit_block);

                self.current_block = exit_block;
            }

            ast::Stmt::While(box ref expr, box ref body) => {
                let next_block = self.make_block();
                let body_block = self.make_block();
                let exit_block = self.make_block();

                self.emit_jump(next_block);

                self.current_block = next_block;
                let expr = self.emit_rvalue(expr);
                self.emit_branch(expr, body_block, exit_block);
                self.builder.seal_block(body_block);

                self.current_block = body_block;
                self.with_loop(next_block, exit_block, |self_| {
                    self_.emit_statement(body);
                });
                self.emit_jump(next_block);
                self.builder.seal_block(next_block);
                self.builder.seal_block(exit_block);

                self.current_block = exit_block;
            }

            ast::Stmt::Do(box ref body, box ref expr) => {
                let body_block = self.make_block();
                let next_block = self.make_block();
                let exit_block = self.make_block();

                self.emit_jump(body_block);

                self.current_block = body_block;
                self.with_loop(next_block, exit_block, |self_| {
                    self_.emit_statement(body);
                });
                self.emit_jump(next_block);
                self.builder.seal_block(next_block);

                self.current_block = next_block;
                let expr = self.emit_rvalue(expr);
                self.emit_branch(expr, exit_block, body_block);
                self.builder.seal_block(body_block);
                self.builder.seal_block(exit_block);

                self.current_block = exit_block;
            }

            ast::Stmt::For(box ref init, box ref expr, box ref next, box ref body) => {
                let expr_block = self.make_block();
                let body_block = self.make_block();
                let next_block = self.make_block();
                let exit_block = self.make_block();

                self.emit_statement(init);
                self.emit_jump(expr_block);

                self.current_block = expr_block;
                let expr = self.emit_rvalue(expr);
                self.emit_branch(expr, body_block, exit_block);
                self.builder.seal_block(body_block);

                self.current_block = body_block;
                self.with_loop(next_block, exit_block, |self_| {
                    self_.emit_statement(body);
                });
                self.emit_jump(next_block);
                self.builder.seal_block(next_block);
                self.builder.seal_block(exit_block);

                self.current_block = next_block;
                self.emit_statement(next);
                self.emit_jump(expr_block);
                self.builder.seal_block(expr_block);

                self.current_block = exit_block;
            }

            ast::Stmt::With(box ref expr, box ref body) => {
                let next_block = self.make_block();
                let body_block = self.make_block();
                let exit_block = self.make_block();

                let iter = self.builder.emit_local();

                let expr = self.emit_rvalue(expr);
                let op = ssa::Unary::With;
                let with = self.emit_instruction(ssa::Inst::Unary { op, arg: expr });
                self.builder.write_local(self.current_block, iter, with);
                self.emit_jump(next_block);

                self.current_block = next_block;
                let with = self.builder.read_local(self.current_block, iter);
                let op = ssa::Unary::Next;
                let next = self.emit_instruction(ssa::Inst::Unary { op, arg: with });
                self.builder.write_local(self.current_block, iter, next);
                self.emit_branch(with, body_block, exit_block);
                self.builder.seal_block(body_block);

                self.current_block = body_block;
                self.with_loop(next_block, exit_block, |self_| {
                    self_.emit_statement(body);
                });
                self.emit_jump(next_block);
                self.builder.seal_block(next_block);
                self.builder.seal_block(exit_block);

                self.current_block = exit_block;
            }

            ast::Stmt::Switch(box ref expr, box ref body) => {
                let expr_block = self.current_block;
                let dead_block = self.make_block();
                let default_block = self.make_block();
                let exit_block = self.make_block();

                self.builder.seal_block(dead_block);

                let expr = self.emit_rvalue(expr);

                self.current_block = dead_block;
                self.with_switch(expr, expr_block, default_block, exit_block, |self_| {
                    for statement in body {
                        self_.emit_statement(statement);
                    }
                    self_.emit_jump(exit_block);

                    self_.current_block = self_.current_expr.expect("corrupt switch state");
                    self_.emit_jump(default_block);
                    self_.builder.seal_block(default_block);
                });
                self.builder.seal_block(exit_block);

                self.current_block = exit_block;
            }

            ast::Stmt::Case(Some(box ref expr)) if self.current_expr.is_some() => {
                let case_block = self.make_block();
                let expr_block = self.make_block();

                self.emit_jump(case_block);

                self.current_block = self.current_expr.unwrap();
                self.current_expr = Some(expr_block);
                let switch = self.current_switch.expect("corrupt switch state");
                let expr = self.emit_rvalue(expr);
                let op = ssa::Binary::Eq;
                let inst = ssa::Inst::Binary { op, args: [switch, expr] };
                let expr = self.emit_instruction(inst);
                self.emit_branch(expr, case_block, expr_block);
                self.builder.seal_block(case_block);
                self.builder.seal_block(expr_block);

                self.current_block = case_block;
            }

            ast::Stmt::Case(None) if self.current_default.is_some() => {
                let default_block = self.current_default.unwrap();

                self.emit_jump(default_block);

                self.current_block = default_block;
            }

            ast::Stmt::Case(_) => {
                self.errors.error(statement_span, "case statement outside of switch");
            }

            ast::Stmt::Jump(ast::Jump::Break) if self.current_exit.is_some() => {
                let exit_block = self.current_exit.unwrap();

                self.emit_jump(exit_block);
                self.current_block = self.make_block();
                self.builder.seal_block(self.current_block);
            }

            ast::Stmt::Jump(ast::Jump::Continue) if self.current_next.is_some() => {
                let next_block = self.current_next.unwrap();

                self.emit_jump(next_block);
                self.current_block = self.make_block();
                self.builder.seal_block(self.current_block);
            }

            // exit and break/continue outside loops return 0
            ast::Stmt::Jump(_) => {
                self.emit_jump(ssa::EXIT);

                self.current_block = self.make_block();
                self.builder.seal_block(self.current_block);
            }

            ast::Stmt::Return(box ref expr) => {
                let expr = self.emit_rvalue(expr);
                self.builder.write_local(self.current_block, self.return_value, expr);
                self.emit_jump(ssa::EXIT);

                self.current_block = self.make_block();
                self.builder.seal_block(self.current_block);
            }

            ast::Stmt::Error(_) => {}
        }
    }

    fn emit_rvalue(&mut self, expression: &(ast::Expr, Span)) -> ssa::Value {
        let (ref expr, _expr_span) = *expression;
        match *expr {
            ast::Expr::Value(ast::Value::Real(real)) => self.emit_real(real),
            ast::Expr::Value(ast::Value::String(string)) => self.emit_string(string),

            ast::Expr::Value(ast::Value::Ident(keyword::True)) => self.emit_real(1.0),
            ast::Expr::Value(ast::Value::Ident(keyword::False)) => self.emit_real(0.0),
            ast::Expr::Value(ast::Value::Ident(keyword::Self_)) => self.emit_real(SELF),
            ast::Expr::Value(ast::Value::Ident(keyword::Other)) => self.emit_real(OTHER),
            ast::Expr::Value(ast::Value::Ident(keyword::All)) => self.emit_real(ALL),
            ast::Expr::Value(ast::Value::Ident(keyword::NoOne)) => self.emit_real(NOONE),
            ast::Expr::Value(ast::Value::Ident(keyword::Global)) => self.emit_real(GLOBAL),
            ast::Expr::Value(ast::Value::Ident(keyword::Local)) => self.emit_real(LOCAL),

            ast::Expr::Unary(ast::Unary::Positive, box ref expr) => self.emit_rvalue(expr),
            ast::Expr::Unary(op, box ref expr) => {
                let op = match op {
                    ast::Unary::Negate => ssa::Unary::Negate,
                    ast::Unary::Invert => ssa::Unary::Invert,
                    ast::Unary::BitInvert => ssa::Unary::BitInvert,
                    _ => unreachable!(),
                };
                let expr = self.emit_rvalue(expr);
                self.emit_instruction(ssa::Inst::Unary { op, arg: expr })
            }

            ast::Expr::Binary(op, box ref left, box ref right) => {
                let left = self.emit_rvalue(left);
                let right = self.emit_rvalue(right);
                let op = op.into();
                self.emit_instruction(ssa::Inst::Binary { op, args: [left, right] })
            }

            ast::Expr::Call(ref call) => {
                self.emit_call(call)
            }

            _ => {
                let lvalue = self.emit_lvalue(expression)
                    .expect("_ is not a valid expression");
                self.emit_load(lvalue)
            }
        }
    }

    fn emit_lvalue(&mut self, expression: &(ast::Expr, Span)) -> Result<Lvalue, LvalueError> {
        let (ref expression, expression_span) = *expression;
        match *expression {
            ast::Expr::Value(ast::Value::Ident(symbol)) if !symbol.is_keyword() => {
                if let Some(argument) = symbol.as_argument() {
                    for argument in self.arguments..argument + 1 {
                        let symbol = Symbol::from_argument(argument);

                        let argument = self.builder.function.emit_argument(ssa::ENTRY);

                        let local = self.emit_local(Some(argument));
                        self.locals.insert(symbol, local);
                    }
                    self.arguments = cmp::max(self.arguments, argument + 1);
                }

                if self.locals.contains_key(&symbol) {
                    Ok(Lvalue::Local(symbol))
                } else {
                    let inst = ssa::Inst::Lookup { symbol };
                    let scope = self.emit_instruction(inst);
                    Ok(Lvalue::Field(scope, symbol))
                }
            }

            ast::Expr::Field(box ref expr, (field, _field_span)) => {
                let scope = self.emit_rvalue(expr);
                Ok(Lvalue::Field(scope, field))
            }

            ast::Expr::Index(box ref expr, box ref indices) => {
                if indices.len() < 1 || 2 < indices.len() {
                    self.errors.error(expression_span, "invalid number of array indices");
                }

                let array = self.emit_lvalue(expr)?;
                let zero = self.emit_real(0.0);
                let mut indices = indices.iter().rev()
                    .map(|index| self.emit_rvalue(index))
                    .chain(iter::repeat(zero));

                let j = indices.next().unwrap();
                let i = indices.next().unwrap();

                match array {
                    Lvalue::Local(symbol) => Ok(Lvalue::IndexLocal(symbol, [i, j])),
                    Lvalue::Field(scope, field) => Ok(Lvalue::IndexField(scope, field, [i, j])),
                    _ => {
                        let (_, expr_span) = *expr;
                        self.errors.error(expr_span, "expected a variable");
                        Err(LvalueError)
                    }
                }
            }

            _ => {
                self.errors.error(expression_span, "expected a variable");
                Err(LvalueError)
            }
        }
    }

    /// Language-level variable load.
    ///
    /// This handles GML's odd behavior around arrays. Before GMS:
    /// - all loads produce scalars; if the variable holds an array it loads `a[0, 0]`
    /// - indexed loads from scalar variables treat the variable as a 1x1 array
    fn emit_load(&mut self, lvalue: Lvalue) -> ssa::Value {
        let value = match lvalue {
            Lvalue::Local(symbol) => {
                let Local { flag, local } = self.locals[&symbol];

                let flag = self.builder.read_local(self.current_block, flag);
                self.emit_instruction(ssa::Inst::Read { symbol, arg: flag });

                self.builder.read_local(self.current_block, local)
            }
            Lvalue::Field(scope, field) =>
                self.emit_instruction(ssa::Inst::LoadField { scope, field }),
            Lvalue::Index(array, [i, j]) =>
                self.emit_instruction(ssa::Inst::LoadIndex { args: [array, i, j] }),

            Lvalue::IndexLocal(symbol, [i, j]) => {
                let Local { flag, local } = self.locals[&symbol];

                let flag = self.builder.read_local(self.current_block, flag);
                self.emit_instruction(ssa::Inst::Read { symbol, arg: flag });

                let array = self.builder.read_local(self.current_block, local);

                // TODO: this only happens pre-gms
                let op = ssa::Unary::ToArray;
                let array = self.emit_instruction(ssa::Inst::Unary { op, arg: array });

                self.emit_instruction(ssa::Inst::LoadIndex { args: [array, i, j] })
            }

            Lvalue::IndexField(scope, field, [i, j]) => {
                let array = self.emit_instruction(ssa::Inst::LoadField { scope, field });

                // TODO: this only happens pre-gms
                let op = ssa::Unary::ToArray;
                let array = self.emit_instruction(ssa::Inst::Unary { op, arg: array });

                self.emit_instruction(ssa::Inst::LoadIndex { args: [array, i, j] })
            }
        };

        // TODO: this only happens pre-gms
        let op = ssa::Unary::ToScalar;
        self.emit_instruction(ssa::Inst::Unary { op, arg: value })
    }

    /// Language-level variable store.
    ///
    /// This handles GML's odd behavior around arrays.
    /// - indexed stores convert variables to arrays if they are undefined or scalar
    ///
    /// Before GMS:
    /// - stores to array variables do *not* overwrite the whole array, only `a[0, 0]`
    /// - indexed stores to scalar (or undefined) variables leave the scalar (or `0`) at `a[0, 0]`
    fn emit_store(&mut self, lvalue: Lvalue, value: ssa::Value) {
        match lvalue {
            Lvalue::Local(symbol) => {
                let Local { flag, local } = self.locals[&symbol];

                let one = self.emit_real(1.0);
                self.builder.write_local(self.current_block, flag, one);

                // TODO: this only happens pre-gms
                let array = self.builder.read_local(self.current_block, local);
                let value = self.emit_instruction(ssa::Inst::Write { args: [value, array] });

                self.builder.write_local(self.current_block, local, value);
            }

            Lvalue::Field(scope, field) => {
                // TODO: this only happens pre-gms
                let inst = ssa::Inst::WriteField { args: [value, scope], field };
                let value = self.emit_instruction(inst);

                self.emit_instruction(ssa::Inst::StoreField { args: [value, scope], field });
            }

            Lvalue::Index(array, [i, j]) => {
                self.emit_instruction(ssa::Inst::StoreIndex { args: [value, array, i, j] });
            }

            Lvalue::IndexLocal(symbol, [i, j]) => {
                let Local { flag, local } = self.locals[&symbol];

                let one = self.emit_real(1.0);
                self.builder.write_local(self.current_block, flag, one);

                let array = self.builder.read_local(self.current_block, local);

                // TODO: this only happens pre-gms; gms does need to handle undef
                let op = ssa::Unary::ToArray;
                let array = self.emit_instruction(ssa::Inst::Unary { op, arg: array });
                self.builder.write_local(self.current_block, local, array);

                self.emit_instruction(ssa::Inst::StoreIndex { args: [value, array, i, j] });
            }

            Lvalue::IndexField(scope, field, [i, j]) => {
                // TODO: this only happens pre-gms; gms does need to handle undef
                let array = self.emit_instruction(ssa::Inst::ToArrayField { scope, field });
                self.emit_instruction(ssa::Inst::StoreField { args: [array, scope], field });

                self.emit_instruction(ssa::Inst::StoreIndex { args: [value, array, i, j] });
            }
        }
    }

    fn emit_jump(&mut self, target: ssa::Block) {
        self.emit_instruction(ssa::Inst::Jump { target, args: vec![] });

        self.builder.insert_edge(self.current_block, target);
    }

    fn emit_branch(&mut self, expr: ssa::Value, true_block: ssa::Block, false_block: ssa::Block) {
        self.emit_instruction(ssa::Inst::Branch {
            targets: [true_block, false_block],
            arg_lens: [0, 0],
            args: vec![expr],
        });

        self.builder.insert_edge(self.current_block, true_block);
        self.builder.insert_edge(self.current_block, false_block);
    }

    fn emit_call(&mut self, call: &ast::Call) -> ssa::Value {
        let ast::Call((symbol, _), box ref args) = *call;
        let args: Vec<_> = args.iter()
            .map(|argument| self.emit_rvalue(argument))
            .collect();
        self.emit_instruction(ssa::Inst::Call { symbol, args })
    }

    fn emit_real(&mut self, real: f64) -> ssa::Value {
        let value = ssa::Constant::Real(real);
        let instruction = ssa::Inst::Immediate { value };
        self.emit_instruction(instruction)
    }

    fn emit_string(&mut self, string: Symbol) -> ssa::Value {
        let value = ssa::Constant::String(string);
        let instruction = ssa::Inst::Immediate { value };
        self.emit_instruction(instruction)
    }

    fn emit_instruction(&mut self, instruction: ssa::Inst) -> ssa::Value {
        self.builder.function.emit_instruction(self.current_block, instruction)
    }

    fn emit_initializer(&mut self, instruction: ssa::Inst) -> ssa::Value {
        let function = &mut self.builder.function;

        let value = function.values.push(instruction);
        function.blocks[ssa::ENTRY].instructions.insert(self.initializers as usize, value);
        self.initializers += 1;

        value
    }

    /// Emit a GML-level local.
    ///
    /// Because `var` declarations are not control-flow dependent, set default values in the
    /// function entry block, since that is guaranteed to dominate all uses of the local.
    fn emit_local(&mut self, default: Option<ssa::Value>) -> Local {
        let flag = self.builder.emit_local();
        let local = self.builder.emit_local();

        let value = ssa::Constant::Real(if default.is_some() { 1.0 } else { 0.0 });
        let initialized = self.emit_initializer(ssa::Inst::Immediate { value });
        self.builder.write_local(ssa::ENTRY, flag, initialized);

        let default = default.unwrap_or_else(|| {
            let value = ssa::Constant::Real(0.0);
            self.emit_initializer(ssa::Inst::Immediate { value })
        });
        self.builder.write_local(ssa::ENTRY, local, default);

        Local { flag, local }
    }

    fn make_block(&mut self) -> ssa::Block {
        self.builder.function.make_block()
    }

    fn with_loop<F>(&mut self, next: ssa::Block, exit: ssa::Block, f: F) where
        F: FnOnce(&mut Codegen)
    {
        let old_next = mem::replace(&mut self.current_next, Some(next));
        let old_exit = mem::replace(&mut self.current_exit, Some(exit));

        f(self);

        self.current_next = old_next;
        self.current_exit = old_exit;
    }

    fn with_switch<F>(
        &mut self, switch: ssa::Value, expr: ssa::Block, default: ssa::Block, exit: ssa::Block,
        f: F
    ) where F: FnOnce(&mut Codegen) {
        let old_switch = mem::replace(&mut self.current_switch, Some(switch));
        let old_expr = mem::replace(&mut self.current_expr, Some(expr));
        let old_default = mem::replace(&mut self.current_default, Some(default));
        let old_exit = mem::replace(&mut self.current_exit, Some(exit));

        f(self);

        self.current_switch = old_switch;
        self.current_expr = old_expr;
        self.current_default = old_default;
        self.current_exit = old_exit;
    }
}

impl From<ast::Binary> for ssa::Binary {
    fn from(op: ast::Binary) -> ssa::Binary {
        match op {
            ast::Binary::Lt => ssa::Binary::Lt,
            ast::Binary::Le => ssa::Binary::Le,
            ast::Binary::Eq => ssa::Binary::Eq,
            ast::Binary::Ne => ssa::Binary::Ne,
            ast::Binary::Ge => ssa::Binary::Ge,
            ast::Binary::Gt => ssa::Binary::Gt,

            ast::Binary::Op(ast::Op::Add) => ssa::Binary::Add,
            ast::Binary::Op(ast::Op::Subtract) => ssa::Binary::Subtract,
            ast::Binary::Op(ast::Op::Multiply) => ssa::Binary::Multiply,
            ast::Binary::Op(ast::Op::Divide) => ssa::Binary::Divide,
            ast::Binary::Div => ssa::Binary::Div,
            ast::Binary::Mod => ssa::Binary::Mod,

            ast::Binary::And => ssa::Binary::And,
            ast::Binary::Or => ssa::Binary::Or,
            ast::Binary::Xor => ssa::Binary::Xor,

            ast::Binary::Op(ast::Op::BitAnd) => ssa::Binary::BitAnd,
            ast::Binary::Op(ast::Op::BitOr) => ssa::Binary::BitOr,
            ast::Binary::Op(ast::Op::BitXor) => ssa::Binary::BitXor,
            ast::Binary::ShiftLeft => ssa::Binary::ShiftLeft,
            ast::Binary::ShiftRight => ssa::Binary::ShiftRight,
        }
    }
}
