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
    pub backgrounds: Vec<Background>,
    pub objects: Vec<Object>,
    pub rooms: Vec<Room>,
    pub next_instance: i32,
    pub room_order: Vec<u32>,
}

pub struct Sprite {
    pub origin: (u32, u32),
    pub images: Range<usize>,
}

pub struct Background {
    pub image: usize,
}

pub struct Object {
    pub sprite_index: i32,
    pub depth: f32,
    pub persistent: bool,
}

pub struct Room {
    pub size: (u32, u32),
    pub backgrounds: Vec<Layer>,
    pub instances: Vec<Instance>,
}

pub struct Layer {
    pub visible: bool,
    pub foreground: bool,
    pub background: i32,
    pub x: i32,
    pub y: i32,
    pub htiled: bool,
    pub vtiled: bool,
    pub xscale: f32,
    pub yscale: f32,
    pub hspeed: i32,
    pub vspeed: i32,
}

pub struct Instance {
    pub x: i32,
    pub y: i32,
    pub object_index: i32,
    pub id: i32,
}

/// Build a Game Maker project.
pub fn build<F: FnMut() -> E, E: io::Write>(
    game: &project::Game<'_>, extensions: &[project::Extension<'_>], errors: F
) -> Result<(Assets, vm::Debug), u32>
{
    let mut assets = Assets::default();
    let debug;

    let mut items = HashMap::default();
    World::register(&mut items);
    (assets.code, debug) = match gml::build(game, extensions, &items, errors) {
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
    for &project::Background { name, size, data, .. } in &game.backgrounds[..] {
        let start = builder.len();
        let (width, height) = size;
        if !name.is_empty() && width * height > 0 {
            builder.insert(size, data);
        }

        // TODO: use a placeholder image for unused resource indices
        assets.backgrounds.push(Background { image: start });
    }
    (assets.textures, assets.images) = builder.build();

    assets.objects = game.objects.iter()
        .map(|&project::Object { sprite, depth, persistent, .. }| Object {
            sprite_index: sprite, depth: depth as f32, persistent
        })
        .collect();

    assets.rooms = game.rooms.iter()
        .map(|&project::Room { ref backgrounds, ref instances, width, height, .. }| Room {
            size: (width, height),
            backgrounds: backgrounds.iter()
                .map(|&project::RoomBackground {
                    visible, foreground, background, x, y, htiled, vtiled, hspeed, vspeed, stretch
                }| {
                    let (mut xscale, mut yscale) = (1.0, 1.0);
                    if stretch {
                        if let Some(background) = assets.backgrounds.get(background as usize) {
                            let (w, h) = assets.textures[background.image].size;
                            (xscale, yscale) = (width as f32 / w as f32, height as f32 / h as f32);
                        }
                    }
                    Layer {
                        visible, foreground, background,
                        x, y, htiled, vtiled, xscale, yscale, hspeed, vspeed
                    }
                })
                .collect(),
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
