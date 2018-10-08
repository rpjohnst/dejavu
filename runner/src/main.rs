#![feature(try_from)]

extern crate gml;

use std::collections::HashMap;
use std::convert::TryFrom;

use gml::{symbol::Symbol, vm};

#[derive(Default)]
struct Engine;

impl Engine {
    fn show_debug_message(&mut self, arguments: &[vm::Value]) -> Result<vm::Value, vm::Error> {
        let string = match *arguments {
            [string] => Symbol::try_from(string).unwrap_or(Symbol::intern("")),
            _ => {
                let symbol = Symbol::intern("show_debug_message");
                let kind = vm::ErrorKind::Arity(arguments.len());
                Err(vm::Error { symbol, instruction: 0, kind })?
            }
        };
        eprintln!("{}", string);
        Ok(vm::Value::from(0))
    }
}

fn main() {
    let mut items = HashMap::new();

    let show_debug_message = Symbol::intern("show_debug_message");
    items.insert(show_debug_message, gml::Item::Native(Engine::show_debug_message));

    let main = Symbol::intern("main");
    items.insert(main, gml::Item::Script(r#"{
        show_debug_message("hello world")
    }"#));

    let resources = gml::build(items);
    let mut engine = Engine::default();
    let mut state = gml::vm::State::new();

    let _ = state.execute(&resources, &mut engine, main, &[]);
}
