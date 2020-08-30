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
    game.scripts.push(project::Script { name: b"<playground>", body: source.as_bytes() });

    let object = game.objects.len() as i32;
    game.objects.push(project::Object {
        name: b"playground_obj",
        persistent: false,
        events: vec![],
    });

    let (assets, debug) = match engine::build(&game, &items, HostErr) {
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

    let mut thread = gml::vm::Thread::default();
    let mut cx = engine::Context { world, assets };
    let id = engine::instance::State::instance_create(&mut cx, &mut thread, 0.0, 0.0, object)
        .unwrap_or_else(|_| { let _ = writeln!(HostErr(), "object does not exist"); panic!() });

    let engine::Context { world, .. } = &mut cx;
    let entity = world.world.instances[id];

    if let Err(error) = thread.with(entity).execute(&mut cx, script, vec![]) {
        let (mut errors, span, stack) = match error.frames[..] {
            [ref frame, ref stack @ ..] => {
                let errors = ErrorPrinter::from_debug(&debug, frame.function, HostErr());
                let span = Span::from_debug(&debug, frame);
                (errors, span, stack)
            },
            _ => return,
        };

        ErrorPrinter::error(&mut errors, span, format_args!("{}", error.kind));
        ErrorPrinter::stack_from_debug(&mut errors, &debug, stack);
    };
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
