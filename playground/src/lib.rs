#![cfg(all(target_arch = "wasm32", target_os = "unknown"))]

use std::{str, io};
use std::io::Write;
use wasm_bindgen::prelude::*;

use runner::World;

struct HostOut;
struct HostErr();

#[wasm_bindgen]
pub fn run(source: &str) {
    let mut game = project::Game::default();

    let playground_obj = game.objects.len() as i32;
    game.objects.push(project::Object {
        name: b"playground_obj",
        events: vec![
            project::Event {
                event_type: project::event_type::CREATE,
                event_kind: 0,
                actions: vec![
                    project::Action {
                        library: 1,
                        action: 603,
                        action_kind: project::action_kind::CODE,
                        has_target: true,
                        parameters_used: 1,
                        parameters: vec![project::argument_type::STRING],
                        target: gml::vm::SELF,
                        arguments: vec![source.as_bytes()],
                        ..project::Action::default()
                    },
                ],
            },
        ],
        ..project::Object::default()
    });

    let _playground_rm = game.rooms.len() as i32;

    game.last_instance += 1;
    let id = game.last_instance;

    game.rooms.push(project::Room {
        name: b"playground_rm",
        code: b"",
        instances: vec![
            project::Instance { x: 0, y: 0, object_index: playground_obj, id, code: b"" },
        ],
        ..project::Room::default()
    });

    let (assets, debug) = match runner::build(&game, HostErr) {
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
    let mut world = World::from_assets(&assets, debug);
    world.show.error = |state, error| state.show_vm_error_write(error, HostOut);
    world.show.write = Box::new(HostOut);
    runner::run(&mut runner::Context { world, assets });
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
