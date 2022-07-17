#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{env, fs, io};
use std::error::Error;
use std::ffi::OsStr;
use std::fs::File;
use std::path::Path;

fn main() -> Result<(), Box<dyn Error>> {
    let path = match env::args_os().nth(1) {
        Some(path) => { path }
        None => { return Err("expected game")?; }
    };

    let arena = &quickdry::Arena::default();
    let mut game = project::Game::default();

    let path = Path::new(path.as_os_str());
    let kind = path.extension().unwrap_or_default();
    let gml;
    if kind == OsStr::new("gmk") {
        let read = fs::read(path)?;
        project::read_project(&read[..], &mut game, arena)?;
    } else if kind == OsStr::new("exe") {
        let mut read = io::BufReader::new(File::open(path)?);
        project::read_exe(&mut read, &mut game, arena)?;
    } else if kind == OsStr::new("gml") {
        gml = fs::read(path)?;
        let mut room = project::Room::default();
        room.code = &gml[..];
        game.rooms.push(room);
    } else {
        Err("unrecognized file type: expected project (.gmk), executable (.exe) or script (.gml)")?;
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
