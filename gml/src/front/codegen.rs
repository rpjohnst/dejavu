use std::{mem, cmp, iter};
use std::collections::HashMap;

use crate::ErrorPrinter;
use crate::symbol::{Symbol, keyword};
use crate::front::{self, ast, Span};
use crate::back::ssa;

pub struct Codegen<'p, 'e> {
    function: ssa::Function,
    builder: front::ssa::Builder,
    errors: &'e mut ErrorPrinter,

    prototypes: &'p HashMap<Symbol, ssa::Prototype>,

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
    pub fn new(
        prototypes: &'p HashMap<Symbol, ssa::Prototype>, errors: &'e mut ErrorPrinter
    ) -> Self {
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

    pub fn compile_event(self, event: &(ast::Action, Span)) -> ssa::Function {
        let (_, event_span) = *event;
        self.with_program(event_span.high, move |self_| {
            self_.emit_action(event);
        })
    }

    pub fn compile_program(self, program: &(ast::Stmt, Span)) -> ssa::Function {
        self.with_program(end_loc(program), move |self_| {
            self_.emit_statement(program);
        })
    }

    fn with_program<F>(mut self, end_loc: usize, program: F) -> ssa::Function where
        F: FnOnce(&mut Codegen<'_, '_>)
    {
        let entry_block = self.current_block;
        self.seal_block(entry_block);

        let op = ssa::Opcode::Constant;
        let zero = self.emit_initializer(ssa::Instruction::UnaryReal { op, real: 0.0 });
        self.write_local(self.return_value, zero);

        program(&mut self);

        self.emit_jump(ssa::EXIT, end_loc);
        self.seal_block(ssa::EXIT);

        self.current_block = ssa::EXIT;
        let locals = mem::replace(&mut self.locals, HashMap::default());
        for (_, Local { local, .. }) in locals {
            let value = self.read_local(local);
            self.emit_unary(ssa::Opcode::Release, value, end_loc);
        }
        let return_value = self.read_local(self.return_value);
        self.emit_unary(ssa::Opcode::Return, return_value, end_loc);

        front::ssa::Builder::finish(&mut self.function);
        self.function.return_def = match self.function.blocks[ssa::ENTRY].parameters.get(0) {
            Some(&def) => def,
            None => self.function.values.push(ssa::Instruction::Parameter),
        };

        self.function
    }

    fn emit_action(&mut self, action: &(ast::Action, Span)) {
        let (ref action, action_span) = *action;
        match *action {
            ast::Action::Normal {
                ref question, ref execution, target, relative, box ref arguments
            } => {
                let target = target.map(|target| target as f64).unwrap_or(SELF);
                // TODO: argument_relative
                let _relative = relative.unwrap_or(false);

                // TODO: move into peephole optimizer
                let value = if target == SELF {
                    self.emit_action_call(execution, arguments, action_span)
                } else {
                    let target = self.emit_real(target as f64, action_span.low);
                    self.emit_with(target, action_span.low, action_span.low, |self_| {
                        self_.emit_action_call(execution, arguments, action_span)
                    })
                };

                match *question {
                    Some(box ast::Question { negate, ref true_action, ref false_action }) => {
                        let value = if negate {
                            self.emit_unary(ssa::Opcode::Negate, value, action_span.low)
                        } else {
                            value
                        };

                        self.emit_if(
                            (value, action_span.low),
                            (|self_| self_.emit_action(true_action), action_end_loc(true_action)),
                            false_action.as_ref().map(|false_action| (
                                move |self_: &mut Codegen<'_, '_>| self_.emit_action(false_action),
                                action_end_loc(false_action)
                            ))
                        );
                    }

                    None => {}
                }
            }

            ast::Action::Block { body: box ref actions } => {
                for action in actions {
                    self.emit_action(action);
                }
            }

            ast::Action::Exit => {
                self.emit_exit(action_span.low);
            }

            ast::Action::Repeat { count: box ref expr, body: box ref body } => {
                self.emit_repeat(expr, body.1.high, |self_| {
                    self_.emit_action(body);
                });
            }

            ast::Action::Variable {
                target, relative, variable: box ref place, value: box ref value
            } => {
                let op = if relative { Some(ast::Op::Add) } else { None };
                let op = (op, action_span);

                // TODO: move into peephole optimizer
                if target as f64 == SELF {
                    self.emit_assign(op, place, value);
                } else {
                    let target = self.emit_real(target as f64, action_span.low);
                    self.emit_with(target, action_span.low, action_span.high, |self_| {
                        self_.emit_assign(op, place, value);
                    });
                }
            }

            ast::Action::Code { target, code: box ref statement } => {
                // TODO: move into peephole optimizer
                if target as f64 == SELF {
                    self.emit_statement(statement);
                } else {
                    let target = self.emit_real(target as f64, action_span.low);
                    self.emit_with(target, action_span.low, action_span.high, |self_| {
                        self_.emit_statement(statement);
                    });
                }
            }

            ast::Action::Error => {}
        }
    }

    fn emit_action_call(
        &mut self, exec: &ast::Exec, arguments: &[ast::Argument], span: Span
    ) -> ssa::Value {
        let symbol = match *exec {
            ast::Exec::Function(symbol) => (symbol, span),
            ast::Exec::Code(box ref _statement) => unimplemented!(),
        };

        let mut args = vec![];
        for argument in arguments {
            let loc = span.low;
            let value = match *argument {
                ast::Argument::Expr(box ref expr) => self.emit_value(expr),
                ast::Argument::String(symbol) => self.emit_string(symbol, loc),
                ast::Argument::Bool(value) => self.emit_real(value as u64 as f64, loc),
                ast::Argument::Menu(value) => self.emit_real(value as f64, loc),
                ast::Argument::Sprite(value) => self.emit_real(value as f64, loc),
                ast::Argument::Sound(value) => self.emit_real(value as f64, loc),
                ast::Argument::Background(value) => self.emit_real(value as f64, loc),
                ast::Argument::Path(value) => self.emit_real(value as f64, loc),
                ast::Argument::Script(value) => self.emit_real(value as f64, loc),
                ast::Argument::Object(value) => self.emit_real(value as f64, loc),
                ast::Argument::Room(value) => self.emit_real(value as f64, loc),
                ast::Argument::Font(value) => self.emit_real(value as f64, loc),
                ast::Argument::Color(value) => self.emit_real(value as f64, loc),
                ast::Argument::Timeline(value) => self.emit_real(value as f64, loc),
                ast::Argument::FontString(symbol) => self.emit_string(symbol, loc),
                ast::Argument::Error => self.emit_real(0.0, loc),
            };
            args.push(value);
        }

        self.emit_value_call(symbol, args)
    }

    fn emit_statement(&mut self, statement: &(ast::Stmt, Span)) {
        let (ref statement, statement_span) = *statement;
        match *statement {
            ast::Stmt::Assign(op, box ref place, box ref value) => {
                self.emit_assign(op, place, value);
            }

            ast::Stmt::Invoke(ast::Call(symbol, box ref args)) => {
                let args: Vec<_> = args.iter()
                    .map(|argument| self.emit_value(argument))
                    .collect();
                self.emit_value_call(symbol, args);
            }

            ast::Stmt::Declare(scope, box ref names) => {
                let names: Vec<_> = names.iter().filter_map(|&(name, name_span)| {
                    if name.is_argument() {
                        self.errors.error(name_span,
                            format_args!("cannot redeclare a builtin variable"));
                        return None;
                    }

                    Some((name, name_span))
                }).collect();

                match scope {
                    ast::Declare::Local => {
                        for (symbol, _symbol_span) in names {
                            let local = self.emit_local(None);
                            self.locals.insert(symbol, local);
                        }
                    }

                    ast::Declare::Global => {
                        for (name, name_span) in names {
                            self.emit_unary_symbol(ssa::Opcode::DeclareGlobal, name, name_span.low);
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
                let value = self.emit_value(expr);
                self.emit_if(
                    (value, loc(expr)),
                    (|self_| self_.emit_statement(true_branch), end_loc(true_branch)),
                    false_branch.as_ref().map(|false_branch| {
                        (move |self_: &mut Codegen<'_, '_>| self_.emit_statement(false_branch), end_loc(false_branch))
                    })
                );
            }

            ast::Stmt::Repeat(box ref expr, box ref body) => {
                self.emit_repeat(expr, end_loc(body), |self_| {
                    self_.emit_statement(body);
                });
            }

            ast::Stmt::While(box ref expr, box ref body) => {
                let cond_block = self.make_block();
                let body_block = self.make_block();
                let exit_block = self.make_block();

                self.emit_jump(cond_block, statement_span.low);

                self.current_block = cond_block;
                let value = self.emit_value(expr);
                self.emit_branch(value, body_block, exit_block, loc(expr));
                self.seal_block(body_block);

                self.current_block = body_block;
                self.with_loop(cond_block, exit_block, |self_| {
                    self_.emit_statement(body);
                });
                self.emit_jump(cond_block, end_loc(body));
                self.seal_block(cond_block);
                self.seal_block(exit_block);

                self.current_block = exit_block;
            }

            ast::Stmt::Do(box ref body, box ref expr) => {
                let body_block = self.make_block();
                let cond_block = self.make_block();
                let exit_block = self.make_block();

                self.emit_jump(body_block, statement_span.low);

                self.current_block = body_block;
                self.with_loop(cond_block, exit_block, |self_| {
                    self_.emit_statement(body);
                });
                self.emit_jump(cond_block, end_loc(body));
                self.seal_block(cond_block);

                self.current_block = cond_block;
                let value = self.emit_value(expr);
                self.emit_branch(value, exit_block, body_block, loc(expr));
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
                self.emit_jump(cond_block, end_loc(init));

                self.current_block = cond_block;
                let value = self.emit_value(expr);
                self.emit_branch(value, body_block, exit_block, loc(expr));
                self.seal_block(body_block);

                self.current_block = body_block;
                self.with_loop(next_block, exit_block, |self_| {
                    self_.emit_statement(body);
                });
                self.emit_jump(next_block, end_loc(body));
                self.seal_block(next_block);
                self.seal_block(exit_block);

                self.current_block = next_block;
                self.emit_statement(next);
                self.emit_jump(cond_block, end_loc(next));
                self.seal_block(cond_block);

                self.current_block = exit_block;
            }

            ast::Stmt::With(box ref expr, box ref body) => {
                let target = self.emit_value(expr);
                self.emit_with(target, statement_span.low, end_loc(body), |self_| {
                    self_.emit_statement(body);
                });
            }

            ast::Stmt::Switch(box ref expr, box ref body) => {
                let expr_block = self.current_block;
                let dead_block = self.make_block();
                let exit_block = self.make_block();

                self.seal_block(dead_block);

                let value = self.emit_value(expr);

                self.current_block = dead_block;
                self.with_switch(value, expr_block, exit_block, |self_| {
                    for statement in body {
                        self_.emit_statement(statement);
                    }
                    self_.emit_jump(exit_block, statement_span.high - 1);

                    let default_block = self_.current_default.unwrap_or(exit_block);
                    self_.current_block = self_.current_expr.unwrap();
                    self_.current_expr = None;
                    self_.emit_jump(default_block, loc(expr));
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

                self.emit_jump(case_block, statement_span.low);

                self.current_block = self.current_expr.unwrap();
                self.current_expr = Some(expr_block);
                let switch = self.current_switch.expect("corrupt switch state");
                let value = self.emit_value(expr);
                let value = self.emit_binary(ssa::Opcode::Eq, [switch, value], loc(expr));
                self.emit_branch(value, case_block, expr_block, loc(expr));
                self.seal_block(case_block);
                self.seal_block(expr_block);

                self.current_block = case_block;
            }

            ast::Stmt::Case(None) if self.current_expr.is_some() => {
                let default_block = self.make_block();
                self.current_default = Some(default_block);

                self.emit_jump(default_block, statement_span.low);

                self.current_block = default_block;
            }

            ast::Stmt::Case(_) => {
                self.errors.error(statement_span, format_args!("case statement outside of switch"));
            }

            ast::Stmt::Jump(ast::Jump::Break) if self.current_exit.is_some() => {
                let exit_block = self.current_exit.unwrap();
                let dead_block = self.make_block();

                self.emit_jump(exit_block, statement_span.low);
                self.current_block = dead_block;
                self.seal_block(dead_block);
            }

            ast::Stmt::Jump(ast::Jump::Continue) if self.current_next.is_some() => {
                let next_block = self.current_next.unwrap();
                let dead_block = self.make_block();

                self.emit_jump(next_block, statement_span.low);
                self.current_block = dead_block;
                self.seal_block(dead_block);
            }

            // exit and break/continue outside loops return 0
            ast::Stmt::Jump(_) => {
                self.emit_exit(statement_span.low);
            }

            ast::Stmt::Return(box ref expr) => {
                let dead_block = self.make_block();

                let expr = self.emit_value(expr);
                self.write_local(self.return_value, expr);
                self.emit_jump(ssa::EXIT, statement_span.low);

                self.current_block = dead_block;
                self.seal_block(dead_block);
            }

            ast::Stmt::Error(_) => {}
        }
    }

    fn emit_assign(
        &mut self, op: (Option<ast::Op>, Span), place: &(ast::Expr, Span), value: &(ast::Expr, Span)
    ) {
        let (op, op_span) = op;

        let (_, place_span) = *place;
        let place = match self.emit_place(place) {
            Ok(place) => place,
            Err(PlaceError) => return,
        };

        let value = if let Some(op) = op {
            let place = place.clone();
            let left = self.emit_load(place, place_span);
            let right = self.emit_value(value);

            let op = ast::Binary::Op(op);
            self.emit_binary(ssa::Opcode::from(op), [left, right], op_span.low)
        } else {
            self.emit_value(value)
        };

        self.emit_store(place, value, op_span.low);
    }

    fn emit_if<T, F>(
        &mut self,
        value: (ssa::Value, usize),
        true_branch: (T, usize),
        false_branch: Option<(F, usize)>,
    ) where
        T: FnOnce(&mut Codegen<'_, '_>),
        F: FnOnce(&mut Codegen<'_, '_>),
    {
        let true_block = self.make_block();
        let false_block = self.make_block();
        let merge_block = if false_branch.is_some() {
            self.make_block()
        } else {
            false_block
        };

        let (value, value_loc) = value;
        self.emit_branch(value, true_block, false_block, value_loc);
        self.seal_block(true_block);
        self.seal_block(false_block);

        self.current_block = true_block;
        let (true_branch, true_loc) = true_branch;
        true_branch(self);
        self.emit_jump(merge_block, true_loc);

        if let Some((false_branch, false_loc)) = false_branch {
            self.current_block = false_block;
            false_branch(self);
            self.emit_jump(merge_block, false_loc);
        }

        self.seal_block(merge_block);
        self.current_block = merge_block;
    }

    fn emit_repeat<F>(&mut self, expr: &(ast::Expr, Span), end_loc: usize, body: F) where
        F: FnOnce(&mut Codegen<'_, '_>)
    {
        let cond_block = self.make_block();
        let body_block = self.make_block();
        let exit_block = self.make_block();

        let iter = self.builder.emit_local();
        let count = self.emit_value(expr);
        self.write_local(iter, count);
        self.emit_jump(cond_block, loc(expr));

        self.current_block = cond_block;
        let count = self.read_local(iter);
        let one = self.emit_real(1.0, loc(expr));
        let next = self.emit_binary(ssa::Opcode::Subtract, [count, one], loc(expr));
        self.write_local(iter, next);
        self.emit_branch(count, body_block, exit_block, loc(expr));
        self.seal_block(body_block);

        self.current_block = body_block;
        self.with_loop(cond_block, exit_block, move |self_| {
            body(self_);
        });
        self.emit_jump(cond_block, end_loc);
        self.seal_block(cond_block);
        self.seal_block(exit_block);

        self.current_block = exit_block;
    }

    fn emit_with<F, R>(
        &mut self, target: ssa::Value, with_loc: usize, end_loc: usize, body: F
    ) -> R where
        F: FnOnce(&mut Codegen<'_, '_>) -> R
    {
        let self_value = self.emit_unary_real(ssa::Opcode::LoadScope, SELF, with_loc);
        let other_value = self.emit_unary_real(ssa::Opcode::LoadScope, OTHER, with_loc);
        self.emit_binary_real(ssa::Opcode::StoreScope, self_value, OTHER, with_loc);

        let With {
            cond_block,
            body_block,
            exit_block,
            entity
        } = self.emit_with_header(target, with_loc);
        self.seal_block(body_block);

        self.current_block = body_block;
        self.emit_binary_real(ssa::Opcode::StoreScope, entity, SELF, with_loc);
        let result = self.with_loop(cond_block, exit_block, move |self_| {
            body(self_)
        });
        self.emit_jump(cond_block, end_loc);
        self.seal_block(cond_block);
        self.seal_block(exit_block);

        self.current_block = exit_block;
        self.emit_binary_real(ssa::Opcode::StoreScope, self_value, SELF, end_loc);
        self.emit_binary_real(ssa::Opcode::StoreScope, other_value, OTHER, end_loc);

        result
    }

    fn emit_exit(&mut self, loc: usize) {
        let dead_block = self.make_block();

        self.emit_jump(ssa::EXIT, loc);
        self.current_block = dead_block;
        self.seal_block(dead_block);
    }

    fn emit_value(&mut self, expression: &(ast::Expr, Span)) -> ssa::Value {
        let (ref expr, expr_span) = *expression;
        let expr_loc = expr_span.low;
        match *expr {
            ast::Expr::Value(ast::Value::Real(real)) => self.emit_real(real, expr_loc),
            ast::Expr::Value(ast::Value::String(string)) => self.emit_string(string, expr_loc),

            ast::Expr::Value(ast::Value::Ident(keyword::True)) => self.emit_real(1.0, expr_loc),
            ast::Expr::Value(ast::Value::Ident(keyword::False)) => self.emit_real(0.0, expr_loc),
            ast::Expr::Value(ast::Value::Ident(keyword::Self_)) => self.emit_real(SELF, expr_loc),
            ast::Expr::Value(ast::Value::Ident(keyword::Other)) => self.emit_real(OTHER, expr_loc),
            ast::Expr::Value(ast::Value::Ident(keyword::All)) => self.emit_real(ALL, expr_loc),
            ast::Expr::Value(ast::Value::Ident(keyword::NoOne)) => self.emit_real(NOONE, expr_loc),
            ast::Expr::Value(ast::Value::Ident(keyword::Global)) => self.emit_real(GLOBAL, expr_loc),
            ast::Expr::Value(ast::Value::Ident(keyword::Local)) => self.emit_real(LOCAL, expr_loc),

            ast::Expr::Unary((ast::Unary::Positive, _), box ref expr) => self.emit_value(expr),
            ast::Expr::Unary((op, op_span), box ref expr) => {
                let op = match op {
                    ast::Unary::Negate => ssa::Opcode::Negate,
                    ast::Unary::Invert => ssa::Opcode::Invert,
                    ast::Unary::BitInvert => ssa::Opcode::BitInvert,
                    _ => unreachable!(),
                };
                let expr = self.emit_value(expr);
                self.emit_unary(op, expr, op_span.low)
            }

            ast::Expr::Binary((op, op_span), box ref left, box ref right) => {
                let left = self.emit_value(left);
                let right = self.emit_value(right);
                self.emit_binary(ssa::Opcode::from(op), [left, right], op_span.low)
            }

            ast::Expr::Call(ast::Call(symbol, box ref args)) => {
                let args: Vec<_> = args.iter()
                    .map(|argument| self.emit_value(argument))
                    .collect();
                self.emit_value_call(symbol, args)
            }

            _ => {
                let place = self.emit_place(expression)
                    .expect("_ is not a valid expression");
                self.emit_load(place, expr_span)
            }
        }
    }

    fn emit_value_call(&mut self, symbol: (Symbol, Span), args: Vec<ssa::Value>) -> ssa::Value {
        let (symbol, symbol_span) = symbol;

        let (op, arity, variadic) = match self.prototypes.get(&symbol) {
            Some(&ssa::Prototype::Script { .. }) =>
                (ssa::Opcode::Call, 0, true),
            Some(&ssa::Prototype::Native { arity, variadic }) =>
                (ssa::Opcode::CallApi, arity, variadic),
            _ => {
                self.errors.error(symbol_span,
                    format_args!("unknown function or script: {}", symbol));
                (ssa::Opcode::Call, 0, true)
            }
        };

        if args.len() < arity || (!variadic && args.len() > arity) {
            self.errors.error(symbol_span,
                format_args!("wrong number of arguments to function or script"));
            return self.emit_real(0.0, symbol_span.low);
        }

        let array = self.emit_call(op, symbol, args, symbol_span.low);

        // TODO: this only happens pre-gms
        let value = self.emit_unary(ssa::Opcode::ToScalar, array, symbol_span.low);
        self.emit_unary(ssa::Opcode::Release, array, symbol_span.low);
        value
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
                    // Built-in variables are always local; globalvar cannot redeclare them.
                    // TODO: move into peephole optimizer
                    let entity = if self.field_is_builtin(symbol) {
                        self.emit_unary_real(ssa::Opcode::LoadScope, SELF, expression_span.low)
                    } else {
                        self.emit_unary_symbol(ssa::Opcode::Lookup, symbol, expression_span.low)
                    };
                    Ok(Place { path: Path::Field(entity, symbol), index: None })
                }
            }

            // TODO: move into peephole optimizer
            ast::Expr::Field(
                box (ast::Expr::Value(ast::Value::Ident(keyword::Self_)), expr_span),
                (field, _field_span)
            ) => {
                let entity = self.emit_unary_real(ssa::Opcode::LoadScope, SELF, expr_span.low);
                Ok(Place { path: Path::Field(entity, field), index: None })
            }
            ast::Expr::Field(
                box (ast::Expr::Value(ast::Value::Ident(keyword::Other)), expr_span),
                (field, _field_span)
            ) => {
                let entity = self.emit_unary_real(ssa::Opcode::LoadScope, OTHER, expr_span.low);
                Ok(Place { path: Path::Field(entity, field), index: None })
            }
            ast::Expr::Field(
                box (ast::Expr::Value(ast::Value::Ident(keyword::Global)), expr_span),
                (field, _field_span)
            ) => {
                let entity = self.emit_unary_real(ssa::Opcode::LoadScope, GLOBAL, expr_span.low);
                Ok(Place { path: Path::Field(entity, field), index: None })
            }

            ast::Expr::Field(box ref expr, (field, _field_span)) => {
                let scope = self.emit_value(expr);
                Ok(Place { path: Path::Scope(scope, field), index: None })
            }

            ast::Expr::Index(box ref expr, box ref indices) => {
                if indices.len() < 1 || 2 < indices.len() {
                    self.errors.error(expression_span,
                        format_args!("invalid number of array indices"));
                }

                let array = self.emit_place(expr)?;
                let zero = self.emit_real(0.0, loc(expr));
                let mut indices = indices.iter().rev()
                    .map(|index| self.emit_value(index))
                    .chain(iter::repeat(zero));

                let j = indices.next().unwrap();
                let i = indices.next().unwrap();

                match array {
                    Place { path, index: None } => Ok(Place { path, index: Some([i, j]) }),
                    Place { index: Some(_), .. } => {
                        let (_, expr_span) = *expr;
                        self.errors.error(expr_span, format_args!("expected a variable"));
                        Err(PlaceError)
                    }
                }
            }

            _ => {
                self.errors.error(expression_span, format_args!("expected a variable"));
                Err(PlaceError)
            }
        }
    }

    /// Language-level variable load.
    ///
    /// This handles GML's odd behavior around arrays. Before GMS:
    /// - all loads produce scalars; if the variable holds an array it loads `a[0, 0]`
    /// - indexed loads from scalar variables treat the variable as a 1x1 array
    fn emit_load(&mut self, place: Place, place_span: Span) -> ssa::Value {
        let value = match place {
            // A locally-declared variable: check for initialization, then read it.
            Place { path: Path::Local(symbol), index } => {
                let Local { flag, local } = self.locals[&symbol];

                let flag = self.read_local(flag);
                self.emit_binary_symbol(ssa::Opcode::Read, flag, symbol, place_span.low);

                let value = self.read_local(local);
                match index {
                    None => value,
                    Some(index) => self.emit_load_index(value, index, place_span.low),
                }
            }

            // A built-in member variable: call its getter.
            Place { path: Path::Field(entity, field), index } if
                self.field_is_builtin(field) && !self.entity_is_global(entity)
            => {
                self.emit_load_builtin(entity, field, index, place_span.low)
            }

            // A user-defined member variable: read it.
            Place { path: Path::Field(entity, field), index } => {
                let value = self.emit_binary_symbol(ssa::Opcode::LoadField, entity, field, place_span.low);
                match index {
                    None => value,
                    Some(index) => self.emit_load_index(value, index, place_span.low),
                }
            }

            // A built-in member variable on a scope: check for `global`, then call its getter.
            // (`global` does not have built-in variables, and so must fall back to the
            // user-defined case as above.)
            // TODO: fallback only happens pre-gms.
            Place { path: Path::Scope(scope, field), index } if
                self.field_is_builtin(field)
            => {
                let true_block = self.make_block();
                let false_block = self.make_block();
                let merge_block = self.make_block();

                let load = self.builder.emit_local();
                let global = self.emit_real(GLOBAL, place_span.low);
                let expr = self.emit_binary(ssa::Opcode::Ne, [scope, global], place_span.low);
                self.emit_branch(expr, true_block, false_block, place_span.low);
                self.seal_block(true_block);
                self.seal_block(false_block);

                self.current_block = true_block;
                let entity = self.emit_load_scope(scope, place_span.low);
                let value = self.emit_load_builtin(entity, field, index, place_span.low);
                self.write_local(load, value);
                self.emit_jump(merge_block, place_span.low);

                self.current_block = false_block;
                let entity = self.emit_unary_real(ssa::Opcode::LoadScope, GLOBAL, place_span.low);
                let value = self.emit_binary_symbol(ssa::Opcode::LoadField, entity, field, place_span.low);
                let value = match index {
                    None => value,
                    Some(index) => self.emit_load_index(value, index, place_span.low),
                };
                self.write_local(load, value);
                self.emit_jump(merge_block, place_span.low);

                self.seal_block(merge_block);
                self.current_block = merge_block;
                self.read_local(load)
            }

            // A user-defined member variable on a scope: locate the first entity, then read it.
            Place { path: Path::Scope(scope, field), index } => {
                let entity = self.emit_load_scope(scope, place_span.low);
                let value = self.emit_binary_symbol(ssa::Opcode::LoadField, entity, field, place_span.low);
                match index {
                    None => value,
                    Some(index) => self.emit_load_index(value, index, place_span.low),
                }
            }
        };

        // TODO: this only happens pre-gms
        self.emit_unary(ssa::Opcode::ToScalar, value, place_span.low)
    }

    /// Resolve a scope to its first entity for reading. (Helper for `emit_load`.)
    fn emit_load_scope(&mut self, scope: ssa::Value, location: usize) -> ssa::Value {
        let With { cond_block, body_block, exit_block, entity } = self.emit_with_header(scope, location);
        self.seal_block(cond_block);
        self.seal_block(body_block);
        self.seal_block(exit_block);

        self.current_block = exit_block;
        self.emit_unary(ssa::Opcode::ScopeError, scope, location);

        self.current_block = body_block;
        entity
    }

    /// Built-in member variable load. (Helper for `emit_load`.)
    fn emit_load_builtin(
        &mut self, entity: ssa::Value, field: Symbol, index: Option<[ssa::Value; 2]>, location: usize
    ) -> ssa::Value {
        // GML adjusts the given index arity to match the built-in variable.
        // TODO: pass a typed index to avoid conversions in CallGet
        let i = index.map_or_else(|| self.emit_real(0.0, location), |[_, j]| j);
        self.emit_call(ssa::Opcode::CallGet, field, vec![entity, i], location)
    }

    /// Load an element of an array. (Helper for `emit_load`.)
    fn emit_load_index(&mut self, value: ssa::Value, [i, j]: [ssa::Value; 2], location: usize) -> ssa::Value {
        // TODO: this only happens pre-gms
        let array = self.emit_unary(ssa::Opcode::ToArray, value, location);

        let row = self.emit_binary(ssa::Opcode::LoadRow, [array, i], location);
        let value = self.emit_binary(ssa::Opcode::LoadIndex, [row, j], location);

        // TODO: this only happens pre-gms
        self.emit_unary(ssa::Opcode::Release, array, location);
        value
    }

    /// Language-level variable store.
    ///
    /// This handles GML's odd behavior around arrays.
    /// - indexed stores convert variables to arrays if they are undefined or scalar
    ///
    /// Before GMS:
    /// - stores to array variables do *not* overwrite the whole array, only `a[0, 0]`
    /// - indexed stores to scalar (or undefined) variables leave the scalar (or `0`) at `a[0, 0]`
    // TODO: free overwritten arrays for gms
    fn emit_store(&mut self, place: Place, value: ssa::Value, location: usize) {
        match place {
            // A locally-declared variable: mark as initialized, then write it.
            Place { path: Path::Local(symbol), index } => {
                let Local { flag, local } = self.locals[&symbol];

                let one = self.emit_real(1.0, location);
                self.write_local(flag, one);

                match index {
                    None => {
                        // TODO: this only happens pre-gms
                        let array = self.read_local(local);
                        let value = self.emit_binary(ssa::Opcode::Write, [value, array], location);

                        self.write_local(local, value);
                    }
                    Some([i, j]) => {
                        let array = self.read_local(local);

                        // TODO: this only happens pre-gms; gms does need to handle undef
                        let array = self.emit_unary(ssa::Opcode::ToArray, array, location);
                        self.write_local(local, array);

                        let row = self.emit_binary(ssa::Opcode::StoreRow, [array, i], location);
                        self.emit_ternary(ssa::Opcode::StoreIndex, [value, row, j], location);
                    }
                }
            }

            // A built-in member variable: call its setter.
            Place { path: Path::Field(entity, field), index } if
                self.field_is_builtin(field) && !self.entity_is_global(entity)
            => {
                self.emit_store_builtin(entity, field, index, value, location);
            }

            // A user-defined member variable: write it.
            Place { path: Path::Field(entity, field), index } => {
                self.emit_store_field(entity, field, index, value, location);
            }

            // A built-in member variable on a scope: check for `global`, then call its setter.
            // (`global` does not have built-in variables, and so must fall back to the
            // user-defined case as above.)
            // TODO: fallback only happens pre-gms.
            Place { path: Path::Scope(scope, field), index } if
                self.field_is_builtin(field)
            => {
                let true_block = self.make_block();
                let false_block = self.make_block();
                let merge_block = self.make_block();

                let global = self.emit_real(GLOBAL, location);
                let expr = self.emit_binary(ssa::Opcode::Ne, [scope, global], location);
                self.emit_branch(expr, true_block, false_block, location);
                self.seal_block(true_block);
                self.seal_block(false_block);

                self.current_block = true_block;
                self.emit_store_scope(scope, location, |self_, entity| {
                    self_.emit_store_builtin(entity, field, index, value, location);
                });
                self.emit_jump(merge_block, location);

                self.current_block = false_block;
                let entity = self.emit_unary_real(ssa::Opcode::LoadScope, GLOBAL, location);
                self.emit_store_field(entity, field, index, value, location);
                self.emit_jump(merge_block, location);

                self.seal_block(merge_block);
                self.current_block = merge_block;
            }

            // A user-defined member variable on a scope: write to all entities.
            Place { path: Path::Scope(scope, field), index } => {
                self.emit_store_scope(scope, location, |self_, entity| {
                    self_.emit_store_field(entity, field, index, value, location);
                });
            }
        }
    }

    /// Iterate over each entity in a scope for writing. (Helper for `emit_store`.)
    fn emit_store_scope<F>(&mut self, scope: ssa::Value, location: usize, f: F) where
        F: FnOnce(&mut Codegen<'_, '_>, ssa::Value)
    {
        // TODO: gms errors on empty iteration
        let With { cond_block, body_block, exit_block, entity } = self.emit_with_header(scope, location);
        self.seal_block(body_block);
        self.seal_block(exit_block);
        self.current_block = body_block;

        f(self, entity);

        self.emit_jump(cond_block, location);
        self.seal_block(cond_block);
        self.current_block = exit_block;
    }

    /// Built-in member variable store. (Helper for `emit_store`.)
    fn emit_store_builtin(
        &mut self, entity: ssa::Value, field: Symbol, index: Option<[ssa::Value; 2]>,
        value: ssa::Value,
        location: usize,
    ) {
        // GML adjusts the given index arity to match the built-in variable.
        // TODO: pass a typed index to avoid conversions in CallGet
        let i = index.map_or_else(|| self.emit_real(0.0, location), |[_, j]| j);
        self.emit_call(ssa::Opcode::CallSet, field, vec![value, entity, i], location);
    }

    /// Store to an entity field. (Helper for `emit_store`.)
    ///
    /// Note that this is the same pattern as `emit_store`'s `Path::Local` arm.
    fn emit_store_field(
        &mut self, entity: ssa::Value, field: Symbol, index: Option<[ssa::Value; 2]>,
        value: ssa::Value,
        location: usize,
    ) {
        match index {
            None => {
                // TODO: this only happens pre-gms
                let array = self.emit_binary_symbol(ssa::Opcode::LoadFieldDefault, entity, field, location);
                let value = self.emit_binary(ssa::Opcode::Write, [value, array], location);

                self.emit_ternary_symbol(ssa::Opcode::StoreField, [value, entity], field, location);
            }
            Some([i, j]) => {
                let array = self.emit_binary_symbol(ssa::Opcode::LoadFieldDefault, entity, field, location);

                // TODO: this only happens pre-gms; gms does need to handle undef
                let array = self.emit_unary(ssa::Opcode::ToArray, array, location);
                self.emit_ternary_symbol(ssa::Opcode::StoreField, [array, entity], field, location);

                let row = self.emit_binary(ssa::Opcode::StoreRow, [array, i], location);
                self.emit_ternary(ssa::Opcode::StoreIndex, [value, row, j], location);
            }
        }
    }

    /// Loop header for instance iteration.
    fn emit_with_header(&mut self, scope: ssa::Value, location: usize) -> With {
        let cond_block = self.make_block();
        let scan_block = self.make_block();
        let body_block = self.make_block();
        let exit_block = self.make_block();

        let iter = self.builder.emit_local();
        let with = self.emit_unary(ssa::Opcode::With, scope, location);
        let ptr = self.function.values.push(ssa::Instruction::Project { arg: with, index: 0 });
        let end = self.function.values.push(ssa::Instruction::Project { arg: with, index: 1 });
        self.write_local(iter, ptr);
        self.emit_jump(cond_block, location);

        self.current_block = cond_block;
        let ptr = self.read_local(iter);
        let expr = self.emit_binary(ssa::Opcode::NePointer, [ptr, end], location);
        self.emit_branch(expr, scan_block, exit_block, location);
        self.seal_block(scan_block);

        self.current_block = scan_block;
        let entity = self.emit_unary(ssa::Opcode::LoadPointer, ptr, location);
        let ptr = self.emit_unary(ssa::Opcode::NextPointer, ptr, location);
        self.write_local(iter, ptr);
        let exists = self.emit_unary(ssa::Opcode::ExistsEntity, entity, location);
        self.emit_branch(exists, body_block, cond_block, location);

        self.current_block = exit_block;
        self.emit_nullary(ssa::Opcode::ReleaseWith, location);

        With { cond_block, body_block, exit_block, entity }
    }

    fn with_loop<F, R>(&mut self, next: ssa::Label, exit: ssa::Label, f: F) -> R where
        F: FnOnce(&mut Codegen<'_, '_>) -> R
    {
        let old_next = mem::replace(&mut self.current_next, Some(next));
        let old_exit = mem::replace(&mut self.current_exit, Some(exit));

        let result = f(self);

        self.current_next = old_next;
        self.current_exit = old_exit;

        result
    }

    fn with_switch<F>(
        &mut self, switch: ssa::Value, expr: ssa::Label, exit: ssa::Label,
        f: F
    ) where F: FnOnce(&mut Codegen<'_, '_>) {
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

    // SSA inspection utilities:

    fn field_is_builtin(&self, field: Symbol) -> bool {
        match self.prototypes.get(&field) {
            Some(&ssa::Prototype::Member) => true,
            _ => false,
        }
    }

    // TODO: move into peephole optimizer
    fn entity_is_global(&self, entity: ssa::Value) -> bool {
        match self.function.values[entity] {
            ssa::Instruction::UnaryReal { op: ssa::Opcode::LoadScope, real } => {
                real == GLOBAL
            }
            _ => false,
        }
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

    fn emit_real(&mut self, real: f64, location: usize) -> ssa::Value {
        self.emit_unary_real(ssa::Opcode::Constant, real, location)
    }

    fn emit_string(&mut self, string: Symbol, location: usize) -> ssa::Value {
        self.emit_unary_symbol(ssa::Opcode::Constant, string, location)
    }

    fn emit_nullary(&mut self, op: ssa::Opcode, location: usize) -> ssa::Value {
        let instruction = ssa::Instruction::Nullary { op };
        self.function.emit_instruction(self.current_block, instruction, location)
    }

    fn emit_unary(&mut self, op: ssa::Opcode, arg: ssa::Value, location: usize) -> ssa::Value {
        let instruction = ssa::Instruction::Unary { op, arg };
        self.function.emit_instruction(self.current_block, instruction, location)
    }

    fn emit_unary_real(&mut self, op: ssa::Opcode, real: f64, location: usize) -> ssa::Value {
        let instruction = ssa::Instruction::UnaryReal { op, real };
        self.function.emit_instruction(self.current_block, instruction, location)
    }

    fn emit_unary_symbol(&mut self, op: ssa::Opcode, symbol: Symbol, location: usize) -> ssa::Value {
        let instruction = ssa::Instruction::UnarySymbol { op, symbol };
        self.function.emit_instruction(self.current_block, instruction, location)
    }

    fn emit_binary(&mut self, op: ssa::Opcode, args: [ssa::Value; 2], location: usize) -> ssa::Value {
        let instruction = ssa::Instruction::Binary { op, args };
        self.function.emit_instruction(self.current_block, instruction, location)
    }

    fn emit_binary_real(&mut self, op: ssa::Opcode, arg: ssa::Value, real: f64, location: usize) -> ssa::Value {
        let instruction = ssa::Instruction::BinaryReal { op, arg, real };
        self.function.emit_instruction(self.current_block, instruction, location)
    }

    fn emit_binary_symbol(&mut self, op: ssa::Opcode, arg: ssa::Value, symbol: Symbol, location: usize) -> ssa::Value {
        let instruction = ssa::Instruction::BinarySymbol { op, arg, symbol };
        self.function.emit_instruction(self.current_block, instruction, location)
    }

    fn emit_ternary(&mut self, op: ssa::Opcode, args: [ssa::Value; 3], location: usize) -> ssa::Value {
        let instruction = ssa::Instruction::Ternary { op, args };
        self.function.emit_instruction(self.current_block, instruction, location)
    }

    fn emit_ternary_symbol(&mut self, op: ssa::Opcode, args: [ssa::Value; 2], symbol: Symbol, location: usize) -> ssa::Value {
        let instruction = ssa::Instruction::TernarySymbol { op, args, symbol };
        self.function.emit_instruction(self.current_block, instruction, location)
    }

    fn emit_call(&mut self, op: ssa::Opcode, symbol: Symbol, args: Vec<ssa::Value>, location: usize) -> ssa::Value {
        // TODO: move this logic to live range splitting
        let parameters: Vec<_> = (0..cmp::max(1, args.len()))
            .map(|_| self.function.values.push(ssa::Instruction::Parameter))
            .collect();

        let instruction = ssa::Instruction::Call { op, symbol, args, parameters };
        self.function.emit_instruction(self.current_block, instruction, location)
    }

    fn emit_jump(&mut self, target: ssa::Label, location: usize) {
        let op = ssa::Opcode::Jump;
        let instruction = ssa::Instruction::Jump { op, target, args: vec![] };
        self.function.emit_instruction(self.current_block, instruction, location);

        self.builder.insert_edge(self.current_block, target);
    }

    fn emit_branch(&mut self, expr: ssa::Value, true_block: ssa::Label, false_block: ssa::Label, location: usize) {
        let op = ssa::Opcode::Branch;
        let instruction = ssa::Instruction::Branch {
            op,
            targets: [true_block, false_block],
            arg_lens: [0, 0],
            args: vec![expr],
        };
        self.function.emit_instruction(self.current_block, instruction, location);

        self.builder.insert_edge(self.current_block, true_block);
        self.builder.insert_edge(self.current_block, false_block);
    }
}

/// Debug location of the "last" location in an action.
// TODO: track blocks' actual closing delimiter position?
fn action_end_loc(&(ref action, span): &(ast::Action, Span)) -> usize {
    match *action {
        ast::Action::Block { .. } => span.high - 1,
        _ => span.low,
    }
}

/// Debug location of the "last" location in a statement.
// TODO: track blocks' actual closing delimiter position?
fn end_loc(&(ref stmt, span): &(ast::Stmt, Span)) -> usize {
    match *stmt {
        ast::Stmt::Block(_) | ast::Stmt::Switch(_, _) => span.high - 1,
        _ => span.low,
    }
}

fn loc(&(_, span): &(ast::Expr, Span)) -> usize {
    span.low
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
