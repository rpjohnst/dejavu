use std::collections::HashMap;
use std::io;

use gml::vm;

pub use crate::world::*;

mod world;

pub struct Context {
    pub world: World,
    pub assets: Assets,
}

#[derive(Default)]
pub struct Assets {
    pub code: vm::Assets<Context>,
    pub objects: Vec<Object>,
    pub rooms: Vec<Room>,
    pub next_instance: i32,
}

pub struct Object {
    pub persistent: bool,
}

pub struct Room {
    pub instances: Vec<Instance>,
}

pub struct Instance {
    pub x: i32,
    pub y: i32,
    pub object_index: i32,
    pub id: i32,
}

/// Build a Game Maker project.
pub fn build<'a, F: FnMut() -> E, E: io::Write + 'static>(game: &'a project::Game, errors: F) ->
    Result<(Assets, vm::Debug), u32>
{
    let mut assets = Assets::default();
    assets.objects = game.objects.iter()
        .map(|&project::Object { persistent, .. }| Object { persistent })
        .collect();
    assets.rooms = game.rooms.iter()
        .map(|&project::Room { ref instances, .. }| Room {
            instances: instances.iter()
                .map(|&project::Instance { x, y, object_index, id, .. }| Instance {
                    x, y, object_index, id
                })
                .collect()
        })
        .collect();
    assets.next_instance = game.last_instance + 1;

    let mut items = HashMap::default();
    World::register(&mut items);
    match gml::build(game, &items, errors) {
        Ok((code, debug)) => Ok((Assets { code, ..assets }, debug)),
        Err(count) => Err(count),
    }
}

// Run a Game Maker game.
pub fn run(cx: &mut Context) {
    let mut thread = vm::Thread::default();
    if let Err(error) = crate::room::State::load_room(cx, &mut thread, 0) {
        let crate::World { show, .. } = &cx.world;
        show.show_vm_error(&*error);
    }
}
