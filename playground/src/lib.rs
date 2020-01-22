#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
#[macro_use]
extern crate wasm_host;

use std::collections::HashMap;
use wasm_bindgen::prelude::*;
use js_sys::Function;

use gml::front::{Span, ErrorHandler, Lines, ErrorPrinter};
use gml::symbol::Symbol;
use engine::{Engine, instance::Instance};

#[wasm_bindgen]
pub fn setup(out: Function, err: Function) {
    wasm_host::redirect_print(out, err);
}

#[wasm_bindgen]
pub fn run(source: &str) {
    let mut items = HashMap::new();
    Engine::register(&mut items);

    let script = Symbol::intern("script");
    items.insert(script, gml::Item::Script(source.as_bytes()));

    let resources = match gml::build(&items) {
        Ok(resources) => resources,
        Err((errors, _)) => {
            if errors > 1 {
                eprintln!("aborting due to {} previous errors", errors);
            } else {
                eprintln!("aborting due to previous error");
            }
            return;
        }
    };

    let mut engine = Engine::default();
    let mut thread = gml::vm::Thread::new();

    let object_index = 0;
    let id = 100001;
    let persistent = false;
    let entity = engine.world.create_instance(object_index, id);
    engine.instance.create_instance(entity, Instance { object_index, id, persistent });
    thread.set_self(entity);

    if let Err(error) = thread.execute(&mut engine, &resources, script, &[]) {
        let location = resources.debug[&error.symbol].get_location(error.instruction as u32);
        let lines = match items[&error.symbol] {
            gml::Item::Event(source) => Lines::from_event(source),
            gml::Item::Script(source) => Lines::from_script(source),
            _ => Lines::from_script(b""),
        };
        let mut errors = ErrorPrinter::new(error.symbol, lines);
        let span = Span { low: location as usize, high: location as usize };
        errors.error(span, &format!("{}", error.kind));
    }
}
