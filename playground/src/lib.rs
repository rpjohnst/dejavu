#![cfg(target_arch = "wasm32")]
#![feature(const_mut_refs)]

use std::{ptr, str, io};
use std::io::Write;
use std::sync::atomic::{AtomicPtr, Ordering};
use wasm_bindgen::prelude::*;
use runner::World;

struct HostOut;
struct HostErr();

static STATE: AtomicPtr<runner::State> = AtomicPtr::new(ptr::null_mut());

#[wasm_bindgen]
pub fn run(source: &str) {
    // Cancel the game loop and free old state.
    stop();
    clear();
    let state = STATE.swap(ptr::null_mut(), Ordering::Relaxed);
    if !state.is_null() {
        // Safety: STATE was set to a leaked box if non-null.
        unsafe { Box::from_raw(state) };
    }

    let mut game = project::Game::default();

    let circle_spr = game.sprites.len() as i32;
    game.sprites.push(project::Sprite {
        name: b"circle_spr",
        origin: (0, 0),
        frames: vec![
            project::Frame {
                size: (32, 32),
                data: &SPRITE,
            },
        ],
        masks: vec![],
    });

    let playground_obj = game.objects.len() as i32;
    game.objects.push(project::Object {
        name: b"playground_obj",
        sprite: circle_spr,
        events: vec![
            project::Event {
                event_type: project::event_type::STEP,
                event_kind: project::event_kind::STEP,
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
    let id1 = game.last_instance;
    game.last_instance += 1;
    let id2 = game.last_instance;

    game.rooms.push(project::Room {
        name: b"playground_rm",
        code: b"",
        instances: vec![
            project::Instance {
                x: 85, y: 128, object_index: playground_obj, id: id1,
                code: b"hspeed = 3; vspeed = -3;"
            },
            project::Instance {
                x: 171, y: 128, object_index: playground_obj, id: id2,
                code: b"hspeed = -3; vspeed = 3;"
            },
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
    world.draw.platform.canvas = canvas.clone();
    world.show.error = |state, error| state.show_vm_error_write(error, HostErr());
    world.show.write = Box::new(HostOut);

    let state = Box::into_raw(Box::new(runner::run(runner::Context { world, assets })));
    STATE.store(state, Ordering::Relaxed);
}

#[wasm_bindgen]
pub fn stop() { runner::stop() }

static SPRITE: [u8; 32 * 32 * 4] = sprite();

const fn sprite() -> [u8; 32 * 32 * 4] {
    let mut data = [0; 32 * 32 * 4];
    circle(&mut data, 32, 15, 15, 15, [0xED, 0xED, 0xED, 0xFF]);
    circle(&mut data, 32, 15, 15, 13, [0xCE, 0, 0, 0xFF]);
    data
}

const fn circle(data: &mut [u8], width: i32, cx: i32, cy: i32, r: i32, c: [u8; 4]) {
    let mut x = r;
    let mut y = 0;
    let mut error = 1 - x;
    while x >= y {
        let left = cx - x;
        let right = cx + x;
        line(data, width, left, right, cy - y, c);
        line(data, width, left, right, cy + y, c);

        y += 1;
        if error < 0 {
            error += 2 * y + 1;
        } else {
            if x >= y {
                let left = cy - y + 1;
                let right = cy + y - 1;
                line(data, width, left, right, cy - x, c);
                line(data, width, left, right, cy + x, c);
            }
            x -= 1;
            error += 2 * (y - x + 1);
        }
    }
}

const fn line(data: &mut [u8], width: i32, left: i32, right: i32, y: i32, c: [u8; 4]) {
    let mut x = left;
    while x <= right {
        data[((y * width + x) * 4 + 0) as usize] = c[0];
        data[((y * width + x) * 4 + 1) as usize] = c[1];
        data[((y * width + x) * 4 + 2) as usize] = c[2];
        data[((y * width + x) * 4 + 3) as usize] = c[3];
        x += 1;
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

#[wasm_bindgen(module = "/src/page.js")]
extern "C" {
    static canvas: JsValue;

    fn clear();
    fn out_print(string: &str);
    fn err_print(string: &str);
}
