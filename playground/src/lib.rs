#![cfg(target_arch = "wasm32")]

use std::{io, ptr, slice};
use std::io::Write;
use runner::World;
use wasm::JsValue;

#[unsafe(no_mangle)]
pub extern "system" fn with_game(f: JsValue) {
    let arena = quickdry::Arena::default();
    let mut game = project::Game::default();
    unsafe { with_game_call(f, &arena, &mut game) };
}

unsafe extern "system" {
    // `Arena` and `Game` are not FFI-safe, but they are Sized, so references to them are FFI-safe.
    // These references are opaque on the host side, and passed back in to APIs exported from here.
    #[allow(improper_ctypes)]
    fn with_game_call<'a>(load: JsValue, arena: &'a quickdry::Arena, game: &mut project::Game<'a>);
}

#[unsafe(no_mangle)]
pub extern "system" fn game_run<'a>(
    game: &mut project::Game<'_>, arena: &quickdry::Arena, canvas: JsValue
) -> *mut runner::State {
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
pub extern "system" fn end(state: *mut runner::State) {
    if !state.is_null() {
        unsafe { runner::end(state) };
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn arena_alloc(arena: &quickdry::Arena, size: usize, align: usize) -> *mut u8 {
    use std::alloc::Layout;
    unsafe { arena.alloc(Layout::from_size_align_unchecked(size, align)) }
}

#[unsafe(no_mangle)]
pub extern "system" fn game_read_project<'a>(
    game: &mut project::Game<'a>,
    arena: &'a quickdry::Arena,
    data_ptr: *const u8, data_len: usize
) {
    let data = unsafe { slice::from_raw_parts(data_ptr, data_len) };

    // Allow the host to use an empty buffer to represent a default game.
    if data.is_empty() {
        let circle_spr = game.sprites.len() as i32;
        game.sprites.push(project::Sprite {
            name: b"circle_spr",
            version: 800,
            origin: (16, 16),
            images: vec![
                project::Image { size: (32, 32), data: &SPRITE },
            ],
            ..project::Sprite::default()
        });

        let playground_obj = game.objects.len() as i32;
        game.objects.push(project::Object {
            name: b"playground_obj",
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
                            arguments: vec![b""],
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
                            arguments: vec![b"direction += 2

if x < 0 {
    hspeed = 6
} else if x > room_width {
    hspeed = -6
}

if y < 0 {
    vspeed = 6
} else if y > room_height {
    vspeed = -6
}"],
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
            name: b"playground_rm",
            width: 640,
            height: 480,
            speed: 60,
            code: b"",
            instances: vec![
                project::Instance {
                    x: 213, y: 160, object_index: playground_obj, id: id1,
                    code: b"hspeed = 6; vspeed = 6;"
                },
                project::Instance {
                    x: 427, y: 160, object_index: playground_obj, id: id2,
                    code: b"hspeed = -6; vspeed = 6;"
                },
                project::Instance {
                    x: 213, y: 320, object_index: playground_obj, id: id3,
                    code: b"hspeed = 6; vspeed = -6;"
                },
                project::Instance {
                    x: 427, y: 320, object_index: playground_obj, id: id4,
                    code: b"hspeed = -6; vspeed = -6;"
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
pub extern "system" fn game_visit(game: &mut project::Game<'_>, visitor: JsValue) {
    unsafe {
        for (sprite_index, sprite) in game.sprites[..].iter().enumerate() {
            let [ref image, ..] = sprite.images[..] else { continue };
            visit_sprite(
                visitor, sprite_index as u32, sprite.name.as_ptr(), sprite.name.len(),
                image.size.0, image.size.1, image.data.as_ptr(), image.data.len(),
            );
        }

        for (object_index, object) in game.objects[..].iter().enumerate() {
            visit_object(
                visitor, object_index as u32, object.name.as_ptr(), object.name.len(), object.sprite
            );
            for event in &object.events[..] {
                let [project::Action {
                    library: 1,
                    action: 603,
                    action_kind: project::action_kind::CODE,
                    has_relative: false,
                    is_question: false,
                    has_target: true,
                    action_type: project::action_type::NONE,
                    name: &[],
                    code: &[],
                    parameters_used: 1,
                    ref parameters,
                    target: gml::vm::SELF,
                    relative: false,
                    ref arguments,
                    negate: false,
                }] = event.actions[..] else { continue };
                let [project::argument_type::STRING] = parameters[..] else { continue };
                let [code] = arguments[..] else { continue };
                visit_object_event(
                    visitor, object_index as u32, event.event_type, event.event_kind,
                    code.as_ptr(), code.len(),
                );
            }
        }

        for (room_index, room) in game.rooms[..].iter().enumerate() {
            visit_room(visitor, room_index as u32, room.name.as_ptr(), room.name.len());
        }
    }
}

unsafe extern "system" {
    fn visit_sprite(
        visitor: JsValue,
        sprite_index: u32, name_ptr: *const u8, name_len: usize,
        size_x: u32, size_y: u32, data_ptr: *const u8, data_len: usize,
    );
    fn visit_object(
        visitor: JsValue, object_index: u32, name_ptr: *const u8, name_len: usize, sprite: i32
    );
    fn visit_object_event(
        visitor: JsValue, object_index: u32, event_type: u32, event_kind: i32,
        code_ptr: *const u8, code_len: usize
    );
    fn visit_room(
        visitor: JsValue, room_index: u32, name_ptr: *const u8, name_len: usize,
    );
}

#[unsafe(no_mangle)]
pub extern "system" fn game_sprite<'a>(
    game: &mut project::Game<'a>, sprite_index: u32, name_ptr: *const u8, name_len: usize,
) {
    let sprite = &mut game.sprites[sprite_index as usize];
    sprite.name = unsafe { slice::from_raw_parts(name_ptr, name_len) };
}

#[unsafe(no_mangle)]
pub extern "system" fn game_object<'a>(
    game: &mut project::Game<'a>, object_index: u32, name_ptr: *const u8, name_len: usize,
) {
    let object = &mut game.objects[object_index as usize];
    object.name = unsafe { slice::from_raw_parts(name_ptr, name_len) };
}

#[unsafe(no_mangle)]
pub extern "system" fn game_object_event<'a>(
    game: &mut project::Game<'a>,
    object_index: u32, event_type: u32, event_kind: i32, code_ptr: *const u8, code_len: usize,
) {
    let object = &mut game.objects[object_index as usize];
    let event = object.events.iter_mut()
        .find(|event| (event.event_type, event.event_kind) == (event_type, event_kind))
        .unwrap();
    event.actions[0].arguments[0] = unsafe { slice::from_raw_parts(code_ptr, code_len) };
}

#[unsafe(no_mangle)]
pub extern "system" fn game_room<'a>(
    game: &mut project::Game<'a>, room_index: u32,name_ptr: *const u8, name_len: usize,
) {
    let room = &mut game.rooms[room_index as usize];
    room.name = unsafe { slice::from_raw_parts(name_ptr, name_len) };
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

unsafe extern "system" {
    fn clear();
    fn out_print(string_ptr: *const u8, string_len: usize);
    fn err_print(string_ptr: *const u8, string_len: usize);
}
