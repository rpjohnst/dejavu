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

#[derive(Default)]
pub struct Assets {
    pub code: vm::Assets<World, Self>,
}

/// Build a Game Maker project.
pub fn build<'a, F: FnMut() -> E, E: io::Write + 'static>(
    game: &'a project::Game, engine: &HashMap<Symbol, gml::Item<World, Assets>>, errors: F
) -> Result<(Assets, vm::Debug), u32> {
    let assets = Assets::default();
    match gml::build(game, engine, errors) {
        Ok((code, debug)) => Ok((Assets { code, ..assets }, debug)),
        Err(count) => Err(count),
    }
}
