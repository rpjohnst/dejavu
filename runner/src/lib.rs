use std::{cmp, io};
use std::collections::HashMap;
use gml::vm;

pub use crate::world::*;
pub use crate::atlas::Atlas;
pub use crate::batch::Batch;

#[cfg(target_arch = "wasm32")]
pub use crate::platform::State;
#[cfg(target_arch = "wasm32")]
pub use crate::platform::stop;

mod world;
mod atlas;
mod batch;

#[cfg_attr(target_arch = "wasm32", path = "platform/web.rs")]
#[cfg_attr(not(any(
    target_arch = "wasm32",
)), path = "platform/unknown.rs")]
mod platform;

#[cfg_attr(target_arch = "wasm32", path = "graphics/webgl2.rs")]
#[cfg_attr(not(any(
    target_arch = "wasm32",
)), path = "graphics/unknown.rs")]
mod graphics;

pub struct Context {
    pub world: World,
    pub assets: Assets,
}

#[derive(Default)]
pub struct Assets {
    pub code: vm::Assets<Context>,
    pub textures: Vec<Texture>,
    pub sprites: Vec<Sprite>,
    pub objects: Vec<Object>,
    pub rooms: Vec<Room>,
    pub next_instance: i32,
}

pub struct Texture {
    pub size: (u16, u16),
    pub data: Vec<u8>,
}

pub struct Sprite {
    pub origin: (u32, u32),
    pub frames: Vec<Frame>,
}

pub struct Frame {
    pub texture: i32,
    pub pos: (u16, u16),
    pub size: (u16, u16),
}

pub struct Object {
    pub sprite_index: i32,
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
    let (textures, sprites) = compile_textures(game);
    assets.textures = textures;
    assets.sprites = sprites;
    assets.objects = game.objects.iter()
        .map(|&project::Object { sprite, persistent, .. }| Object {
            sprite_index: sprite,
            persistent
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

    let mut items = HashMap::default();
    World::register(&mut items);
    match gml::build(game, &items, errors) {
        Ok((code, debug)) => Ok((Assets { code, ..assets }, debug)),
        Err(count) => Err(count),
    }
}

struct Image {
    sprite: usize,
    frame: usize,

    pos: (u16, u16),
    size: (u16, u16),
}

fn compile_textures(game: &project::Game) -> (Vec<Texture>, Vec<Sprite>) {
    let mut texture = Vec::default();
    let mut sprites = Vec::default();

    let mut images = Vec::default();
    let mut area = 0;
    let mut max_width = 0;
    let mut max_height = 0;
    for (sprite, &project::Sprite { origin, ref frames, .. }) in game.sprites.iter().enumerate() {
        let mut assets = Vec::default();
        for (frame, &project::Frame { size, .. }) in frames.iter().enumerate() {
            let pos = (0, 0);

            let (width, height) = size;
            let size = (width as u16, height as u16);
            debug_assert!(width <= u16::MAX as u32 && height <= u16::MAX as u32);

            assets.push(Frame { pos, size, texture: 0 });
            images.push(Image { sprite, frame, pos, size });

            area += width as u32 * height as u32;
            max_width = cmp::max(max_width, width as u16);
            max_height = cmp::max(max_height, height as u16);
        }
        sprites.push(Sprite { origin, frames: assets });
    }

    // As a heuristic, sort by height and assume the packer will achieve about 75% utilization.
    images.sort_by_key(|&Image { size: (_, height), .. }| cmp::Reverse(height));
    let square = f32::sqrt(area as f32 / 0.75) as u16;
    let mut atlas_width = u16::next_power_of_two(cmp::max(max_width, square));
    let mut atlas_height = u16::next_power_of_two(cmp::max(max_height, square));
    let mut atlas = Atlas::new(atlas_width, atlas_height);
    'pack: loop {
        for image in &mut images {
            let (width, height) = image.size;
            if let Some(pos) = atlas.pack(width, height) {
                image.pos = pos;
            } else {
                // Something didn't fit. Move up to the next power of two and retry.
                atlas_width *= 2;
                atlas_height *= 2;
                atlas.reset(atlas_width, atlas_height);
                continue 'pack;
            }
        }
        break;
    }

    let len = atlas_width as usize * atlas_height as usize * 4;
    texture.resize_with(len, u8::default);
    for &Image { sprite, frame, pos: (x, y), size: (width, _) } in &images {
        sprites[sprite].frames[frame].pos = (x, y);

        let data = game.sprites[sprite].frames[frame].data;
        for (i, row) in data.chunks_exact(width as usize * 4).enumerate() {
            let start = (y as usize + i) * (atlas_width as usize * 4) + (x as usize * 4);
            texture[start..start + row.len()].copy_from_slice(row);
        }
    }

    let texture = Texture { size: (atlas_width, atlas_height), data: texture };
    (vec![texture], sprites)
}

// Run a Game Maker game.
pub fn run(cx: Context) -> platform::State { platform::run(cx) }
