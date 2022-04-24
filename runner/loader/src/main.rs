#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{env, io};
use std::error::Error;
use std::fs::File;

fn main() -> Result<(), Box<dyn Error>> {
    let path = match env::args_os().nth(1) {
        Some(path) => { path }
        None => { return Err("expected project file, executable (.exe) or GML script (.gml)")?; }
    };

    let arena = quickdry::Arena::default();
    let mut game = project::Game::default();
    let gml;

    if path.to_string_lossy().ends_with(".gml") {
        gml = std::fs::read(path)?;
        let mut room = project::Room::default();
        room.code = &gml;
        game.rooms.push(room);
    } else if path.to_string_lossy().ends_with(".exe") {
        let mut read = std::io::BufReader::new(File::open(path)?);
        project::read_exe(&mut read, &mut game, &arena)?;
    } else {
        let mut read = File::open(path)?;
        project::read_gmk(&mut read, &mut game, &arena)?;
    }

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
