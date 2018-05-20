use std::{mem, cmp, iter};
use std::collections::HashMap;

use symbol::{Symbol, keyword};
use front::{self, ast, Span, ErrorHandler};
use back::ssa;

pub struct Codegen<'p, 'e> {
    function: ssa::Function,
    builder: front::ssa::Builder,
    errors: &'e ErrorHandler,

    // TODO: replace this with an actual prototype descriptor
    prototypes: &'p HashMap<Symbol, ssa::Opcode>,

    /// GML `var` declarations are static and independent of control flow. All references to a
    /// `var`-declared name after its declaration in the source text are treated as local.
    locals: HashMap<Symbol, Local>,
    /// The number of script arguments that have been created so far.
    arguments: u32,
    /// The return value of the program.
    return_value: front::ssa::Local,

    /// The number of entry-block instructions initializing local variables. This is used as an
    /// insertion point so more can be inserted.
    initializers: usize,

    current_block: ssa::Label,

    current_next: Option<ssa::Label>,
    current_exit: Option<ssa::Label>,

    current_switch: Option<ssa::Value>,
    current_expr: Option<ssa::Label>,
    current_default: Option<ssa::Label>,
}

/// A location that can be read from or written to.
///
/// Pre-studio GML arrays are not first class values, and are instead tied to variable bindings.
/// To accomodate this, `Place` uses a `Path` rather than an `ssa::Value`. To support GMS arrays
/// and data structure accessors, `Path` might gain a `Value` variant.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
struct Place {
    path: Path,
    index: Option<[ssa::Value; 2]>,
}

/// A "path" to a variable. See `Place`.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
enum Path {
    /// A variable declared with `var`.
    Local(Symbol),
    /// An unprefixed variable referencing either `self` or `global`.
    Field(ssa::Value, Symbol),
    /// A prefixed variable dynamically referencing an instance or object.
    Scope(ssa::Value, Symbol),
}

#[derive(Debug)]
struct PlaceError;

/// A GML-level local variable.
#[derive(Copy, Clone)]
struct Local {
    /// Each local variable must dynamically track whether it has been initialized- a compile-time
    /// error for uninitialized uses would reject some valid GML programs.
    flag: front::ssa::Local,
    local: front::ssa::Local,
}

/// The header of a `with` loop.
///
/// This is extracted into a struct because it is used not only for literal `with` statements, but
/// also for loads and stores of fields.
struct With {
    cond_block: ssa::Label,
    body_block: ssa::Label,
    exit_block: ssa::Label,
    entity: ssa::Value,
}

// TODO: deduplicate these with the ones from vm::interpreter?
const SELF: f64 = -1.0;
const OTHER: f64 = -2.0;
const ALL: f64 = -3.0;
const NOONE: f64 = -4.0;
const GLOBAL: f64 = -5.0;
// -6?
const LOCAL: f64 = -7.0;

impl<'p, 'e> Codegen<'p, 'e> {
    pub fn new(prototypes: &'p HashMap<Symbol, ssa::Opcode>, errors: &'e ErrorHandler) -> Self {
        let function = ssa::Function::new();

        let mut builder = front::ssa::Builder::new();
        let return_value = builder.emit_local();

        Codegen {
            function,
            builder,
            errors,

            prototypes,

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
        self.seal_block(entry_block);

        // TODO: move this back inline with NLL
        let local = self.return_value;
        let op = ssa::Opcode::Constant;
        let zero = self.emit_initializer(ssa::Instruction::UnaryReal { op, real: 0.0 });
        self.write_local(local, zero);

        self.emit_statement(program);

        self.emit_jump(ssa::EXIT);
        self.seal_block(ssa::EXIT);

        self.current_block = ssa::EXIT;
        let locals = mem::replace(&mut self.locals, HashMap::default());
        for (_, Local { local, .. }) in locals {
            let value = self.read_local(local);
            self.emit_unary(ssa::Opcode::Release, value);
        }
        // TODO: move this back inline with NLL
        let local = self.return_value;
        let return_value = self.read_local(local);
        self.emit_unary(ssa::Opcode::Return, return_value);

        front::ssa::Builder::finish(&mut self.function);
        self.function.return_def = match self.function.blocks[ssa::ENTRY].parameters.get(0) {
            Some(&def) => def,
            None => self.function.values.push(ssa::Instruction::Parameter),
        };

        self.function
    }

    fn emit_statement(&mut self, statement: &(ast::Stmt, Span)) {
        let (ref statement, statement_span) = *statement;
        match *statement {
            ast::Stmt::Assign(op, box ref place, box ref value) => {
                let place = match self.emit_place(place) {
                    Ok(place) => place,
                    Err(PlaceError) => return,
                };

                let value = if let Some(op) = op {
                    let place = place.clone();
                    let left = self.emit_load(place);
                    let right = self.emit_value(value);

                    let op = ast::Binary::Op(op);
                    self.emit_binary(ssa::Opcode::from(op), [left, right])
                } else {
                    self.emit_value(value)
                };

                self.emit_store(place, value);
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
                            self.emit_unary_symbol(ssa::Opcode::DeclareGlobal, symbol);
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

                let expr = self.emit_value(expr);
                self.emit_branch(expr, true_block, false_block);
                self.seal_block(true_block);
                self.seal_block(false_block);

                self.current_block = true_block;
                self.emit_statement(true_branch);
                self.emit_jump(merge_block);

                if let Some(box ref false_branch) = *false_branch {
                    self.current_block = false_block;
                    self.emit_statement(false_branch);
                    self.emit_jump(merge_block);
                }

                self.seal_block(merge_block);

                self.current_block = merge_block;
            }

            ast::Stmt::Repeat(box ref expr, box ref body) => {
                let cond_block = self.make_block();
                let body_block = self.make_block();
                let exit_block = self.make_block();

                let iter = self.builder.emit_local();
                let count = self.emit_value(expr);
                self.write_local(iter, count);
                self.emit_jump(cond_block);

                self.current_block = cond_block;
                let count = self.read_local(iter);
                let one = self.emit_real(1.0);
                let next = self.emit_binary(ssa::Opcode::Subtract, [count, one]);
                self.write_local(iter, next);
                self.emit_branch(count, body_block, exit_block);
                self.seal_block(body_block);

                self.current_block = body_block;
                self.with_loop(cond_block, exit_block, |self_| {
                    self_.emit_statement(body);
                });
                self.emit_jump(cond_block);
                self.seal_block(cond_block);
                self.seal_block(exit_block);

                self.current_block = exit_block;
            }

            ast::Stmt::While(box ref expr, box ref body) => {
                let cond_block = self.make_block();
                let body_block = self.make_block();
                let exit_block = self.make_block();

                self.emit_jump(cond_block);

                self.current_block = cond_block;
                let expr = self.emit_value(expr);
                self.emit_branch(expr, body_block, exit_block);
                self.seal_block(body_block);

                self.current_block = body_block;
                self.with_loop(cond_block, exit_block, |self_| {
                    self_.emit_statement(body);
                });
                self.emit_jump(cond_block);
                self.seal_block(cond_block);
                self.seal_block(exit_block);

                self.current_block = exit_block;
            }

            ast::Stmt::Do(box ref body, box ref expr) => {
                let body_block = self.make_block();
                let cond_block = self.make_block();
                let exit_block = self.make_block();

                self.emit_jump(body_block);

                self.current_block = body_block;
                self.with_loop(cond_block, exit_block, |self_| {
                    self_.emit_statement(body);
                });
                self.emit_jump(cond_block);
                self.seal_block(cond_block);

                self.current_block = cond_block;
                let expr = self.emit_value(expr);
                self.emit_branch(expr, exit_block, body_block);
                self.seal_block(body_block);
                self.seal_block(exit_block);

                self.current_block = exit_block;
            }

            ast::Stmt::For(box ref init, box ref expr, box ref next, box ref body) => {
                let cond_block = self.make_block();
                let body_block = self.make_block();
                let next_block = self.make_block();
                let exit_block = self.make_block();

                self.emit_statement(init);
                self.emit_jump(cond_block);

                self.current_block = cond_block;
                let expr = self.emit_value(expr);
                self.emit_branch(expr, body_block, exit_block);
                self.seal_block(body_block);

                self.current_block = body_block;
                self.with_loop(next_block, exit_block, |self_| {
                    self_.emit_statement(body);
                });
                self.emit_jump(next_block);
                self.seal_block(next_block);
                self.seal_block(exit_block);

                self.current_block = next_block;
                self.emit_statement(next);
                self.emit_jump(cond_block);
                self.seal_block(cond_block);

                self.current_block = exit_block;
            }

            ast::Stmt::With(box ref expr, box ref body) => {
                let self_value = self.emit_unary_real(ssa::Opcode::LoadScope, SELF);
                let other_value = self.emit_unary_real(ssa::Opcode::LoadScope, OTHER);
                self.emit_binary_real(ssa::Opcode::StoreScope, self_value, OTHER);

                let expr = self.emit_value(expr);
                let With { cond_block, body_block, exit_block, entity } = self.emit_with(expr);
                self.seal_block(body_block);

                self.current_block = body_block;
                self.emit_binary_real(ssa::Opcode::StoreScope, entity, SELF);
                self.with_loop(cond_block, exit_block, |self_| {
                    self_.emit_statement(body);
                });
                self.emit_jump(cond_block);
                self.seal_block(cond_block);
                self.seal_block(exit_block);

                self.current_block = exit_block;
                self.emit_binary_real(ssa::Opcode::StoreScope, self_value, SELF);
                self.emit_binary_real(ssa::Opcode::StoreScope, other_value, OTHER);
            }

            ast::Stmt::Switch(box ref expr, box ref body) => {
                let expr_block = self.current_block;
                let dead_block = self.make_block();
                let exit_block = self.make_block();

                self.seal_block(dead_block);

                let expr = self.emit_value(expr);

                self.current_block = dead_block;
                self.with_switch(expr, expr_block, exit_block, |self_| {
                    for statement in body {
                        self_.emit_statement(statement);
                    }
                    self_.emit_jump(exit_block);

                    let default_block = self_.current_default.unwrap_or(exit_block);
                    self_.current_block = self_.current_expr.unwrap();
                    self_.current_expr = None;
                    self_.emit_jump(default_block);
                    if let Some(default_block) = self_.current_default {
                        self_.builder.seal_block(&mut self_.function, default_block);
                    }
                });
                self.seal_block(exit_block);

                self.current_block = exit_block;
            }

            ast::Stmt::Case(Some(box ref expr)) if self.current_expr.is_some() => {
                let case_block = self.make_block();
                let expr_block = self.make_block();

                self.emit_jump(case_block);

                self.current_block = self.current_expr.unwrap();
                self.current_expr = Some(expr_block);
                let switch = self.current_switch.expect("corrupt switch state");
                let expr = self.emit_value(expr);
                let expr = self.emit_binary(ssa::Opcode::Eq, [switch, expr]);
                self.emit_branch(expr, case_block, expr_block);
                self.seal_block(case_block);
                self.seal_block(expr_block);

                self.current_block = case_block;
            }

            ast::Stmt::Case(None) if self.current_expr.is_some() => {
                let default_block = self.make_block();
                self.current_default = Some(default_block);

                self.emit_jump(default_block);

                self.current_block = default_block;
            }

            ast::Stmt::Case(_) => {
                self.errors.error(statement_span, "case statement outside of switch");
            }

            ast::Stmt::Jump(ast::Jump::Break) if self.current_exit.is_some() => {
                let exit_block = self.current_exit.unwrap();
                let dead_block = self.make_block();

                self.emit_jump(exit_block);
                self.current_block = dead_block;
                self.seal_block(dead_block);
            }

            ast::Stmt::Jump(ast::Jump::Continue) if self.current_next.is_some() => {
                let next_block = self.current_next.unwrap();
                let dead_block = self.make_block();

                self.emit_jump(next_block);
                self.current_block = dead_block;
                self.seal_block(dead_block);
            }

            // exit and break/continue outside loops return 0
            ast::Stmt::Jump(_) => {
                let dead_block = self.make_block();

                self.emit_jump(ssa::EXIT);
                self.current_block = dead_block;
                self.seal_block(dead_block);
            }

            ast::Stmt::Return(box ref expr) => {
                let dead_block = self.make_block();

                // TODO: move this back inline with NLL
                let local = self.return_value;
                let expr = self.emit_value(expr);
                self.write_local(local, expr);
                self.emit_jump(ssa::EXIT);

                self.current_block = dead_block;
                self.seal_block(dead_block);
            }

            ast::Stmt::Error(_) => {}
        }
    }

    fn emit_value(&mut self, expression: &(ast::Expr, Span)) -> ssa::Value {
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

            ast::Expr::Unary(ast::Unary::Positive, box ref expr) => self.emit_value(expr),
            ast::Expr::Unary(op, box ref expr) => {
                let op = match op {
                    ast::Unary::Negate => ssa::Opcode::Negate,
                    ast::Unary::Invert => ssa::Opcode::Invert,
                    ast::Unary::BitInvert => ssa::Opcode::BitInvert,
                    _ => unreachable!(),
                };
                let expr = self.emit_value(expr);
                self.emit_unary(op, expr)
            }

            ast::Expr::Binary(op, box ref left, box ref right) => {
                let left = self.emit_value(left);
                let right = self.emit_value(right);
                self.emit_binary(ssa::Opcode::from(op), [left, right])
            }

            ast::Expr::Call(ref call) => self.emit_call(call),

            _ => {
                let place = self.emit_place(expression)
                    .expect("_ is not a valid expression");
                self.emit_load(place)
            }
        }
    }

    fn emit_place(&mut self, expression: &(ast::Expr, Span)) -> Result<Place, PlaceError> {
        let (ref expression, expression_span) = *expression;
        match *expression {
            ast::Expr::Value(ast::Value::Ident(symbol)) if !symbol.is_keyword() => {
                if let Some(argument) = symbol.as_argument() {
                    for argument in self.arguments..argument + 1 {
                        let symbol = Symbol::from_argument(argument);

                        let parameter = self.function.emit_parameter(ssa::ENTRY);

                        let local = self.emit_local(Some(parameter));
                        self.locals.insert(symbol, local);
                    }
                    self.arguments = cmp::max(self.arguments, argument + 1);
                }

                if self.locals.contains_key(&symbol) {
                    Ok(Place { path: Path::Local(symbol), index: None })
                } else {
                    let entity = self.emit_unary_symbol(ssa::Opcode::Lookup, symbol);
                    Ok(Place { path: Path::Field(entity, symbol), index: None })
                }
            }

            // TODO: do this as constant propagation instead? or keep as a peephole optimization?
            ast::Expr::Field(
                box (ast::Expr::Value(ast::Value::Ident(keyword::Self_)), _expr_span),
                (field, _field_span)
            ) => {
                let entity = self.emit_unary_real(ssa::Opcode::LoadScope, SELF);
                Ok(Place { path: Path::Field(entity, field), index: None })
            }
            ast::Expr::Field(
                box (ast::Expr::Value(ast::Value::Ident(keyword::Other)), _expr_span),
                (field, _field_span)
            ) => {
                let entity = self.emit_unary_real(ssa::Opcode::LoadScope, OTHER);
                Ok(Place { path: Path::Field(entity, field), index: None })
            }
            ast::Expr::Field(
                box (ast::Expr::Value(ast::Value::Ident(keyword::Global)), _expr_span),
                (field, _field_span)
            ) => {
                let entity = self.emit_unary_real(ssa::Opcode::LoadScope, GLOBAL);
                Ok(Place { path: Path::Field(entity, field), index: None })
            }

            ast::Expr::Field(box ref expr, (field, _field_span)) => {
                let scope = self.emit_value(expr);
                Ok(Place { path: Path::Scope(scope, field), index: None })
            }

            ast::Expr::Index(box ref expr, box ref indices) => {
                if indices.len() < 1 || 2 < indices.len() {
                    self.errors.error(expression_span, "invalid number of array indices");
                }

                let array = self.emit_place(expr)?;
                let zero = self.emit_real(0.0);
                let mut indices = indices.iter().rev()
                    .map(|index| self.emit_value(index))
                    .chain(iter::repeat(zero));

                let j = indices.next().unwrap();
                let i = indices.next().unwrap();

                match array {
                    Place { path, index: None } => Ok(Place { path, index: Some([i, j]) }),
                    Place { index: Some(_), .. } => {
                        let (_, expr_span) = *expr;
                        self.errors.error(expr_span, "expected a variable");
                        Err(PlaceError)
                    }
                }
            }

            _ => {
                self.errors.error(expression_span, "expected a variable");
                Err(PlaceError)
            }
        }
    }

    /// Language-level variable load.
    ///
    /// This handles GML's odd behavior around arrays. Before GMS:
    /// - all loads produce scalars; if the variable holds an array it loads `a[0, 0]`
    /// - indexed loads from scalar variables treat the variable as a 1x1 array
    fn emit_load(&mut self, place: Place) -> ssa::Value {
        let value = match place.path {
            Path::Local(symbol) => {
                let Local { flag, local } = self.locals[&symbol];

                let flag = self.read_local(flag);
                self.emit_binary_symbol(ssa::Opcode::Read, flag, symbol);

                self.read_local(local)
            }

            Path::Field(entity, field) => {
                self.emit_binary_symbol(ssa::Opcode::LoadField, entity, field)
            }

            Path::Scope(scope, field) => {
                let With { cond_block, body_block, exit_block, entity } = self.emit_with(scope);
                self.seal_block(cond_block);
                self.seal_block(body_block);
                self.seal_block(exit_block);

                self.current_block = exit_block;
                self.emit_unary(ssa::Opcode::ScopeError, scope);

                self.current_block = body_block;
                self.emit_binary_symbol(ssa::Opcode::LoadField, entity, field)
            }
        };

        let value = match place.index {
            None => value,

            Some([i, j]) => {
                // TODO: this only happens pre-gms
                let array = self.emit_unary(ssa::Opcode::ToArray, value);

                let row = self.emit_binary(ssa::Opcode::LoadRow, [array, i]);
                self.emit_binary(ssa::Opcode::LoadIndex, [row, j])
            }
        };

        // TODO: this only happens pre-gms
        self.emit_unary(ssa::Opcode::ToScalar, value)
    }

    /// Language-level variable store.
    ///
    /// This handles GML's odd behavior around arrays.
    /// - indexed stores convert variables to arrays if they are undefined or scalar
    ///
    /// Before GMS:
    /// - stores to array variables do *not* overwrite the whole array, only `a[0, 0]`
    /// - indexed stores to scalar (or undefined) variables leave the scalar (or `0`) at `a[0, 0]`
    fn emit_store(&mut self, place: Place, value: ssa::Value) {
        match place {
            Place { path: Path::Local(symbol), index: None } => {
                let Local { flag, local } = self.locals[&symbol];

                let one = self.emit_real(1.0);
                self.write_local(flag, one);

                // TODO: this only happens pre-gms
                let array = self.read_local(local);
                let value = self.emit_binary(ssa::Opcode::Write, [value, array]);

                self.write_local(local, value);
            }

            Place { path: Path::Field(entity, field), index: None } => {
                // TODO: this only happens pre-gms
                let array = self.emit_binary_symbol(ssa::Opcode::LoadFieldDefault, entity, field);
                let value = self.emit_binary(ssa::Opcode::Write, [value, array]);

                self.emit_ternary_symbol(ssa::Opcode::StoreField, [value, entity], field);
            }

            Place { path: Path::Scope(scope, field), index: None } => {
                // TODO: gms errors on empty iteration
                let With { cond_block, body_block, exit_block, entity } = self.emit_with(scope);
                self.seal_block(body_block);
                self.seal_block(exit_block);
                self.current_block = body_block;

                // TODO: this only happens pre-gms
                let array = self.emit_binary_symbol(ssa::Opcode::LoadFieldDefault, entity, field);
                let value = self.emit_binary(ssa::Opcode::Write, [value, array]);

                self.emit_ternary_symbol(ssa::Opcode::StoreField, [value, entity], field);

                self.emit_jump(cond_block);
                self.seal_block(cond_block);
                self.current_block = exit_block;
            }

            Place { path: Path::Local(symbol), index: Some([i, j]) } => {
                let Local { flag, local } = self.locals[&symbol];

                let one = self.emit_real(1.0);
                self.write_local(flag, one);

                let array = self.read_local(local);

                // TODO: this only happens pre-gms; gms does need to handle undef
                let array = self.emit_unary(ssa::Opcode::ToArray, array);
                self.write_local(local, array);

                let row = self.emit_binary(ssa::Opcode::StoreRow, [array, i]);
                self.emit_ternary(ssa::Opcode::StoreIndex, [value, row, j]);
            }

            Place { path: Path::Field(entity, field), index: Some([i, j]) } => {
                let array = self.emit_binary_symbol(ssa::Opcode::LoadFieldDefault, entity, field);

                // TODO: this only happens pre-gms; gms does need to handle undef
                let array = self.emit_unary(ssa::Opcode::ToArray, array);
                self.emit_ternary_symbol(ssa::Opcode::StoreField, [array, entity], field);

                let row = self.emit_binary(ssa::Opcode::StoreRow, [array, i]);
                self.emit_ternary(ssa::Opcode::StoreIndex, [value, row, j]);
            }

            Place { path: Path::Scope(scope, field), index: Some([i, j]) } => {
                // TODO: gms errors on empty iteration
                let With { cond_block, body_block, exit_block, entity } = self.emit_with(scope);
                self.seal_block(body_block);
                self.seal_block(exit_block);
                self.current_block = body_block;

                let array = self.emit_binary_symbol(ssa::Opcode::LoadFieldDefault, entity, field);

                // TODO: this only happens pre-gms; gms does need to handle undef
                let array = self.emit_unary(ssa::Opcode::ToArray, array);
                self.emit_ternary_symbol(ssa::Opcode::StoreField, [value, entity], field);

                let row = self.emit_binary(ssa::Opcode::StoreRow, [array, i]);
                self.emit_ternary(ssa::Opcode::StoreIndex, [value, row, j]);

                self.emit_jump(cond_block);
                self.seal_block(cond_block);
                self.current_block = exit_block;
            }
        }
    }

    /// Loop header for instance iteration.
    fn emit_with(&mut self, scope: ssa::Value) -> With {
        let cond_block = self.make_block();
        let scan_block = self.make_block();
        let body_block = self.make_block();
        let exit_block = self.make_block();

        let iter = self.builder.emit_local();
        let with = self.emit_unary(ssa::Opcode::With, scope);
        let ptr = self.function.values.push(ssa::Instruction::Project { arg: with, index: 0 });
        let end = self.function.values.push(ssa::Instruction::Project { arg: with, index: 1 });
        self.write_local(iter, ptr);
        self.emit_jump(cond_block);

        self.current_block = cond_block;
        let ptr = self.read_local(iter);
        let expr = self.emit_binary(ssa::Opcode::NePointer, [ptr, end]);
        self.emit_branch(expr, scan_block, exit_block);
        self.seal_block(scan_block);

        self.current_block = scan_block;
        let entity = self.emit_unary(ssa::Opcode::LoadPointer, ptr);
        let ptr = self.emit_unary(ssa::Opcode::NextPointer, ptr);
        self.write_local(iter, ptr);
        let exists = self.emit_unary(ssa::Opcode::ExistsEntity, entity);
        self.emit_branch(exists, body_block, cond_block);

        With { cond_block, body_block, exit_block, entity }
    }

    fn with_loop<F>(&mut self, next: ssa::Label, exit: ssa::Label, f: F) where
        F: FnOnce(&mut Codegen)
    {
        let old_next = mem::replace(&mut self.current_next, Some(next));
        let old_exit = mem::replace(&mut self.current_exit, Some(exit));

        f(self);

        self.current_next = old_next;
        self.current_exit = old_exit;
    }

    fn with_switch<F>(
        &mut self, switch: ssa::Value, expr: ssa::Label, exit: ssa::Label,
        f: F
    ) where F: FnOnce(&mut Codegen) {
        let old_switch = mem::replace(&mut self.current_switch, Some(switch));
        let old_expr = mem::replace(&mut self.current_expr, Some(expr));
        let old_default = mem::replace(&mut self.current_default, None);
        let old_exit = mem::replace(&mut self.current_exit, Some(exit));

        f(self);

        self.current_switch = old_switch;
        self.current_expr = old_expr;
        self.current_default = old_default;
        self.current_exit = old_exit;
    }

    // SSA builder utilities:

    fn make_block(&mut self) -> ssa::Label {
        self.function.make_block()
    }

    fn seal_block(&mut self, block: ssa::Label) {
        self.builder.seal_block(&mut self.function, block);
    }

    /// Emit a GML-level local.
    ///
    /// Because `var` declarations are not control-flow dependent, set default values in the
    /// function entry block, since that is guaranteed to dominate all uses of the local.
    fn emit_local(&mut self, default: Option<ssa::Value>) -> Local {
        let flag = self.builder.emit_local();
        let local = self.builder.emit_local();

        let op = ssa::Opcode::Constant;
        let real = if default.is_some() { 1.0 } else { 0.0 };
        let initialized = self.emit_initializer(ssa::Instruction::UnaryReal { op, real });
        self.builder.write_local(ssa::ENTRY, flag, initialized);

        let default = default.unwrap_or_else(|| {
            let op = ssa::Opcode::Constant;
            self.emit_initializer(ssa::Instruction::UnaryReal { op, real: 0.0 })
        });
        self.builder.write_local(ssa::ENTRY, local, default);

        Local { flag, local }
    }

    fn emit_initializer(&mut self, instruction: ssa::Instruction) -> ssa::Value {
        let value = self.function.values.push(instruction);
        self.function.blocks[ssa::ENTRY].instructions.insert(self.initializers, value);
        self.initializers += 1;
        value
    }

    fn read_local(&mut self, local: front::ssa::Local) -> ssa::Value {
        self.builder.read_local(&mut self.function, self.current_block, local)
    }

    fn write_local(&mut self, local: front::ssa::Local, value: ssa::Value) {
        self.builder.write_local(self.current_block, local, value);
    }

    // Instruction format utilities:

    fn emit_real(&mut self, real: f64) -> ssa::Value {
        self.emit_unary_real(ssa::Opcode::Constant, real)
    }

    fn emit_string(&mut self, string: Symbol) -> ssa::Value {
        self.emit_unary_symbol(ssa::Opcode::Constant, string)
    }

    fn emit_unary(&mut self, op: ssa::Opcode, arg: ssa::Value) -> ssa::Value {
        let instruction = ssa::Instruction::Unary { op, arg };
        self.function.emit_instruction(self.current_block, instruction)
    }

    fn emit_unary_real(&mut self, op: ssa::Opcode, real: f64) -> ssa::Value {
        let instruction = ssa::Instruction::UnaryReal { op, real };
        self.function.emit_instruction(self.current_block, instruction)
    }

    fn emit_unary_symbol(&mut self, op: ssa::Opcode, symbol: Symbol) -> ssa::Value {
        let instruction = ssa::Instruction::UnarySymbol { op, symbol };
        self.function.emit_instruction(self.current_block, instruction)
    }

    fn emit_binary(&mut self, op: ssa::Opcode, args: [ssa::Value; 2]) -> ssa::Value {
        let instruction = ssa::Instruction::Binary { op, args };
        self.function.emit_instruction(self.current_block, instruction)
    }

    fn emit_binary_real(&mut self, op: ssa::Opcode, arg: ssa::Value, real: f64) -> ssa::Value {
        let instruction = ssa::Instruction::BinaryReal { op, arg, real };
        self.function.emit_instruction(self.current_block, instruction)
    }

    fn emit_binary_symbol(&mut self, op: ssa::Opcode, arg: ssa::Value, symbol: Symbol) -> ssa::Value {
        let instruction = ssa::Instruction::BinarySymbol { op, arg, symbol };
        self.function.emit_instruction(self.current_block, instruction)
    }

    fn emit_ternary(&mut self, op: ssa::Opcode, args: [ssa::Value; 3]) -> ssa::Value {
        let instruction = ssa::Instruction::Ternary { op, args };
        self.function.emit_instruction(self.current_block, instruction)
    }

    fn emit_ternary_symbol(&mut self, op: ssa::Opcode, args: [ssa::Value; 2], symbol: Symbol) -> ssa::Value {
        let instruction = ssa::Instruction::TernarySymbol { op, args, symbol };
        self.function.emit_instruction(self.current_block, instruction)
    }

    fn emit_call(&mut self, call: &ast::Call) -> ssa::Value {
        let ast::Call((symbol, symbol_span), box ref args) = *call;

        let args: Vec<_> = args.iter()
            .map(|argument| self.emit_value(argument))
            .collect();

        // TODO: move this logic to live range splitting
        let parameters: Vec<_> = (0..cmp::max(1, args.len()))
            .map(|_| self.function.values.push(ssa::Instruction::Parameter))
            .collect();

        let op = self.prototypes.get(&symbol)
            .map(|&op| op)
            .unwrap_or_else(|| {
                self.errors.error(symbol_span, "function does not exist");
                ssa::Opcode::Call
            });
        let instruction = ssa::Instruction::Call { op, symbol, args, parameters };
        self.function.emit_instruction(self.current_block, instruction)
    }

    fn emit_jump(&mut self, target: ssa::Label) {
        let op = ssa::Opcode::Jump;
        let instruction = ssa::Instruction::Jump { op, target, args: vec![] };
        self.function.emit_instruction(self.current_block, instruction);

        self.builder.insert_edge(self.current_block, target);
    }

    fn emit_branch(&mut self, expr: ssa::Value, true_block: ssa::Label, false_block: ssa::Label) {
        let op = ssa::Opcode::Branch;
        let instruction = ssa::Instruction::Branch {
            op,
            targets: [true_block, false_block],
            arg_lens: [0, 0],
            args: vec![expr],
        };
        self.function.emit_instruction(self.current_block, instruction);

        self.builder.insert_edge(self.current_block, true_block);
        self.builder.insert_edge(self.current_block, false_block);
    }
}

impl From<ast::Binary> for ssa::Opcode {
    fn from(op: ast::Binary) -> ssa::Opcode {
        match op {
            ast::Binary::Lt => ssa::Opcode::Lt,
            ast::Binary::Le => ssa::Opcode::Le,
            ast::Binary::Eq => ssa::Opcode::Eq,
            ast::Binary::Ne => ssa::Opcode::Ne,
            ast::Binary::Ge => ssa::Opcode::Ge,
            ast::Binary::Gt => ssa::Opcode::Gt,

            ast::Binary::Op(ast::Op::Add) => ssa::Opcode::Add,
            ast::Binary::Op(ast::Op::Subtract) => ssa::Opcode::Subtract,
            ast::Binary::Op(ast::Op::Multiply) => ssa::Opcode::Multiply,
            ast::Binary::Op(ast::Op::Divide) => ssa::Opcode::Divide,
            ast::Binary::Div => ssa::Opcode::Div,
            ast::Binary::Mod => ssa::Opcode::Mod,

            ast::Binary::And => ssa::Opcode::And,
            ast::Binary::Or => ssa::Opcode::Or,
            ast::Binary::Xor => ssa::Opcode::Xor,

            ast::Binary::Op(ast::Op::BitAnd) => ssa::Opcode::BitAnd,
            ast::Binary::Op(ast::Op::BitOr) => ssa::Opcode::BitOr,
            ast::Binary::Op(ast::Op::BitXor) => ssa::Opcode::BitXor,
            ast::Binary::ShiftLeft => ssa::Opcode::ShiftLeft,
            ast::Binary::ShiftRight => ssa::Opcode::ShiftRight,
        }
    }
}
