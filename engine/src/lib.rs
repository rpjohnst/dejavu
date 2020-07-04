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
pub fn build<'a, F, E>(
    game: &'a project::Game, engine: &HashMap<Symbol, gml::Item<World, Assets>>, mut errors: F
) ->
    Result<Assets, (u32, Assets)>
where
    F: FnMut() -> E,
    E: io::Write + 'static,
{
    let mut assets = Assets::default();
    match gml::build(game, engine, &mut errors) {
        Ok(code) => { assets.code = code; },
        Err((count, code)) => {
            assets.code = code;
            return Err((count, assets));
        }
    };
    Ok(assets)
}
