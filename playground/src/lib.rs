#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
#[macro_use]
extern crate wasm_host;

use std::collections::HashMap;
use wasm_bindgen::prelude::*;

use gml::{Function, ErrorPrinter};
use gml::front::Span;
use engine::Engine;

#[wasm_bindgen]
pub fn setup(out: js_sys::Function, err: js_sys::Function) {
    wasm_host::redirect_print(out, err);
}

#[wasm_bindgen]
pub fn run(source: &str) {
    let mut game = project::Game::default();
    let mut items = HashMap::default();
    Engine::register(&mut items);

    let script = Function::Script(game.scripts.len() as i32);
    game.scripts.push(project::Script { name: b"script", body: source.as_bytes() });

    let resources = match gml::build(&game, &items) {
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
        let mut errors = ErrorPrinter::from_game(&game, error.function);
        let location = resources.debug[&error.function].get_location(error.instruction as u32);
        let span = Span { low: location as usize, high: location as usize };
        errors.error(span, format_args!("{}", error.kind));
    }
}
