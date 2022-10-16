#![feature(type_alias_impl_trait)]

use std::io;
use std::ops::Range;
use std::collections::HashMap;
use gml::vm;

pub use crate::world::*;
pub use crate::batch::Batch;

#[cfg(target_arch = "wasm32")]
pub use crate::platform::State;
pub use crate::platform::run;
#[cfg(target_arch = "wasm32")]
pub use crate::platform::stop;

mod world;
mod batch;

#[cfg_attr(target_arch = "wasm32", path = "platform/web.rs")]
#[cfg_attr(windows, path = "platform/win32.rs")]
#[cfg_attr(not(any(
    target_arch = "wasm32",
    windows,
)), path = "platform/unknown.rs")]
mod platform;

#[cfg_attr(target_arch = "wasm32", path = "graphics/webgl2.rs")]
#[cfg_attr(windows, path = "graphics/d3d11.rs")]
#[cfg_attr(not(any(
    target_arch = "wasm32",
    windows,
)), path = "graphics/unknown.rs")]
mod graphics;

pub struct Context {
    pub world: World,
    pub assets: Assets,
}

#[derive(Default)]
pub struct Assets {
    pub code: vm::Assets<Context>,

    pub textures: Vec<atlas::Texture>,
    pub images: Vec<atlas::Image>,

    pub sprites: Vec<Sprite>,
    pub objects: Vec<Object>,
    pub rooms: Vec<Room>,
    pub next_instance: i32,
    pub room_order: Vec<u32>,
}

pub struct Sprite {
    pub origin: (u32, u32),
    pub images: Range<usize>,
}

pub struct Object {
    pub sprite_index: i32,
    pub depth: f32,
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
pub fn build<'a, F: FnMut() -> E, E: io::Write>(game: &'a project::Game, errors: F) ->
    Result<(Assets, vm::Debug), u32>
{
    let mut assets = Assets::default();
    let debug;

    let mut items = HashMap::default();
    World::register(&mut items);
    (assets.code, debug) = match gml::build(game, &items, errors) {
        Ok(gml) => { gml }
        Err(count) => { return Err(count); }
    };

    let mut builder = atlas::Builder::default();
    for sprite @ &project::Sprite { origin, .. } in &game.sprites[..] {
        let start = builder.len();
        for &project::Image { size, data } in &sprite.images[..] {
            builder.insert(size, data);
        }
        let end = builder.len();

        assets.sprites.push(Sprite { origin, images: start..end });
    }
    (assets.textures, assets.images) = builder.build();

    assets.objects = game.objects.iter()
        .map(|&project::Object { sprite, depth, persistent, .. }| Object {
            sprite_index: sprite, depth: depth as f32, persistent
        })
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
    assets.room_order = game.room_order.clone();

    Ok((assets, debug))
}
