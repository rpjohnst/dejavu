#![feature(optin_builtin_traits)]
#![feature(box_patterns)]
#![feature(box_syntax)]
#![feature(extern_types)]

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
#[macro_use]
extern crate wasm_host;

use std::collections::HashMap;

use crate::symbol::Symbol;
use crate::front::{Lexer, Parser, ErrorHandler, ErrorPrinter};
use crate::back::ssa;
use crate::vm::code;

pub use gml_meta::bind;

#[macro_use]
mod handle_map;
mod index_map;
mod bit_vec;
pub mod symbol;

pub mod front;
pub mod back;
pub mod vm;

/// A GML item definition, used as input to build a project.
pub enum Item<'a, E> {
    Script(&'a [u8]),
    Native(vm::ApiFunction<E>, usize, bool),
    Member(Option<vm::GetFunction<E>>, Option<vm::SetFunction<E>>),
}

/// Build a GML project.
pub fn build<E>(items: &HashMap<Symbol, Item<E>>) -> Result<vm::Resources<E>, (u32, vm::Resources<E>)> {
    let prototypes: HashMap<Symbol, ssa::Prototype> = items.iter()
        .map(|(&name, resource)| match *resource {
            Item::Script(_) => (name, ssa::Prototype::Script),
            Item::Native(_, arity, variadic) => (name, ssa::Prototype::Native { arity, variadic }),
            Item::Member(_, _) => (name, ssa::Prototype::Member),
        })
        .collect();

    let mut resources = vm::Resources::default();
    let mut error_count = 0;
    for (&name, item) in items.iter() {
        match *item {
            Item::Script(source) => {
                let mut errors = ErrorPrinter::new(name, source);
                let (function, debug) = compile(&prototypes, source, &mut errors);
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

fn compile(
    prototypes: &HashMap<Symbol, ssa::Prototype>, source: &[u8],
    errors: &mut dyn ErrorHandler
) -> (code::Function, code::Debug) {
    let reader = Lexer::new(source);
    let mut parser = Parser::new(reader, errors);
    let program = parser.parse_program();
    let codegen = front::Codegen::new(prototypes, errors);
    let program = codegen.compile(&program);
    let codegen = back::Codegen::new();
    codegen.compile(&program)
}
