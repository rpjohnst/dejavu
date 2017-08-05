use std::mem;

use symbol::{Symbol, keyword};
use front::{ast, Span, ErrorHandler};
use back::ssa;

pub struct Codegen<'e> {
    function: ssa::Function,
    errors: &'e ErrorHandler,

    current_block: ssa::Block,

    current_iter: Option<Box<[ssa::Value]>>,
    current_next: Option<ssa::Block>,
    current_exit: Option<ssa::Block>,

    current_switch: Option<ssa::Value>,
    current_expr: Option<ssa::Block>,
    current_default: Option<ssa::Block>,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
enum Lvalue {
    Local(Symbol),
    Field(ssa::Value, Symbol),
    Index(ssa::Value, Box<[ssa::Value]>),
}

const ONE: f64 = 1.0;
const ZERO: f64 = 0.0;
const SELF: f64 = -1.0;
const OTHER: f64 = -2.0;
const ALL: f64 = -3.0;
const NOONE: f64 = -4.0;
const GLOBAL: f64 = -5.0;
const LOCAL: f64 = -6.0;

impl<'e> Codegen<'e> {
    pub fn new(errors: &'e ErrorHandler) -> Codegen<'e> {
        let function = ssa::Function::new();
        let entry = function.entry();

        Codegen {
            function: function,
            errors: errors,

            current_block: entry,

            current_iter: None,
            current_next: None,
            current_exit: None,

            current_switch: None,
            current_expr: None,
            current_default: None,
        }
    }

    pub fn compile(mut self, program: &(ast::Stmt, Span)) -> ssa::Function {
        self.emit_statement(program);
        self.function
    }

    fn emit_statement(&mut self, statement: &(ast::Stmt, Span)) {
        let (ref statement, statement_span) = *statement;
        match *statement {
            ast::Stmt::Assign(op, box ref lvalue, box ref rvalue) => {
                let lvalue = match self.emit_lvalue(lvalue) {
                    Ok(lvalue) => lvalue,
                    Err(()) => return,
                };

                let rvalue = if let Some(op) = op {
                    let lvalue = lvalue.clone();
                    let left = self.emit_load(lvalue);
                    let right = self.emit_rvalue(rvalue);
                    self.emit_instruction(
                        ssa::Instruction::Binary(ast::Binary::Op(op).into(), left, right)
                    )
                } else {
                    self.emit_rvalue(rvalue)
                };

                self.emit_store(lvalue, rvalue);
            }

            ast::Stmt::Invoke(ref call) => {
                self.emit_call(call);
            }

            ast::Stmt::Declare(scope, box ref names) => {
                let scope = match scope {
                    ast::Declare::Local => LOCAL,
                    ast::Declare::Global => GLOBAL,
                };

                for &(name, _) in names {
                    self.emit_instruction(ssa::Instruction::Declare(scope, name));
                }
            }

            ast::Stmt::Block(box ref statements) => {
                for statement in statements {
                    self.emit_statement(statement);
                }
            }

            ast::Stmt::If(box ref expr, box ref true_branch, ref false_branch) => {
                let true_block = self.function.make_block();
                let false_block = self.function.make_block();
                let merge_block = if false_branch.is_some() {
                    self.function.make_block()
                } else {
                    false_block
                };

                let expr = self.emit_rvalue(expr);
                self.emit_branch(expr, true_block, &[], false_block, &[]);

                self.current_block = true_block;
                self.emit_statement(true_branch);
                self.emit_jump(merge_block, box []);

                if let Some(box ref false_branch) = *false_branch {
                    self.current_block = false_block;
                    self.emit_statement(false_branch);
                    self.emit_jump(merge_block, box []);
                }

                self.current_block = merge_block;
            }

            ast::Stmt::Repeat(box ref count, box ref body) => {
                let next_block = self.function.make_block();
                let body_block = self.function.make_block();
                let exit_block = self.function.make_block();

                let count = self.emit_rvalue(count);
                self.emit_jump(next_block, box [count]);

                self.current_block = next_block;
                let count = self.emit_argument();
                let one = self.emit_real(ONE);
                let next = self.emit_instruction(
                    ssa::Instruction::Binary(ssa::Binary::Subtract, count, one)
                );
                self.emit_branch(count, body_block, &[], exit_block, &[]);

                self.current_block = body_block;
                self.with_loop(&[next], next_block, exit_block, |codegen| {
                    codegen.emit_statement(body);
                });
                self.emit_jump(next_block, box [next]);

                self.current_block = exit_block;
            }

            ast::Stmt::While(box ref expr, box ref body) => {
                let next_block = self.function.make_block();
                let body_block = self.function.make_block();
                let exit_block = self.function.make_block();

                self.emit_jump(next_block, box []);

                self.current_block = next_block;
                let expr = self.emit_rvalue(expr);
                self.emit_branch(expr, body_block, &[], exit_block, &[]);

                self.current_block = body_block;
                self.with_loop(&[], next_block, exit_block, |codegen| {
                    codegen.emit_statement(body);
                });
                self.emit_jump(next_block, box []);

                self.current_block = exit_block;
            }

            ast::Stmt::Do(box ref body, box ref expr) => {
                let body_block = self.function.make_block();
                let next_block = self.function.make_block();
                let exit_block = self.function.make_block();

                self.emit_jump(body_block, box []);

                self.current_block = body_block;
                self.with_loop(&[], next_block, exit_block, |codegen| {
                    codegen.emit_statement(body);
                });
                self.emit_jump(next_block, box []);

                self.current_block = next_block;
                let expr = self.emit_rvalue(expr);
                self.emit_branch(expr, exit_block, &[], body_block, &[]);

                self.current_block = exit_block;
            }

            ast::Stmt::For(box ref init, box ref expr, box ref next, box ref body) => {
                let expr_block = self.function.make_block();
                let body_block = self.function.make_block();
                let next_block = self.function.make_block();
                let exit_block = self.function.make_block();

                self.emit_statement(init);
                self.emit_jump(expr_block, box []);

                self.current_block = expr_block;
                let expr = self.emit_rvalue(expr);
                self.emit_branch(expr, body_block, &[], exit_block, &[]);

                self.current_block = body_block;
                self.with_loop(&[], next_block, exit_block, |codegen| {
                    codegen.emit_statement(body);
                });
                self.emit_jump(next_block, box []);

                self.current_block = next_block;
                self.emit_statement(next);
                self.emit_jump(expr_block, box []);

                self.current_block = exit_block;
            }

            ast::Stmt::With(box ref expr, box ref body) => {
                let next_block = self.function.make_block();
                let body_block = self.function.make_block();
                let exit_block = self.function.make_block();

                let expr = self.emit_rvalue(expr);
                let with = self.emit_instruction(ssa::Instruction::With(expr));
                self.emit_jump(next_block, box [with]);

                self.current_block = next_block;
                let with = self.emit_argument();
                let next = self.emit_instruction(ssa::Instruction::Next(with));
                self.emit_branch(with, body_block, &[], exit_block, &[]);

                self.current_block = body_block;
                self.with_loop(&[next], next_block, exit_block, |codegen| {
                    codegen.emit_statement(body);
                });
                self.emit_jump(next_block, box [next]);

                self.current_block = exit_block;
            }

            ast::Stmt::Switch(box ref expr, box ref body) => {
                let expr_block = self.current_block;
                let dead_block = self.function.make_block();
                let default_block = self.function.make_block();
                let exit_block = self.function.make_block();

                let expr = self.emit_rvalue(expr);

                self.current_block = dead_block;
                self.with_switch(expr, expr_block, default_block, exit_block, |codegen| {
                    for statement in body {
                        codegen.emit_statement(statement);
                    }
                    codegen.emit_jump(exit_block, box []);

                    codegen.current_block = codegen.current_expr.expect("corrupt switch state");
                    codegen.emit_jump(default_block, box []);

                    codegen.current_block = default_block;
                    codegen.emit_jump(exit_block, box []);
                });

                self.current_block = exit_block;
            }

            ast::Stmt::Jump(ast::Jump::Break) if self.current_exit.is_some() => {
                let exit_block = self.current_exit.unwrap();

                self.emit_jump(exit_block, box []);
            }

            ast::Stmt::Jump(ast::Jump::Continue) if self.current_next.is_some() => {
                let next_block = self.current_next.unwrap();

                let iter = self.current_iter.as_ref().expect("corrupt loop state").clone();
                self.emit_jump(next_block, iter);
            }

            ast::Stmt::Jump(ast::Jump::Exit) => {
                self.emit_instruction(ssa::Instruction::Exit);
            }

            ast::Stmt::Jump(_) => {
                let zero = self.emit_real(ZERO);
                self.emit_instruction(ssa::Instruction::Return(zero));
            }

            ast::Stmt::Return(box ref expr) => {
                let expr = self.emit_rvalue(expr);
                self.emit_instruction(ssa::Instruction::Return(expr));
            }

            ast::Stmt::Case(Some(box ref expr)) if self.current_expr.is_some() => {
                let case_block = self.function.make_block();
                let expr_block = self.function.make_block();

                self.emit_jump(case_block, box []);

                self.current_block = self.current_expr.unwrap();
                self.current_expr = Some(expr_block);
                let switch = self.current_switch.expect("corrupt switch state");
                let expr = self.emit_rvalue(expr);
                let expr = self.emit_instruction(
                    ssa::Instruction::Binary(ssa::Binary::Eq, switch, expr)
                );
                self.emit_branch(expr, case_block, &[], expr_block, &[]);

                self.current_block = case_block;
            }

            ast::Stmt::Case(None) if self.current_default.is_some() => {
                let default_block = self.current_default.unwrap();

                self.emit_jump(default_block, box []);

                self.current_block = default_block;
            }

            ast::Stmt::Case(_) => {
                self.errors.error(statement_span, "case statement outside of switch");
            }

            ast::Stmt::Error(_) => {}
        }
    }

    fn emit_rvalue(&mut self, expression: &(ast::Expr, Span)) -> ssa::Value {
        let (ref expr, _expr_span) = *expression;
        match *expr {
            ast::Expr::Value(ast::Value::Real(real)) => self.emit_real(real),
            ast::Expr::Value(ast::Value::String(string)) => self.emit_string(string),

            ast::Expr::Value(ast::Value::Ident(keyword::True)) => self.emit_real(ONE),
            ast::Expr::Value(ast::Value::Ident(keyword::False)) => self.emit_real(ZERO),
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
                self.emit_instruction(ssa::Instruction::Unary(op, expr))
            }

            ast::Expr::Binary(op, box ref left, box ref right) => {
                let left = self.emit_rvalue(left);
                let right = self.emit_rvalue(right);
                self.emit_instruction(ssa::Instruction::Binary(op.into(), left, right))
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

    fn emit_lvalue(&mut self, expression: &(ast::Expr, Span)) -> Result<Lvalue, ()> {
        let (ref expression, expression_span) = *expression;
        match *expression {
            ast::Expr::Value(ast::Value::Ident(symbol)) if !symbol.is_keyword() => {
                Ok(Lvalue::Local(symbol))
            }

            ast::Expr::Field(box ref expr, (field, _field_span)) => {
                let scope = self.emit_rvalue(expr);
                Ok(Lvalue::Field(scope, field))
            }

            ast::Expr::Index(box ref expr, box ref indices) => {
                if indices.len() < 1 || 2 < indices.len() {
                    self.errors.error(expression_span, "invalid number of array indices");
                }

                let array = self.emit_rvalue(expr);
                let indices: Vec<_> = indices.iter()
                    .map(|index| self.emit_rvalue(index))
                    .collect();
                let indices = indices.into_boxed_slice();

                Ok(Lvalue::Index(array, indices))
            }

            _ => {
                self.errors.error(expression_span, "expected a variable");
                Err(())
            }
        }
    }

    fn emit_load(&mut self, lvalue: Lvalue) -> ssa::Value {
        let instruction = match lvalue {
            Lvalue::Local(symbol) => ssa::Instruction::LoadDynamic(symbol),
            Lvalue::Field(scope, field) => ssa::Instruction::LoadField(scope, field),
            Lvalue::Index(array, indices) => ssa::Instruction::LoadIndex(array, indices),
        };
        self.emit_instruction(instruction)
    }

    fn emit_store(&mut self, lvalue: Lvalue, value: ssa::Value) -> ssa::Value {
        let instruction = match lvalue {
            Lvalue::Local(symbol) => ssa::Instruction::StoreDynamic(symbol, value),
            Lvalue::Field(scope, field) => ssa::Instruction::StoreField(scope, field, value),
            Lvalue::Index(array, indices) => ssa::Instruction::StoreIndex(array, indices, value),
        };
        self.emit_instruction(instruction)
    }

    fn emit_jump(&mut self, block: ssa::Block, arguments: Box<[ssa::Value]>) {
        self.emit_instruction(ssa::Instruction::Jump(block, arguments));
    }

    fn emit_branch(
        &mut self, expr: ssa::Value,
        true_block: ssa::Block, true_arguments: &[ssa::Value],
        false_block: ssa::Block, false_arguments: &[ssa::Value],
    ) {
        let true_arguments = true_arguments.to_vec().into_boxed_slice();
        let false_arguments = false_arguments.to_vec().into_boxed_slice();
        self.emit_instruction(ssa::Instruction::Branch(
            expr, true_block, true_arguments, false_block, false_arguments,
        ));
    }

    fn emit_call(&mut self, call: &ast::Call) -> ssa::Value {
        let ast::Call(box ref function, box ref arguments) = *call;
        let function = self.emit_rvalue(function);
        let arguments: Vec<_> = arguments.iter()
            .map(|argument| self.emit_rvalue(argument))
            .collect();
        self.emit_instruction(ssa::Instruction::Call(function, arguments.into_boxed_slice()))
    }

    fn emit_real(&mut self, real: f64) -> ssa::Value {
        let constant = ssa::Constant::Real(real);
        let instruction = ssa::Instruction::Immediate(constant);
        self.function.emit_instruction(self.current_block, instruction)
    }

    fn emit_string(&mut self, string: Symbol) -> ssa::Value {
        let constant = ssa::Constant::String(string);
        let instruction = ssa::Instruction::Immediate(constant);
        self.function.emit_instruction(self.current_block, instruction)
    }

    fn emit_instruction(&mut self, instruction: ssa::Instruction) -> ssa::Value {
        self.function.emit_instruction(self.current_block, instruction)
    }

    fn emit_argument(&mut self) -> ssa::Value {
        self.function.emit_argument(self.current_block)
    }

    fn with_loop<F>(
        &mut self, iter: &[ssa::Value], next: ssa::Block, exit: ssa::Block, f: F
    ) where F: FnOnce(&mut Codegen) {
        let iter = iter.to_vec().into_boxed_slice();

        let old_iter = mem::replace(&mut self.current_iter, Some(iter));
        let old_next = mem::replace(&mut self.current_next, Some(next));
        let old_exit = mem::replace(&mut self.current_exit, Some(exit));

        f(self);

        self.current_iter = old_iter;
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