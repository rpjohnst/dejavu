#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
#[macro_use]
extern crate wasm_host;

use std::collections::HashMap;
use wasm_bindgen::prelude::*;
use js_sys::Function;

use gml::ErrorPrinter;
use gml::front::{Span, Lines};
use gml::symbol::Symbol;
use engine::Engine;

#[wasm_bindgen]
pub fn setup(out: Function, err: Function) {
    wasm_host::redirect_print(out, err);
}

#[wasm_bindgen]
pub fn run(source: &str) {
    let mut items = HashMap::new();
    Engine::register(&mut items);

    let script = Symbol::intern(b"script");
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
    let id = engine.instance.instance_create(&mut engine.world, &mut engine.motion, 0.0, 0.0, 0)
        .unwrap_or_else(|_| panic!("object does not exist"));

    let mut thread = gml::vm::Thread::default();
    thread.set_self(engine.world.instances[id]);
    if let Err(error) = thread.execute(&mut engine, &resources, script, vec![]) {
        let location = resources.debug[&error.symbol].get_location(error.instruction as u32);
        let lines = match items[&error.symbol] {
            gml::Item::Event(source) => Lines::from_actions(source),
            gml::Item::Script(source) => Lines::from_code(source),
            _ => Lines::from_code(b""),
        };
        let mut errors = ErrorPrinter::new(error.symbol, lines);
        let span = Span { low: location as usize, high: location as usize };
        errors.error(span, format_args!("{}", error.kind));
    }
}
