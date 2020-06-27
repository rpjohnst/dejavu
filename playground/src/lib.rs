#![cfg(all(target_arch = "wasm32", target_os = "unknown"))]

use std::collections::HashMap;
use std::{str, io::{self, Write}};
use wasm_bindgen::prelude::*;

use gml::{Function, ErrorPrinter};
use gml::front::Span;
use engine::Engine;

struct HostOut;
struct HostErr();

#[wasm_bindgen]
pub fn run(source: &str) {
    let mut game = project::Game::default();
    let mut items = HashMap::default();
    Engine::register(&mut items);

    let script = Function::Script(game.scripts.len() as i32);
    game.scripts.push(project::Script { name: b"script", body: source.as_bytes() });

    let resources = match gml::build(&game, &items, HostErr) {
        Ok(resources) => resources,
        Err((errors, _)) => {
            if errors > 1 {
                let _ = write!(HostErr(), "aborting due to {} previous errors", errors);
            } else {
                let _ = write!(HostErr(), "aborting due to previous error");
            }
            return;
        }
    };

    let mut engine = Engine::default();
    engine.show.set_write(Box::new(HostOut));
    let id = engine.instance.instance_create(&mut engine.world, &mut engine.motion, 0.0, 0.0, 0)
        .unwrap_or_else(|_| { let _ = writeln!(HostErr(), "object does not exist"); panic!() });

    let mut thread = gml::vm::Thread::default();
    thread.set_self(engine.world.instances[id]);
    if let Err(error) = thread.execute(&mut engine, &resources, script, vec![]) {
        let mut errors = ErrorPrinter::from_game(&game, error.function, HostErr());
        let location = resources.debug[&error.function].get_location(error.instruction as u32);
        let span = Span { low: location as usize, high: location as usize };
        ErrorPrinter::error(&mut errors, span, format_args!("{}", error.kind));
    }
}

impl io::Write for HostOut {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let text = String::from_utf8_lossy(buf);
        out_print(&text[..]);
        Ok(text.len())
    }

    fn flush(&mut self) -> io::Result<()> { Ok(()) }
}

impl io::Write for HostErr {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let text = String::from_utf8_lossy(buf);
        err_print(&text[..]);
        Ok(text.len())
    }

    fn flush(&mut self) -> io::Result<()> { Ok(()) }
}

#[wasm_bindgen(raw_module = "../src/print.js")]
extern "C" {
    fn out_print(string: &str);
    fn err_print(string: &str);
}
