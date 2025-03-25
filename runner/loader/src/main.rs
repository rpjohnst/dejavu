#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{env, fs, io, mem};
use std::error::Error;
use std::ffi::OsStr;
use std::fs::File;
use std::path::Path;
use bstr::BStr;

fn main() -> Result<(), Box<dyn Error>> {
    let mut path = None;
    let mut installed = Vec::default();

    let mut args = env::args_os();
    args.next();
    while let Some(arg) = args.next() {
        if arg == OsStr::new("--extension") {
            let extension = args.next().ok_or("expected extension (.ged or .gex)")?;
            installed.push(extension);
        } else if path.is_none() {
            path = Some(arg);
        } else {
            Err("expected a single project")?;
        }
    }
    let (path, kind) = path.as_deref()
        .map(Path::new)
        .and_then(|path| { let kind = path.extension()?; Some((path, kind)) })
        .ok_or("expected project (.gmd or .gmk), executable (.exe), or script (.gml)")?;

    let arena = quickdry::Arena::default();
    let mut game = project::Game::default();
    let mut extensions = Vec::with_capacity(installed.len());

    let gml;
    if kind == OsStr::new("gmk") {
        let read = fs::read(path)?;
        project::read_project(&read[..], &mut game, &arena)?;
    } else if kind == OsStr::new("exe") {
        let mut read = io::BufReader::new(File::open(path)?);
        project::read_exe(&mut read, &mut game, &mut extensions, &arena)?;
    } else if kind == OsStr::new("gml") {
        gml = fs::read(path)?;
        let mut room = project::Room::default();
        room.code = BStr::new(&gml[..]);
        game.rooms.push(room);
        game.room_order.push(0);
    }

    for path in installed {
        extensions.push(project::Extension::default());

        let extension = extensions.last_mut().unwrap();
        let path = Path::new(path.as_os_str());
        let kind = path.extension().unwrap_or_default();
        if kind == OsStr::new("ged") {
            let read = fs::read(path)?;
            project::read_ged(&mut &read[..], false, extension, &arena)?;
        } else if kind == OsStr::new("gex") {
            let mut read = io::BufReader::new(File::open(path)?);
            project::read_gex(&mut read, extension, &arena)?;
        } else {
            Err("unrecognized extension type")?;
        }
    }

    let (mut assets, debug) = match runner::build(&game, &extensions[..], &arena, io::stderr) {
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
    runner::load(&mut assets, &extensions[..])?;
    mem::drop(arena);

    let world = runner::World::from_assets(&assets, debug);
    runner::run(runner::Context { world, assets });

    Ok(())
}
