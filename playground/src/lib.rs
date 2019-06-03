#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
#[macro_use]
extern crate wasm_host;

use std::{collections::HashMap, cell::Cell};
use wasm_bindgen::prelude::*;
use js_sys::Function;

use gml::{symbol::Symbol, front::{Span, ErrorHandler}};
use engine::{Engine, instance::Instance};

struct Logger<'a> {
    name: Symbol,
    lines: Vec<usize>,
    count: &'a Cell<u32>,
}

impl<'a> Logger<'a> {
    pub fn new(name: Symbol, source: &str, count: &'a Cell<u32>) -> Self {
        let lines = gml::front::compute_lines(source);
        Logger { name, lines, count }
    }
}

impl ErrorHandler for Logger<'_> {
    fn error(&mut self, span: Span, message: &str) {
        let (line, column) = gml::front::get_position(&self.lines, span.low);
        eprintln!("error: {}:{}:{}: {}", self.name, line, column, message);
        self.count.set(self.count.get() + 1);
    }
}

#[wasm_bindgen]
pub fn setup(out: Function, err: Function) {
    wasm_host::redirect_print(out, err);
}

#[wasm_bindgen]
pub fn run(source: &str) {
    let mut items = HashMap::new();
    Engine::register(&mut items);

    let script = Symbol::intern("script");
    items.insert(script, gml::Item::Script(source));

    let error_count = Cell::new(0);
    let resources = gml::build(&items, |symbol, source| Logger::new(symbol, source, &error_count));
    if error_count.get() > 1 {
        eprintln!("aborting due to {} previous errors", error_count.get());
        return;
    } else if error_count.get() > 0 {
        eprintln!("aborting due to previous error");
        return;
    }

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
        let source = match items[&error.symbol] {
            gml::Item::Script(source) => source,
            _ => "",
        };
        let lines = gml::front::compute_lines(source);
        let (line, column) = gml::front::get_position(&lines, location as usize);
        eprintln!("fatal error: {}:{}:{}: {}", error.symbol, line, column, error.kind);
    }
}
