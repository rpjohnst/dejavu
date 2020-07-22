#![cfg(all(target_arch = "wasm32", target_os = "unknown"))]

use std::collections::HashMap;
use std::{str, io::{self, Write}};
use wasm_bindgen::prelude::*;

use gml::{Function, ErrorPrinter};
use gml::front::Span;
use engine::World;

struct HostOut;
struct HostErr();

#[wasm_bindgen]
pub fn run(source: &str) {
    let mut game = project::Game::default();
    let mut items = HashMap::default();
    World::register(&mut items);

    let script = Function::Script { id: game.scripts.len() as i32 };
    game.scripts.push(project::Script { name: b"script", body: source.as_bytes() });

    let (mut assets, debug) = match engine::build(&game, &items, HostErr) {
        Ok(assets) => assets,
        Err(errors) => {
            if errors > 1 {
                let _ = write!(HostErr(), "aborting due to {} previous errors", errors);
            } else {
                let _ = write!(HostErr(), "aborting due to previous error");
            }
            return;
        }
    };

    let mut world = World::default();
    world.show.set_write(Box::new(HostOut));
    let id = world.instance.instance_create(&mut world.world, &mut world.motion, 0.0, 0.0, 0)
        .unwrap_or_else(|_| { let _ = writeln!(HostErr(), "object does not exist"); panic!() });

    let mut thread = gml::vm::Thread::default();
    thread.set_self(world.world.instances[id]);
    if let Err(error) = thread.execute(&mut world, &mut assets, script, vec![]) {
        let mut errors = ErrorPrinter::from_debug(&debug, error.function, HostErr());
        let offset = error.instruction as u32;
        let location = debug.locations[&error.function].locations.get_location(offset);
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
