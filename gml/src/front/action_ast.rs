use crate::symbol::Symbol;
use crate::front::{ast, Span};

pub enum Action {
    Error,
    Normal {
        question: Option<Box<Question>>,
        execution: Exec,
        target: Option<i32>,
        relative: Option<bool>,
        arguments: Box<[Argument]>,
    },
    Block {
        body: Box<[(Action, Span)]>,
    },
    Exit,
    Repeat {
        count: Box<(ast::Expr, Span)>,
        body: Box<(Action, Span)>,
    },
    Variable {
        target: i32,
        relative: bool,
        variable: Box<(ast::Expr, Span)>,
        value: Box<(ast::Expr, Span)>,
    },
    Code {
        target: i32,
        code: Box<(ast::Stmt, Span)>,
    },
}

pub struct Question {
    pub negate: bool,
    pub true_action: (Action, Span),
    pub false_action: Option<(Action, Span)>,
}

pub enum Exec {
    Function(Symbol),
    Code(Box<(ast::Stmt, Span)>),
}

pub enum Argument {
    Error,
    Expr(Box<(ast::Expr, Span)>),
    String(Symbol),
    Bool(bool),
    Menu(i32),
    Sprite(i32),
    Sound(i32),
    Background(i32),
    Path(i32),
    Script(i32),
    Object(i32),
    Room(i32),
    Font(i32),
    Color(u32),
    Timeline(i32),
    FontString(Symbol),
}
