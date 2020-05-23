#![feature(maybe_uninit_extra)]
#![feature(box_patterns)]
#![feature(box_syntax)]
#![feature(extern_types)]
#![feature(untagged_unions)]

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
#[macro_use]
extern crate wasm_host;

use std::collections::HashMap;

use crate::symbol::Symbol;
use crate::front::{Lexer, Parser, ActionParser, ErrorHandler, Lines, ErrorPrinter};
use crate::back::ssa;
use crate::vm::code;

pub use gml_meta::bind;

#[macro_use]
mod handle_map;
mod rc_vec;
mod bit_vec;
pub mod symbol;

pub mod front;
pub mod back;
pub mod vm;

/// A GML item definition, used as input to build a project.
pub enum Item<'a, E> {
    Event(&'a [project::Action]),
    Script(&'a [u8]),
    Native(vm::ApiFunction<E>, usize, bool),
    Member(Option<vm::GetFunction<E>>, Option<vm::SetFunction<E>>),
}

/// Build a GML project.
pub fn build<E>(items: &HashMap<Symbol, Item<E>>) -> Result<vm::Resources<E>, (u32, vm::Resources<E>)> {
    let prototypes: HashMap<Symbol, ssa::Prototype> = items.iter()
        .filter_map(|(&name, resource)| match *resource {
            Item::Event(_) => None,
            Item::Script(_) => Some((name, ssa::Prototype::Script)),
            Item::Native(_, arity, variadic) => Some((name, ssa::Prototype::Native { arity, variadic })),
            Item::Member(_, _) => Some((name, ssa::Prototype::Member)),
        })
        .collect();

    let mut resources = vm::Resources::default();
    let mut error_count = 0;
    for (&name, item) in items.iter() {
        match *item {
            Item::Event(actions) => {
                let mut errors = ErrorPrinter::new(name, Lines::from_event(actions));
                let (function, debug) = compile_event(&prototypes, actions, &mut errors);
                error_count += errors.count;

                resources.scripts.insert(name, function);
                resources.debug.insert(name, debug);
            }
            Item::Script(source) => {
                let mut errors = ErrorPrinter::new(name, Lines::from_script(source));
                let (function, debug) = compile_script(&prototypes, source, &mut errors);
                error_count += errors.count;

                resources.scripts.insert(name, function);
                resources.debug.insert(name, debug);
            }
            Item::Native(function, _, _) => {
                resources.api.insert(name, function);
            }
            Item::Member(get, set) => {
                if let Some(get) = get { resources.get.insert(name, get); }
                if let Some(set) = set { resources.set.insert(name, set); }
            }
        }
    }
    if error_count > 0 {
        return Err((error_count, resources));
    }

    Ok(resources)
}

fn compile_event(
    prototypes: &HashMap<Symbol, ssa::Prototype>, source: &[project::Action],
    errors: &mut dyn ErrorHandler
) -> (code::Function, code::Debug) {
    let reader = source.iter();
    let mut parser = ActionParser::new(reader, errors);
    let program = parser.parse_event();
    let codegen = front::Codegen::new(prototypes, errors);
    let program = codegen.compile_event(&program);
    let codegen = back::Codegen::new();
    codegen.compile(&program)
}

fn compile_script(
    prototypes: &HashMap<Symbol, ssa::Prototype>, source: &[u8],
    errors: &mut dyn ErrorHandler
) -> (code::Function, code::Debug) {
    let reader = Lexer::new(source, 0);
    let mut parser = Parser::new(reader, errors);
    let program = parser.parse_program();
    let codegen = front::Codegen::new(prototypes, errors);
    let program = codegen.compile_program(&program);
    let codegen = back::Codegen::new();
    codegen.compile(&program)
}
