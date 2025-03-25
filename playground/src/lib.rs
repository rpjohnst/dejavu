#![cfg(target_arch = "wasm32")]

use std::{io, ptr, slice};
use std::io::Write;
use runner::World;
use wasm::{JsValue, Reflect, Layout};
use bstr::BStr;

#[unsafe(no_mangle)]
pub extern "C" fn read_project<'a>(
    game: &mut project::Game<'a>, arena: &'a quickdry::Arena, data_ptr: *const u8, data_len: usize
) {
    let data = unsafe { slice::from_raw_parts(data_ptr, data_len) };

    // Allow the host to use an empty buffer to represent a default game.
    if data.is_empty() {
        let circle_spr = game.sprites.len() as i32;
        game.sprites.push(project::Sprite {
            name: BStr::new(b"circle_spr"),
            version: 800,
            origin: (16, 16),
            images: vec![
                project::Image { size: (32, 32), data: &SPRITE },
            ],
            ..project::Sprite::default()
        });

        let playground_obj = game.objects.len() as i32;
        game.objects.push(project::Object {
            name: BStr::new(b"playground_obj"),
            sprite: circle_spr,
            visible: true,
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
                            arguments: vec![BStr::new(b"")],
                            ..project::Action::default()
                        },
                    ],
                },
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
                            arguments: vec![BStr::new(b"direction += 2

if x < 0 {
    hspeed = 6
} else if x > room_width {
    hspeed = -6
}

if y < 0 {
    vspeed = 6
} else if y > room_height {
    vspeed = -6
}")],
                            ..project::Action::default()
                        },
                    ],
                },
            ],
            ..project::Object::default()
        });

        let playground_rm = game.rooms.len() as u32;
        game.last_instance += 1;
        let id1 = game.last_instance;
        game.last_instance += 1;
        let id2 = game.last_instance;
        game.last_instance += 1;
        let id3 = game.last_instance;
        game.last_instance += 1;
        let id4 = game.last_instance;
        game.rooms.push(project::Room {
            name: BStr::new(b"playground_rm"),
            width: 640,
            height: 480,
            speed: 60,
            code: BStr::new(b""),
            instances: vec![
                project::Instance {
                    x: 213, y: 160, object_index: playground_obj, id: id1,
                    code: BStr::new(b"hspeed = 6; vspeed = 6;")
                },
                project::Instance {
                    x: 427, y: 160, object_index: playground_obj, id: id2,
                    code: BStr::new(b"hspeed = -6; vspeed = 6;")
                },
                project::Instance {
                    x: 213, y: 320, object_index: playground_obj, id: id3,
                    code: BStr::new(b"hspeed = 6; vspeed = -6;")
                },
                project::Instance {
                    x: 427, y: 320, object_index: playground_obj, id: id4,
                    code: BStr::new(b"hspeed = -6; vspeed = -6;")
                },
            ],
            ..project::Room::default()
        });
        game.room_order.push(playground_rm);

        return;
    }

    match project::read_project(data, game, arena) {
        Ok(_) => {}
        Err(err) => {
            let _ = write!(HostErr(), "{err:?}");
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn run<'a>(
    game: &mut project::Game<'_>, arena: &quickdry::Arena, canvas: JsValue
) -> *mut runner::State {
    std::panic::set_hook(Box::new(|info| { let _ = writeln!(HostErr(), "{info}"); }));
    unsafe { clear() };

    let (mut assets, debug) = match runner::build(game, &[], &arena, HostErr) {
        Ok(assets) => assets,
        Err(errors) => {
            if errors > 1 {
                let _ = write!(HostErr(), "aborting due to {} previous errors", errors);
            } else {
                let _ = write!(HostErr(), "aborting due to previous error");
            }
            return ptr::null_mut();
        }
    };
    let _ = runner::load(&mut assets, &[]);
    let mut world = World::from_assets(&assets, debug);
    world.draw.platform.canvas = canvas;
    world.debug.error = |state, error| state.show_vm_error_write(error, HostErr());
    world.debug.write = Box::new(HostOut);

    runner::run(runner::Context { world, assets })
}

#[unsafe(no_mangle)]
pub extern "C" fn end(state: *mut runner::State) {
    if !state.is_null() {
        unsafe { runner::end(state) };
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn with_arena(f: JsValue) {
    let arena = quickdry::Arena::default();
    unsafe { call_ptr(f, &raw const arena as _) };
}

#[unsafe(no_mangle)]
pub extern "C" fn arena_alloc(arena: &quickdry::Arena, size: usize, align: usize) -> *mut u8 {
    use core::alloc::Layout;
    unsafe { arena.alloc(Layout::from_size_align_unchecked(size, align)) }
}

#[unsafe(no_mangle)]
pub static GAME_LAYOUT: Layout = project::Game::LAYOUT;

#[unsafe(no_mangle)]
pub extern "C" fn with_game(f: JsValue) {
    let mut game = project::Game::default();
    unsafe { call_ptr(f, &raw mut game as _) };
}

#[allow(improper_ctypes)]
unsafe extern "C" {
    fn call_ptr(f: JsValue, ptr: *const ());
}

static SPRITE: [u8; 32 * 32 * 4] = const {
    let mut data = [0; 32 * 32 * 4];
    circle(&mut data, 32, 15, 15, 15, [0xED, 0xED, 0xED, 0xFF]);
    circle(&mut data, 32, 15, 15, 13, [0, 0, 0xCE, 0xFF]);
    data
};

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

struct HostOut;
struct HostErr();

impl io::Write for HostOut {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        unsafe {
            let ptr = buf.as_ptr();
            let len = buf.len();
            out_print(ptr, len);
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> { Ok(()) }
}

impl io::Write for HostErr {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        unsafe {
            let ptr = buf.as_ptr();
            let len = buf.len();
            err_print(ptr, len);
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> { Ok(()) }
}

unsafe extern "C" {
    fn clear();
    fn out_print(string_ptr: *const u8, string_len: usize);
    fn err_print(string_ptr: *const u8, string_len: usize);
}
