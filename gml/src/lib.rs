#![feature(optin_builtin_traits)]
#![feature(box_patterns)]
#![feature(box_syntax)]
#![feature(slice_patterns)]
#![feature(range_contains)]
#![feature(extern_types)]
#![feature(try_from)]

use std::path::PathBuf;
use std::collections::HashMap;

use symbol::Symbol;
use front::{Lexer, Parser, SourceFile, ErrorHandler};
use back::ssa;
use vm::code;

#[macro_use]
mod handle_map;
mod index_map;
mod bit_vec;
pub mod symbol;

pub mod front;
pub mod back;
pub mod vm;

/// A GML item definition, used as input to build a project.
pub enum Item<E> {
    Script(&'static str),
    Native(vm::ApiFunction<E>),
    Member(vm::GetFunction<E>, vm::SetFunction<E>),
}

/// Build a GML project.
pub fn build<E: Default>(items: HashMap<Symbol, Item<E>>) -> vm::Resources<E> {
    let prototypes: HashMap<Symbol, ssa::Prototype> = items.iter()
        .map(|(&name, resource)| match *resource {
            Item::Script(_) => (name, ssa::Prototype::Script),
            Item::Native(_) => (name, ssa::Prototype::Native),
            Item::Member(_, _) => (name, ssa::Prototype::Member),
        })
        .collect();

    let mut resources = vm::Resources::default();
    for (name, item) in items.into_iter() {
        match item {
            Item::Script(source) => {
                resources.scripts.insert(name, compile(&prototypes, name, source));
            }
            Item::Native(function) => {
                resources.api.insert(name, function);
            }
            Item::Member(get, set) => {
                resources.get.insert(name, get);
                resources.set.insert(name, set);
            }
        }
    }

    resources
}

fn compile(
    prototypes: &HashMap<Symbol, ssa::Prototype>, name: Symbol, source: &str
) -> code::Function {
    let source = SourceFile {
        name: PathBuf::from(&*name),
        source: String::from(source),
    };
    let errors = ErrorHandler;
    let reader = Lexer::new(&source);
    let mut parser = Parser::new(reader, &errors);
    let program = parser.parse_program();
    let codegen = front::Codegen::new(prototypes, &errors);
    let program = codegen.compile(&program);
    let codegen = back::Codegen::new();
    codegen.compile(&program)
}
