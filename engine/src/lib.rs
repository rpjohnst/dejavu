use std::collections::HashMap;
use std::io;

use gml::{self, symbol::Symbol, vm};

pub use crate::world::World;

mod world;
pub mod real;
pub mod string;
pub mod motion;
pub mod instance;
pub mod room;
pub mod show;
pub mod data;

pub struct Context {
    pub world: World,
    pub assets: Assets,
}

#[derive(Default)]
pub struct Assets {
    pub code: vm::Assets<Context>,
    pub objects: Vec<Object>,
    pub rooms: Vec<Room>,
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
pub fn build<'a, F: FnMut() -> E, E: io::Write + 'static>(
    game: &'a project::Game, engine: &HashMap<Symbol, gml::Item<Context>>, errors: F
) -> Result<(Assets, vm::Debug), u32> {
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
    match gml::build(game, engine, errors) {
        Ok((code, debug)) => Ok((Assets { code, ..assets }, debug)),
        Err(count) => Err(count),
    }
}
