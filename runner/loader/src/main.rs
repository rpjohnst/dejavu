#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{env, io};
use std::error::Error;
use std::fs::File;

fn main() -> Result<(), Box<dyn Error>> {
    let path = match env::args_os().nth(1) {
        Some(path) => { path }
        None => { return Err("expected project")?; }
    };

    let arena = quickdry::Arena::default();
    let mut game = project::Game::default();
    let mut read = File::open(path)?;
    project::read_gmk(&mut read, &mut game, &arena)?;

    let (assets, debug) = match runner::build(&game, io::stderr) {
        Ok(assets) => assets,
        Err(errors) => {
            let error = if errors > 1 {
                format!("aborting due to {} previous errors", errors)
            } else {
                format!("aborting due to previous error")
            };
            return Err(error)?;
        }
    };
    let world = runner::World::from_assets(&assets, debug);
    runner::run(runner::Context { world, assets });

    Ok(())
}
