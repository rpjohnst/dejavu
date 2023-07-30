#![feature(type_alias_impl_trait)]

use std::{io, iter};
use std::ops::Range;
use std::collections::HashMap;
use quickdry::Arena;
use gml::vm;
use gml::symbol::Symbol;

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
    pub libraries: Vec<platform::Library>,

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
    pub visible: bool,
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
pub fn build<F: Clone + FnMut() -> E, E: io::Write>(
    game: &project::Game<'_>, extensions: &[project::Extension<'_>], arena: &Arena, mut errors: F
) -> Result<(Assets, vm::Debug), u32>
{
    let mut assets = Assets::default();
    let debug;

    let mut items = HashMap::default();
    World::register(&mut items);
    (assets.code, debug) = gml::build(game, extensions, &items, errors.clone())?;

    let mut builder = atlas::Builder::default();
    for sprite @ &project::Sprite { origin, .. } in &game.sprites[..] {
        let start = builder.len();
        for &project::Image { size, data } in &sprite.images[..] {
            let data = match sprite.version {
                400 => { build_bmp(data, sprite.transparent, arena, errors())? }
                800 => { data }
                _ => unreachable!()
            };
            builder.insert(size, data);
        }
        let end = builder.len();

        assets.sprites.push(Sprite { origin, images: start..end });
    }
    for background @ &project::Background { name, size, data, .. } in &game.backgrounds[..] {
        let start = builder.len();
        let (width, height) = size;
        if !name.is_empty() && width * height > 0 {
            let data = match background.version {
                400 => { build_bmp(data, background.transparent, arena, errors())? }
                710 => { data }
                _ => unreachable!()
            };
            builder.insert(size, data);
        }

        // TODO: use a placeholder image for unused resource indices
        assets.backgrounds.push(Background { image: start });
    }
    (assets.textures, assets.images) = builder.build();

    assets.objects = game.objects.iter()
        .map(|&project::Object { sprite, visible, depth, persistent, .. }| Object {
            sprite_index: sprite, visible, depth: depth as f32, persistent
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
                            let (w, h) = assets.images[background.image].size;
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

fn build_bmp<'a, E: io::Write>(
    data: &[u8], transparent: bool, arena: &'a Arena, mut errors: E
) -> Result<&'a [u8], u32> {
    match project::read_bmp(&mut { data }, transparent, arena) {
        Ok(data) => { Ok(data) }
        Err(err) => {
            let _ = writeln!(errors, "error reading sprite: {err}");
            Err(1)
        }
    }
}

pub fn load(assets: &mut Assets, extensions: &[project::Extension<'_>]) -> io::Result<()> {
    let mut items = HashMap::default();
    World::register(&mut items);
    gml::load(&mut assets.code, &items);

    for extension in extensions {
        for file in &extension.files[..] {
            if file.kind == project::extension_kind::DLL {
                let file_name = Symbol::intern(file.file_name);
                std::fs::write(std::str::from_utf8(&file_name[..]).unwrap(), file.contents)?;
                let dll = platform::Library::load(file_name).unwrap();

                for function in &file.functions[..] {
                    let name = Symbol::intern(function.name);
                    let external_name = Symbol::intern(function.external_name);
                    let proc = dll.symbol(external_name.as_cstr()).unwrap();

                    let calltype = match function.calling_convention {
                        project::calling_convention::CDECL => { vm::dll::Cc::Cdecl }
                        project::calling_convention::STDCALL => { vm::dll::Cc::Stdcall }
                        _ => panic!()
                    };
                    let restype = match function.result {
                        project::parameter_type::REAL => { vm::dll::Type::Real }
                        project::parameter_type::STRING => { vm::dll::Type::String }
                        _ => panic!(),
                    };

                    let mut types = [vm::dll::Type::Real; 16];
                    let argtypes = &function.parameters[..function.parameters_used as usize];
                    for (ty, &argtype) in iter::zip(&mut types[..], argtypes) {
                        *ty = match argtype {
                            project::parameter_type::REAL => { vm::dll::Type::Real }
                            project::parameter_type::STRING => { vm::dll::Type::String }
                            _ => panic!()
                        }
                    }

                    let thunk = vm::dll::thunk(calltype, restype, &types[..argtypes.len()]).unwrap();

                    assets.code.dll.insert(name, (proc, thunk));
                }

                assets.libraries.push(dll);
            }
        }
    }

    Ok(())
}