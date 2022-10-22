#![cfg(target_arch = "wasm32")]
#![feature(const_mut_refs)]
#![feature(let_else)]

use std::{io, ptr, slice};
use std::io::Write;
use std::sync::atomic::{AtomicPtr, Ordering};
use runner::World;
use wasm::JsValue;

pub use runner::stop;

static STATE: AtomicPtr<runner::State> = AtomicPtr::new(ptr::null_mut());

#[no_mangle]
pub extern "system" fn run(load: JsValue) {
    // Cancel the game loop and free old state.
    unsafe {
        stop();
        clear();
    }
    let state = STATE.swap(ptr::null_mut(), Ordering::Relaxed);
    if !state.is_null() {
        // Safety: STATE was set to a leaked box if non-null.
        let _ = unsafe { Box::from_raw(state) };
    }

    let arena = quickdry::Arena::default();
    let mut game = project::Game::default();
    unsafe { load_call(load, &arena, &mut game) };

    let (assets, debug) = match runner::build(&game, &[], HostErr) {
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
    world.draw.platform.canvas = unsafe { canvas() };
    world.show.error = |state, error| state.show_vm_error_write(error, HostErr());
    world.show.write = Box::new(HostOut);

    let state = runner::run(runner::Context { world, assets });
    STATE.store(state, Ordering::Relaxed);
}

extern "system" {
    // `Arena` and `Game` are not FFI-safe, but they are Sized, so references to them are FFI-safe.
    // These references are opaque on the host side, and passed back in to APIs exported from here.
    #[allow(improper_ctypes)]
    fn load_call<'a>(load: JsValue, arena: &'a quickdry::Arena, game: &mut project::Game<'a>);

    fn canvas() -> JsValue;
}

#[no_mangle]
pub extern "system" fn arena_alloc(arena: &quickdry::Arena, size: usize, align: usize) -> *mut u8 {
    use std::alloc::Layout;
    unsafe { arena.alloc(Layout::from_size_align_unchecked(size, align)) }
}

#[no_mangle]
pub extern "system" fn game_sprite<'a>(
    game: &mut project::Game<'a>,
    name_ptr: *const u8, name_len: usize,
    origin_x: u32, origin_y: u32,
    size_x: u32, size_y: u32,
    data_ptr: *const u8, data_len: usize,
) {
    let name = unsafe { slice::from_raw_parts(name_ptr, name_len) };
    let origin = (origin_x, origin_y);
    let size = (size_x, size_y);
    let data = unsafe { slice::from_raw_parts(data_ptr, data_len) };
    game.sprites.push(project::Sprite {
        name,
        origin,
        images: vec![
            project::Image { size, data },
        ],
        masks: vec![],
        ..project::Sprite::default()
    });
}

#[no_mangle]
pub extern "system" fn game_object<'a>(
    game: &mut project::Game<'a>,
    name_ptr: *const u8, name_len: usize,
    sprite: i32,
    events_len: usize,
) {
    let name = unsafe { slice::from_raw_parts(name_ptr, name_len) };
    game.objects.push(project::Object {
        name,
        sprite,
        events: Vec::with_capacity(events_len),
        ..project::Object::default()
    });
}

#[no_mangle]
pub extern "system" fn game_object_event<'a>(
    game: &mut project::Game<'a>,
    event_type: u32, event_kind: i32,
    code_ptr: *const u8, code_len: usize,
) {
    let Some(object) = game.objects.last_mut() else { return };
    let code = unsafe { slice::from_raw_parts(code_ptr, code_len) };
    object.events.push(project::Event {
        event_type,
        event_kind,
        actions: vec![
            project::Action {
                library: 1,
                action: 603,
                action_kind: project::action_kind::CODE,
                has_target: true,
                parameters_used: 1,
                parameters: vec![project::argument_type::STRING],
                target: gml::vm::SELF,
                arguments: vec![code],
                ..project::Action::default()
            },
        ],
    });
}

#[no_mangle]
pub extern "system" fn game_room<'a>(
    game: &mut project::Game<'a>,
    name_ptr: *const u8, name_len: usize,
    object_index: i32,
) {
    let name = unsafe { slice::from_raw_parts(name_ptr, name_len) };

    game.last_instance += 1;
    let id1 = game.last_instance;
    game.last_instance += 1;
    let id2 = game.last_instance;

    game.room_order.push(game.rooms.len() as u32);
    game.rooms.push(project::Room {
        name,
        code: b"",
        instances: vec![
            project::Instance {
                x: 85, y: 128, object_index, id: id1,
                code: b"hspeed = 3; vspeed = -3;"
            },
            project::Instance {
                x: 171, y: 128, object_index, id: id2,
                code: b"hspeed = -3; vspeed = 3;"
            },
        ],
        ..project::Room::default()
    });
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

extern "system" {
    fn clear();
    fn out_print(string_ptr: *const u8, string_len: usize);
    fn err_print(string_ptr: *const u8, string_len: usize);
}
