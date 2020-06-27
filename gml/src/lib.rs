#![feature(maybe_uninit_extra)]
#![feature(box_patterns)]
#![feature(box_syntax)]
#![feature(extern_types)]
#![feature(untagged_unions)]

use std::collections::HashMap;
use std::{fmt, io};

use crate::symbol::Symbol;
use crate::front::{Lexer, Parser, ActionParser, Lines, Position, Span};
use crate::back::ssa;

pub use gml_meta::bind;

#[macro_use]
mod handle_map;
mod rc_vec;
mod bit_vec;
pub mod symbol;

pub mod front;
pub mod back;
pub mod vm;

/// The name of a single executable unit of GML or D&D actions.
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub enum Function {
    Event(Event),
    Script(i32),
}

/// The name of an event.
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct Event {
    pub object_index: i32,
    pub event_type: u32,
    pub event_kind: i32,
}

/// An entity defined by the engine.
pub enum Item<E> {
    Native(vm::ApiFunction<E>, usize, bool),
    Member(Option<vm::GetFunction<E>>, Option<vm::SetFunction<E>>),
}

/// Build a Game Maker project.
pub fn build<E, F, W>(game: &project::Game, engine: &HashMap<Symbol, Item<E>>, mut write: F) ->
    Result<vm::Resources<E>, (u32, vm::Resources<E>)>
where
    F: FnMut() -> W,
    W: io::Write + 'static,
{
    // Collect the prototypes of entities that may be referred to in code.
    let scripts = game.scripts.iter()
        .enumerate()
        .map(|(id, &project::Script { ref name, .. })| {
            let id = id as i32;
            (Symbol::intern(name), ssa::Prototype::Script { id })
        });
    let builtins = engine.iter()
        .map(|(&name, item)| match *item {
            Item::Native(_, arity, variadic) => (name, ssa::Prototype::Native { arity, variadic }),
            Item::Member(_, _) => (name, ssa::Prototype::Member),
        });
    let prototypes: HashMap<Symbol, ssa::Prototype> = Iterator::chain(scripts, builtins).collect();

    let mut resources = vm::Resources::default();
    let mut error_count = 0;

    // Insert engine entities.
    for (&name, item) in engine.iter() {
        match *item {
            Item::Native(api, _, _) => { resources.api.insert(name, api); }
            Item::Member(get, set) => {
                if let Some(get) = get { resources.get.insert(name, get); }
                if let Some(set) = set { resources.set.insert(name, set); }
            }
        }
    }

    // Compile scripts.
    for (id, &project::Script { body, .. }) in game.scripts.iter().enumerate() {
        let function = Function::Script(id as i32);
        let mut errors = ErrorPrinter::new(function, Lines::from_code(body), write());
        let program = Parser::new(Lexer::new(body, 0), &mut errors).parse_program();
        let program = front::Codegen::new(&prototypes, &mut errors).compile_program(&program);
        let (code, debug) = back::Codegen::new(&prototypes).compile(&program);
        error_count += errors.count;

        resources.scripts.insert(id as i32, code);
        resources.debug.insert(function, debug);
    }

    // Compile object events.
    let events = game.objects.iter()
        .enumerate()
        .flat_map(|(object_index, &project::Object { ref events, .. })| {
            let object_index = object_index as i32;
            events.iter().map(move |&project::Event { event_type, event_kind, ref actions }| {
                (Event { object_index, event_type, event_kind }, &actions[..])
            })
        });
    for (event, actions) in events {
        let function = Function::Event(event);
        let mut errors = ErrorPrinter::new(function, Lines::from_actions(actions), write());
        let program = ActionParser::new(actions.iter(), &mut errors).parse_event();
        let program = front::Codegen::new(&prototypes, &mut errors).compile_event(&program);
        let (code, debug) = back::Codegen::new(&prototypes).compile(&program);
        error_count += errors.count;

        resources.events.insert(event, code);
        resources.debug.insert(function, debug);
    }

    if error_count > 0 {
        return Err((error_count, resources));
    }

    Ok(resources)
}

impl fmt::Display for Function {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Function::Event(event) => event.fmt(f),
            Function::Script(script) => script.fmt(f),
        }
    }
}

impl fmt::Display for Event {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Event {}({}) for object {}", self.event_type, self.event_kind, self.object_index)?;
        Ok(())
    }
}

pub struct ErrorPrinter<W: ?Sized = dyn io::Write> {
    pub name: Function,
    pub lines: Lines,
    pub count: u32,
    pub write: W,
}

impl ErrorPrinter {
    pub fn new<W: io::Write>(name: Function, lines: Lines, write: W) -> ErrorPrinter<W> {
        ErrorPrinter { name, lines, count: 0, write }
    }

    pub fn from_game<W: io::Write>(game: &project::Game, function: Function, write: W) ->
        ErrorPrinter<W>
    {
        let lines = match function {
            Function::Script(script) => Lines::from_code(game.scripts[script as usize].body),
            Function::Event(Event { object_index, event_type, event_kind }) => {
                let event = game.objects[object_index as usize].events.iter()
                    .find(|&e| (e.event_type, e.event_kind) == (event_type, event_kind))
                    .unwrap();
                Lines::from_actions(&event.actions[..])
            }
        };
        ErrorPrinter::new(function, lines, write)
    }

    pub fn error(&mut self, span: Span, message: fmt::Arguments<'_>) {
        let Position { action, argument, line, column } = self.lines.get_position(span.low);
        let _ = write!(self.write, "error in {}", self.name);
        if let Some(action) = action {
            let _ = write!(self.write, ", action {}", action);
        }
        if let (Some(argument), None) = (argument, line) {
            let _ = write!(self.write, ", argument {}", argument);
        }
        if let Some(line) = line {
            let _ = write!(self.write, ":{}", line);
        }
        if let Some(column) = column {
            let _ = write!(self.write, ":{}", column);
        }
        let _ = writeln!(self.write, ": {}", message);
        self.count += 1;
    }
}
