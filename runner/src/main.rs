extern crate gml;

use std::collections::HashMap;

use gml::{symbol::Symbol, vm};

#[derive(Default)]
struct Engine;

impl Engine {
    fn show_debug_message(&mut self, arguments: &[vm::Value]) -> Result<vm::Value, vm::Error> {
        eprintln!("{:?}", arguments[0]);
        Ok(vm::Value::from(0))
    }
}

fn main() {
    let mut items = HashMap::new();

    let show_debug_message = Symbol::intern("show_debug_message");
    items.insert(show_debug_message, gml::Item::Native(Engine::show_debug_message, 1, false));

    let main = Symbol::intern("main");
    items.insert(main, gml::Item::Script(r#"{
        show_debug_message("hello world")
    }"#));

    let resources = gml::build(items);
    let mut engine = Engine::default();
    let mut state = gml::vm::State::new();

    let _ = state.execute(&resources, &mut engine, main, &[]);
}
