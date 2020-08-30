use std::collections::HashMap;
use std::io;

use gml::{self, symbol::Symbol, vm};

pub use crate::world::World;

mod world;
pub mod real;
pub mod string;
pub mod motion;
pub mod instance;
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
}

pub struct Object {
    pub persistent: bool,
}

/// Build a Game Maker project.
pub fn build<'a, F: FnMut() -> E, E: io::Write + 'static>(
    game: &'a project::Game, engine: &HashMap<Symbol, gml::Item<Context>>, errors: F
) -> Result<(Assets, vm::Debug), u32> {
    let mut assets = Assets::default();
    assets.objects = game.objects.iter()
        .map(|&project::Object { persistent, .. }| Object { persistent })
        .collect();
    match gml::build(game, engine, errors) {
        Ok((code, debug)) => Ok((Assets { code, ..assets }, debug)),
        Err(count) => Err(count),
    }
}
