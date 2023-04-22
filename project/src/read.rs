use std::{iter, ptr, slice};
use std::alloc::{Layout, handle_alloc_error};
use std::io::{self, Read, BufRead, Seek, SeekFrom};
use flate2::bufread::ZlibDecoder;
use quickdry::Arena;

use crate::{
    Game, Constant,
    Sound,
    Sprite, Image, Mask,
    Background,
    Path, Point,
    Script,
    Object, Action, Event,
    Room, RoomBackground, View, Instance, Tile,
    Extension, ExtensionFile, ExtensionFunction, ExtensionConstant,
};

const GM_OFFSET_500: u64 = 1_500_000;
const EXE_MAGIC_500: u32 = 1230500;

const GM_OFFSET_800: u64 = 2_000_000;
const GM_MAGIC: u32 = 1234321;

pub fn read_project<'a>(read: &[u8], game: &mut Game<'a>, arena: &'a Arena) -> io::Result<()> {
    let read = &mut { read };
    let buf = &mut Vec::default();

    if read.next_u32()? != GM_MAGIC {
        return Err(io::Error::from(io::ErrorKind::InvalidData));
    }

    read.read_u32(&mut game.version)?;
    if game.version != 530 && game.version != 800 {
        return Err(io::Error::from(io::ErrorKind::InvalidData));
    }

    if game.version == 530 { read.read_u32(&mut game.debug)?; }
    read_body(read, buf, false, game, &mut Vec::default(), arena)?;

    Ok(())
}

pub fn read_exe<'a, R: BufRead + Seek>(
    read: &mut R, game: &mut Game<'a>, extensions: &mut Vec<Extension<'a>>, arena: &'a Arena
) -> io::Result<()> {
    let mut buf = [0; 2];
    read.read_exact(&mut buf[..])?;
    if &buf != b"MZ" {
        return Err(io::Error::from(io::ErrorKind::InvalidData));
    }

    if {
        read.seek(SeekFrom::Start(GM_OFFSET_500))?;
        read.next_u32()? == EXE_MAGIC_500
    } {
        // swap tables

        let seed = read.next_u32()? as usize;

        let mut forward = [0u8; 256];
        for i in 0..256 { forward[i] = i as u8; }
        for i in 1..10001 {
            let j = (i * seed) % 254 + 1;
            forward.swap(j, j + 1);
        }

        let mut reverse = [0u8; 256];
        for i in 0..256 { reverse[forward[i] as usize] = i as u8; }

        // decrypt

        let mut data = Vec::default();
        read.read_to_end(&mut data)?;
        for x in &mut data[..] { *x = reverse[*x as usize]; }

        // game data

        let read = &mut &data[..];

        read.next_u32()?;
        read.skip_blob()?;

        read_project(*read, game, arena)?;
    } else if {
        read.seek(SeekFrom::Start(GM_OFFSET_800))?;
        read.next_u32()? == GM_MAGIC
    } {
        let buf = &mut Vec::default();

        read.read_u32(&mut game.version)?;
        if game.version != 800 {
            return Err(io::Error::from(io::ErrorKind::InvalidData));
        }

        read.read_u32(&mut game.debug)?;

        {
            let version = read.next_u32()?;
            if version != 800 {
                return Err(io::Error::from(io::ErrorKind::InvalidData));
            }

            let read = &mut read.read_blob_zlib(buf)?;
            read_settings(read, true, version, game, arena)?;
            assert!(read.is_empty());
        }

        read.skip_blob()?;
        read.skip_blob()?;

        // swap tables

        let mut forward = [0u8; 256];
        let skip_1 = read.next_u32()? as i64;
        let skip_2 = read.next_u32()? as i64;
        read.seek(SeekFrom::Current(skip_1 * 4))?;
        read.read_exact(&mut forward)?;
        read.seek(SeekFrom::Current(skip_2 * 4))?;

        let mut reverse = [0u8; 256];
        for i in 0..256 { reverse[forward[i] as usize] = i as u8; }

        // decrypt

        let len = read.next_u32()? as usize;
        let mut data = Vec::from_iter(iter::repeat(0).take(len));
        read.read_exact(&mut data[..])?;

        for i in (1..data.len()).rev() {
            data[i] = (reverse[data[i] as usize] as i32 - data[i - 1] as i32 - i as i32) as u8;
        }
        for i in (0..data.len()).rev() {
            let j = usize::saturating_sub(i, forward[i & 0xff] as usize);
            data.swap(i, j);
        }

        // game data

        let read = &mut &data[..];

        let skip = read.next_u32()? as usize;
        *read = &read[skip * 4..];

        read.read_bool(&mut game.pro)?;
        read_body(read, buf, true, game, extensions, arena)?;
    } else {
        return Err(io::Error::from(io::ErrorKind::InvalidData));
    }

    Ok(())
}

fn read_body<'a>(
    read: &mut &[u8], buf: &mut Vec<u8>, exe: bool,
    game: &mut Game<'a>, extensions: &mut Vec<Extension<'a>>, arena: &'a Arena
) -> io::Result<()> {
    read.read_u32(&mut game.id)?;
    read.read_u32(&mut game.guid[0])?;
    read.read_u32(&mut game.guid[1])?;
    read.read_u32(&mut game.guid[2])?;
    read.read_u32(&mut game.guid[3])?;

    // settings

    if game.version == 530 || (game.version == 800 && !exe) {
        let version = read.next_u32()?;
        if version != 530 && version != 800 {
            return Err(io::Error::from(io::ErrorKind::InvalidData));
        }

        if version == 530 {
            read_settings(read, exe, version, game, arena)?;
        }
        if version == 800 {
            let read = &mut read.read_blob_zlib(buf)?;
            read_settings(read, exe, version, game, arena)?;
            assert!(read.is_empty());
        }
    }

    // extensions

    if game.version == 800 && exe {
        let _version = read.next_u32()?;
        let len = read.next_u32()? as usize;
        game.extensions.reserve(len);
        extensions.reserve(len);
        for id in 0..len {
            extensions.push(Extension::default());

            let extension = &mut extensions[id];
            read_ged(read, true, extension, arena)?;
            game.extensions.push(extension.name);

            let len = read.next_u32()?;
            if read.len() < len as usize {
                return Err(io::Error::from(io::ErrorKind::UnexpectedEof));
            }
            let read = &mut &std::mem::replace(read, &read[len as usize..])[..len as usize];

            // swap tables

            let seed = read.next_u32()? as usize;
            let a = seed % 250 + 6;
            let b = seed / 250;

            let mut forward = [0u8; 256];
            for i in 0..256 { forward[i] = i as u8; }
            for i in 1..10001 {
                let j = (i * a + b) % 254 + 1;
                forward.swap(j, j + 1);
            }

            let mut reverse = [0u8; 256];
            for i in 0..256 { reverse[forward[i] as usize] = i as u8; }

            // decrypt

            let mut data = Vec::with_capacity(read.len());
            data.push(read[0]);
            for &x in &read[1..] { data.push(reverse[x as usize]); }

            // extension files

            let read = &mut &data[..];

            for file in &mut extension.files[..] {
                read.read_blob_zlib(buf)?;
                file.contents = alloc_buf(arena, &buf[..]);
            }
        }
    }

    // triggers

    if game.version == 800 {
        let _version = read.next_u32()?;
        let _len = read.next_u32()?;
        assert_eq!(_len, 0);
        if !exe { let _time = read.next_f64()?; }
    }

    // constants

    if game.version == 800 {
        let _version = read.next_u32()?;
        read_constants(read, game, arena)?;
        if !exe { let _time = read.next_f64()?; }
    }

    // sounds

    let version = read.next_u32()?;
    if version != 400 && version != 800 {
        return Err(io::Error::from(io::ErrorKind::InvalidData));
    }

    let len = read.next_u32()? as usize;
    game.sounds.reserve(len);
    for id in 0..len {
        game.sounds.push(Sound::default());

        let sound = &mut game.sounds[id];
        if version == 400 {
            read_sound(read, exe, version, sound, arena)?;
        }
        if version == 800 {
            let read = &mut read.read_blob_zlib(buf)?;
            read_sound(read, exe, version, sound, arena)?;
            assert!(read.is_empty());
        }
    }

    fn read_sound<'a>(
        read: &mut &[u8], exe: bool, version: u32, sound: &mut Sound<'a>, arena: &'a Arena
    ) -> io::Result<()> {
        if !read.next_bool()? { return Ok(()); }

        read.read_blob(&mut sound.name, arena)?;
        if version == 800 && !exe { let _time = read.next_f64()?; }

        let version = read.next_u32()?;
        if version != 440 && version != 800 {
            return Err(io::Error::from(io::ErrorKind::InvalidData));
        }

        let mut kind_440 = -1;
        if version == 440 {
            read.read_i32(&mut kind_440)?;
        }
        if version == 800 {
            read.read_u32(&mut sound.kind)?;
        }
        read.read_blob(&mut sound.file_type, arena)?;
        if version == 440 {
            if kind_440 != -1 {
                let mut data = Vec::default();
                read.read_blob_zlib(&mut data)?;
            }
            let _allow_for_effects = read.next_bool()?;
            let _buffers = read.next_u32()?;
            let _load_on_use = read.next_bool()?;
        }
        if version == 800 {
            read.read_blob(&mut sound.file_name, arena)?;
            if read.next_bool()? {
                read.read_blob(&mut sound.data, arena)?;
            }
            read.read_u32(&mut sound.effects)?;
            read.read_f64(&mut sound.volume)?;
            read.read_f64(&mut sound.pan)?;
            read.read_bool(&mut sound.preload)?;
        }

        Ok(())
    }

    // sprites

    let version = read.next_u32()?;
    if version != 400 && version != 800 {
        return Err(io::Error::from(io::ErrorKind::InvalidData));
    }

    let len = read.next_u32()? as usize;
    game.sprites.reserve(len);
    for id in 0..len {
        game.sprites.push(Sprite::default());

        let sprite = &mut game.sprites[id];
        if version == 400 {
            read_sprite(read, buf, exe, version, sprite, arena)?;
        }
        if version == 800 {
            let read = &mut read.read_blob_zlib(buf)?;
            read_sprite(read, &mut Vec::default(), exe, version, sprite, arena)?;
            assert!(read.is_empty());
        }
    }

    fn read_sprite<'a>(
        read: &mut &[u8], buf: &mut Vec<u8>, exe: bool,
        version: u32, sprite: &mut Sprite<'a>, arena: &'a Arena
    ) -> io::Result<()> {
        if !read.next_bool()? { return Ok(()); }

        read.read_blob(&mut sprite.name, arena)?;
        if version == 800 && !exe { let _time = read.next_f64()?; }

        let version = read.next_u32()?;
        if version != 400 && version != 800 {
            return Err(io::Error::from(io::ErrorKind::InvalidData));
        }

        let mut width = 0;
        let mut height = 0;
        if version == 400 {
            width = read.next_u32()?;
            height = read.next_u32()?;

            read.read_i32(&mut sprite.bounds.left)?;
            read.read_i32(&mut sprite.bounds.right)?;
            read.read_i32(&mut sprite.bounds.bottom)?;
            read.read_i32(&mut sprite.bounds.top)?;

            let _transparent = read.next_bool()?;

            read.read_u32(&mut sprite.bounds_kind)?;
            let _precise = read.next_bool()?;

            let _use_vram = read.next_bool()?;
            let _lazy_load = read.next_bool()?;
        }

        read.read_u32(&mut sprite.origin.0)?;
        read.read_u32(&mut sprite.origin.1)?;

        let len = read.next_u32()? as usize;
        sprite.images.reserve(len);
        for i in 0..len {
            sprite.images.push(Image::default());

            let image = &mut sprite.images[i];
            if version == 400 {
                if read.next_i32()? == -1 { continue; }

                image.size.0 = width;
                image.size.1 = height;

                read.read_blob_zlib(buf)?;
                image.data = alloc_buf(arena, buf);
            }
            if version == 800 {
                let version = read.next_u32()?;
                if version != 800 {
                    return Err(io::Error::from(io::ErrorKind::InvalidData));
                }

                read.read_u32(&mut image.size.0)?;
                read.read_u32(&mut image.size.1)?;
                if image.size.0 != 0 && image.size.1 != 0 {
                    read.read_blob(&mut image.data, arena)?;
                }
            }
        }

        if version == 800 {
            if !exe {
                read.read_u32(&mut sprite.shape)?;
                read.read_u32(&mut sprite.alpha_tolerance)?;
            }
            read.read_bool(&mut sprite.separate_collision)?;
            if !exe {
                read.read_u32(&mut sprite.bounds_kind)?;
                read.read_i32(&mut sprite.bounds.left)?;
                read.read_i32(&mut sprite.bounds.right)?;
                read.read_i32(&mut sprite.bounds.bottom)?;
                read.read_i32(&mut sprite.bounds.top)?;
            }

            if exe && len > 0 {
                let len = if sprite.separate_collision { len } else { 1 };
                sprite.masks.reserve(len);
                for i in 0..len {
                    sprite.masks.push(Mask::default());

                    let mask = &mut sprite.masks[i];
                    let _version = read.next_u32()?;
                    read.read_u32(&mut mask.size.0)?;
                    read.read_u32(&mut mask.size.1)?;
                    read.read_i32(&mut mask.bounds.left)?;
                    read.read_i32(&mut mask.bounds.right)?;
                    read.read_i32(&mut mask.bounds.bottom)?;
                    read.read_i32(&mut mask.bounds.top)?;

                    let size = mask.size.0 as usize * mask.size.1 as usize;
                    mask.data.reserve(size);
                    for _ in 0..size {
                        mask.data.push(read.next_u32()?);
                    }
                }
            }
        }

        Ok(())
    }

    // backgrounds

    let version = read.next_u32()?;
    if version != 400 && version != 800 {
        return Err(io::Error::from(io::ErrorKind::InvalidData));
    }

    let len = read.next_u32()? as usize;
    game.backgrounds.reserve(len);
    for id in 0..len {
        game.backgrounds.push(Background::default());

        let background = &mut game.backgrounds[id];
        if version == 400 {
            read_background(read, buf, exe, version, background, arena)?;
        }
        if version == 800 {
            let read = &mut read.read_blob_zlib(buf)?;
            read_background(read, &mut Vec::default(), exe, version, background, arena)?;
            assert!(read.is_empty());
        }
    }

    fn read_background<'a>(
        read: &mut &[u8], buf: &mut Vec<u8>, exe: bool,
        version: u32, background: &mut Background<'a>, arena: &'a Arena
    ) -> io::Result<()> {
        if !read.next_bool()? { return Ok(()); }

        read.read_blob(&mut background.name, arena)?;
        if version == 800 && !exe { let _time = read.next_f64()?; }

        let version = read.next_u32()?;
        if version != 400 && version != 710 {
            return Err(io::Error::from(io::ErrorKind::InvalidData));
        }

        if version == 400 {
            read.read_u32(&mut background.size.0)?;
            read.read_u32(&mut background.size.1)?;

            let _transparent = read.next_bool()?;
            let _use_vram = read.next_bool()?;
            let _lazy_load = read.next_bool()?;

            if read.next_bool()? && read.next_i32()? != -1 {
                read.read_blob_zlib(buf)?;
                background.data = alloc_buf(arena, buf);
            }
        }
        if version == 710 {
            if !exe {
                let _tileset = read.next_bool()?;
                let _tile_width = read.next_u32()?;
                let _tile_height = read.next_u32()?;
                let _tile_off_x = read.next_u32()?;
                let _tile_off_y = read.next_u32()?;
                let _tile_sep_x = read.next_u32()?;
                let _tile_sep_y = read.next_u32()?;
            }

            let version = read.next_u32()?;
            if version != 800 {
                return Err(io::Error::from(io::ErrorKind::InvalidData));
            }

            read.read_u32(&mut background.size.0)?;
            read.read_u32(&mut background.size.1)?;
            if background.size.0 > 0 && background.size.1 > 0 {
                read.read_blob(&mut background.data, arena)?;
            }
        }

        Ok(())
    }

    // paths

    let version = read.next_u32()?;
    if version != 420 && version != 800 {
        return Err(io::Error::from(io::ErrorKind::InvalidData));
    }

    let len = read.next_u32()? as usize;
    game.paths.reserve(len);
    for id in 0..len {
        game.paths.push(Path::default());

        let path = &mut game.paths[id];
        if version == 420 {
            read_path(read, exe, version, path, arena)?;
        }
        if version == 800 {
            let read = &mut read.read_blob_zlib(buf)?;
            read_path(read, exe, version, path, arena)?;
            assert!(read.is_empty());
        }
    }

    fn read_path<'a>(
        read: &mut &[u8], exe: bool, version: u32, path: &mut Path<'a>, arena: &'a Arena
    ) -> io::Result<()> {
        if !read.next_bool()? { return Ok(()); }

        read.read_blob(&mut path.name, arena)?;
        if version == 800 && !exe { let _time = read.next_f64()?; }

        let version = read.next_u32()?;
        if version != 530 {
            return Err(io::Error::from(io::ErrorKind::InvalidData));
        }

        read.read_bool(&mut path.smooth)?;
        read.read_bool(&mut path.closed)?;
        read.read_u32(&mut path.precision)?;
        if version == 530 {
            let _room = read.next_u32()?;
            let _snap_x = read.next_u32()?;
            let _snap_y = read.next_u32()?;
        }

        let len = read.next_u32()? as usize;
        path.points.reserve(len);
        for i in 0..len {
            path.points.push(Point::default());

            let point = &mut path.points[i];
            read.read_f64(&mut point.position.0)?;
            read.read_f64(&mut point.position.1)?;
            read.read_f64(&mut point.speed)?;
        }

        Ok(())
    }

    // scripts

    let version = read.next_u32()?;
    if version != 400 && version != 800 {
        return Err(io::Error::from(io::ErrorKind::InvalidData));
    }

    let len = read.next_u32()? as usize;
    game.scripts.reserve(len);
    for id in 0..len {
        game.scripts.push(Script::default());

        let script = &mut game.scripts[id];
        if version == 400 {
            read_script(read, exe, version, script, arena)?;
        }
        if version == 800 {
            let read = &mut read.read_blob_zlib(buf)?;
            read_script(read, exe, version, script, arena)?;
            assert!(read.is_empty());
        }
    }

    fn read_script<'a>(
        read: &mut &[u8], exe: bool, version: u32, script: &mut Script<'a>, arena: &'a Arena
    ) -> io::Result<()> {
        if !read.next_bool()? { return Ok(()); }

        read.read_blob(&mut script.name, arena)?;
        if version == 800 && !exe { let _time = read.next_f64()?; }

        let version = read.next_u32()?;
        if version != 400 && version != 800 {
            return Err(io::Error::from(io::ErrorKind::InvalidData));
        }

        read.read_blob(&mut script.body, arena)?;

        Ok(())
    }

    // data files and fonts

    let version = read.next_u32()?;
    if version != 440 && version != 800 {
        return Err(io::Error::from(io::ErrorKind::InvalidData));
    }

    let len = read.next_u32()? as usize;
    for _ in 0..len {
        if version == 440 {
            read_data(read, buf, version, arena)?;
        }
        if version == 800 {
            let read = &mut read.read_blob_zlib(buf)?;
            read_font(read, exe, version, arena)?;
            assert!(read.is_empty());
        }
    }

    fn read_data<'a>(
        read: &mut &[u8], buf: &mut Vec<u8>, _version: u32, arena: &'a Arena
    ) -> io::Result<()> {
        if !read.next_bool()? { return Ok(()); }

        let mut name = &[][..];
        read.read_blob(&mut name, arena)?;

        let version = read.next_u32()?;
        if version != 440 {
            return Err(io::Error::from(io::ErrorKind::InvalidData));
        }

        let mut path = &[][..];
        read.read_blob(&mut path, arena)?;

        if read.next_bool()? {
            let _data = read.read_blob_zlib(buf)?;
        }

        let _export = read.next_u32()?;
        let _overwrite = read.next_bool()?;
        let _free = read.next_bool()?;
        let _remove = read.next_bool()?;

        Ok(())
    }

    fn read_font<'a>(
        read: &mut &[u8], exe: bool, _version: u32, arena: &'a Arena
    ) -> io::Result<()> {
        if !read.next_bool()? { return Ok(()); }

        let mut name = &[][..];
        read.read_blob(&mut name, arena)?;
        if !exe { let _time = read.next_f64()?; }

        let version = read.next_u32()?;
        if version != 800 {
            return Err(io::Error::from(io::ErrorKind::InvalidData));
        }

        let mut font = &[][..];
        read.read_blob(&mut font, arena)?;

        let _size = read.next_u32()?;
        let _bold = read.next_bool()?;
        let _italic = read.next_bool()?;
        let _start = read.next_u32()?;
        let _end = read.next_u32()?;

        Ok(())
    }

    // timelines

    let version = read.next_u32()?;
    if version != 500 && version != 800 {
        return Err(io::Error::from(io::ErrorKind::InvalidData));
    }

    let len = read.next_u32()? as usize;
    for _ in 0..len {
        if version == 500 {
            read_timeline(read, exe, version, arena)?;
        }
        if version == 800 {
            let read = &mut read.read_blob_zlib(buf)?;
            read_timeline(read, exe, version, arena)?;
            assert!(read.is_empty());
        }
    }

    fn read_timeline<'a>(
        read: &mut &[u8], exe: bool, version: u32, arena: &'a Arena
    ) -> io::Result<()> {
        if !read.next_bool()? { return Ok(()); }

        let mut name = &[][..];
        read.read_blob(&mut name, arena)?;
        if version == 800 && !exe { let _time = read.next_f64()?; }

        let version = read.next_u32()?;
        if version != 500 {
            return Err(io::Error::from(io::ErrorKind::InvalidData));
        }

        for _ in 0..read.next_u32()? {
            let _time = read.next_u32()?;

            let version = read.next_u32()?;
            if version != 400 {
                return Err(io::Error::from(io::ErrorKind::InvalidData));
            }

            for _ in 0..read.next_u32()? {
                let mut action = Action::default();
                read_action(read, &mut action, arena)?;
            }
        }

        Ok(())
    }

    // objects

    let version = read.next_u32()?;
    if version != 400 && version != 800 {
        return Err(io::Error::from(io::ErrorKind::InvalidData));
    }

    let len = read.next_u32()? as usize;
    game.objects.reserve(len);
    for id in 0..len {
        game.objects.push(Object::default());

        let object = &mut game.objects[id];
        if version == 400 {
            read_object(read, exe, version, object, arena)?;
        }
        if version == 800 {
            let read = &mut read.read_blob_zlib(buf)?;
            read_object(read, exe, version, object, arena)?;
            assert!(read.is_empty());
        }
    }

    fn read_object<'a>(
        read: &mut &[u8], exe: bool, version: u32, object: &mut Object<'a>, arena: &'a Arena
    ) -> io::Result<()> {
        if !read.next_bool()? { return Ok(()); }

        read.read_blob(&mut object.name, arena)?;
        if version == 800 && !exe { let _time = read.next_f64()?; }

        let version = read.next_u32()?;
        if version != 430 {
            return Err(io::Error::from(io::ErrorKind::InvalidData));
        }

        read.read_i32(&mut object.sprite)?;
        read.read_bool(&mut object.solid)?;
        read.read_bool(&mut object.visible)?;
        read.read_i32(&mut object.depth)?;
        read.read_bool(&mut object.persistent)?;
        read.read_i32(&mut object.parent)?;
        read.read_i32(&mut object.mask)?;

        let len = read.next_u32()? + 1;
        for event_type in 0..len {
            loop {
                let event_kind = read.next_i32()?;
                if event_kind == -1 {
                    break;
                }

                object.events.push(Event::default());

                let id = object.events.len() - 1;
                let event = &mut object.events[id];
                event.event_type = event_type;
                event.event_kind = event_kind;

                let version = read.next_u32()?;
                if version != 400 {
                    return Err(io::Error::from(io::ErrorKind::InvalidData));
                }

                let len = read.next_u32()? as usize;
                event.actions.reserve(len);
                for id in 0..len {
                    event.actions.push(Action::default());

                    let action = &mut event.actions[id];
                    read_action(read, action, arena)?;
                }
            }
        }

        Ok(())
    }

    // rooms

    let version = read.next_u32()?;
    if version != 420 && version != 800 {
        return Err(io::Error::from(io::ErrorKind::InvalidData));
    }

    let len = read.next_u32()? as usize;
    game.rooms.reserve(len);
    for id in 0..len {
        game.rooms.push(Room::default());

        let room = &mut game.rooms[id];
        if version == 420 {
            read_room(read, exe, version, room, arena)?;
        }
        if version == 800 {
            let read = &mut read.read_blob_zlib(buf)?;
            read_room(read, exe, version, room, arena)?;
            assert!(read.is_empty());
        }
    }

    fn read_room<'a>(
        read: &mut &[u8], exe: bool, version: u32, room: &mut Room<'a>, arena: &'a Arena
    ) -> io::Result<()> {
        if !read.next_bool()? { return Ok(()); }

        read.read_blob(&mut room.name, arena)?;
        if version == 800 && !exe { let _time = read.next_f64()?; }

        let version = read.next_u32()?;
        if version != 520 && version != 541 {
            return Err(io::Error::from(io::ErrorKind::InvalidData));
        }

        read.read_blob(&mut room.caption, arena)?;

        read.read_u32(&mut room.width)?;
        read.read_u32(&mut room.height)?;
        if version == 520 || (version == 541 && !exe) {
            let _snap_x = read.next_u32()?;
            let _snap_y = read.next_u32()?;
            let _isometric = read.next_bool()?;
        }
        read.read_u32(&mut room.speed)?;
        read.read_bool(&mut room.persistent)?;
        read.read_u32(&mut room.clear_color)?;
        read.read_bool(&mut room.clear)?;

        read.read_blob(&mut room.code, arena)?;

        let len = read.next_u32()? as usize;
        room.backgrounds.reserve(len);
        for i in 0..len {
            room.backgrounds.push(RoomBackground::default());

            let background = &mut room.backgrounds[i];
            read.read_bool(&mut background.visible)?;
            read.read_bool(&mut background.foreground)?;
            read.read_i32(&mut background.background)?;
            read.read_i32(&mut background.x)?;
            read.read_i32(&mut background.y)?;
            read.read_bool(&mut background.htiled)?;
            read.read_bool(&mut background.vtiled)?;
            read.read_i32(&mut background.hspeed)?;
            read.read_i32(&mut background.vspeed)?;
            read.read_bool(&mut background.stretch)?;
        }

        read.read_bool(&mut room.enable_views)?;
        let len = read.next_u32()? as usize;
        room.views.reserve(len);
        for i in 0..len {
            room.views.push(View::default());

            let view = &mut room.views[i];
            read.read_bool(&mut view.visible)?;
            read.read_i32(&mut view.view_x)?;
            read.read_i32(&mut view.view_y)?;
            read.read_u32(&mut view.view_w)?;
            read.read_u32(&mut view.view_h)?;
            read.read_i32(&mut view.port_x)?;
            read.read_i32(&mut view.port_y)?;
            if version == 541 {
                read.read_u32(&mut view.port_w)?;
                read.read_u32(&mut view.port_h)?;
            }
            read.read_i32(&mut view.h_border)?;
            read.read_i32(&mut view.v_border)?;
            read.read_i32(&mut view.h_speed)?;
            read.read_i32(&mut view.v_speed)?;
            read.read_i32(&mut view.target)?;
        }

        let len = read.next_u32()? as usize;
        room.instances.reserve(len);
        for i in 0..len {
            room.instances.push(Instance::default());

            let instance = &mut room.instances[i];
            read.read_i32(&mut instance.x)?;
            read.read_i32(&mut instance.y)?;
            read.read_i32(&mut instance.object_index)?;
            read.read_i32(&mut instance.id)?;
            read.read_blob(&mut instance.code, arena)?;
            if version == 520 || (version == 541 && !exe) { let _locked = read.next_bool()?; }
        }

        let len = read.next_u32()? as usize;
        room.tiles.reserve(len);
        for i in 0..len {
            room.tiles.push(Tile::default());

            let tile = &mut room.tiles[i];
            read.read_i32(&mut tile.x)?;
            read.read_i32(&mut tile.y)?;
            read.read_i32(&mut tile.background)?;
            read.read_i32(&mut tile.tile_x)?;
            read.read_i32(&mut tile.tile_y)?;
            read.read_u32(&mut tile.width)?;
            read.read_u32(&mut tile.height)?;
            read.read_i32(&mut tile.depth)?;
            read.read_i32(&mut tile.id)?;
            if version == 520 || (version == 541 && !exe) { let _locked = read.next_bool()?; }
        }

        if version == 520 || (version == 541 && !exe) {
            let _configured = read.next_bool()?;
            let _editor_width = read.next_u32()?;
            let _editor_height = read.next_u32()?;
            let _editor_grid = read.next_bool()?;
            let _editor_objects = read.next_bool()?;
            let _editor_tiles = read.next_bool()?;
            let _editor_backgrounds = read.next_bool()?;
            let _editor_foregrounds = read.next_bool()?;
            let _editor_views = read.next_bool()?;
            let _editor_delete_objects = read.next_bool()?;
            let _editor_delete_tiles = read.next_bool()?;
            if version == 520 {
                read.next_u32()?;
                read.next_u32()?;
                read.next_u32()?;
                read.next_u32()?;
                read.next_u32()?;
                read.next_u32()?;
            }
            let _editor_tab = read.next_u32()?;
            let _editor_x = read.next_u32()?;
            let _editor_y = read.next_u32()?;
        }

        Ok(())
    }

    read.read_i32(&mut game.last_instance)?;
    read.read_i32(&mut game.last_tile)?;

    // includes

    if game.version == 800 {
        let version = read.next_u32()?;
        if version != 800 {
            return Err(io::Error::from(io::ErrorKind::InvalidData));
        }

        let len = read.next_u32()? as usize;
        for _ in 0..len {
            let read = &mut read.read_blob_zlib(buf)?;
            read_include(read, exe, version, arena)?;
            assert!(read.is_empty());
        }
    }

    fn read_include<'a>(
        read: &mut &[u8], exe: bool, version: u32, arena: &'a Arena
    ) -> io::Result<()> {
        if version == 800 && !exe { let _time = read.next_f64()?; }

        let version = read.next_u32()?;
        if version != 800 {
            return Err(io::Error::from(io::ErrorKind::InvalidData));
        }

        let mut name = &[][..];
        read.read_blob(&mut name, arena)?;

        let mut path = &[][..];
        read.read_blob(&mut path, arena)?;

        let original = read.next_bool()?;
        let _size = read.next_u32()?;
        let in_gmk = read.next_bool()?;
        if original && in_gmk {
            let mut data = &[][..];
            read.read_blob(&mut data, arena)?;
        }

        let _export = read.next_bool()?;

        let mut folder = &[][..];
        read.read_blob(&mut folder, arena)?;

        let _overwrite = read.next_bool()?;
        let _free = read.next_bool()?;
        let _remove = read.next_bool()?;

        Ok(())
    }

    // extensions

    if game.version == 800 && !exe {
        let version = read.next_u32()?;
        if version != 700 {
            return Err(io::Error::from(io::ErrorKind::InvalidData));
        }

        let len = read.next_u32()? as usize;
        game.extensions.reserve(len);
        for id in 0..len {
            game.extensions.push(&[][..]);

            let extension = &mut game.extensions[id];
            read.read_blob(extension, arena)?;
        }
    }

    // game info

    let version = read.next_u32()?;
    if version != 430 && version != 800 {
        return Err(io::Error::from(io::ErrorKind::InvalidData));
    }

    if version == 430 {
        read_info(read, exe, version, arena)?;
    }
    if version == 800 {
        let read = &mut read.read_blob_zlib(buf)?;
        read_info(read, exe, version, arena)?;
        assert!(read.is_empty());
    }

    fn read_info<'a>(
        read: &mut &[u8], exe: bool, version: u32, arena: &'a Arena
    ) -> io::Result<()> {
        let _background = read.next_u32()?;
        let _window = read.next_bool()?;

        if version == 800 {
            let mut caption = &[][..];
            read.read_blob(&mut caption, arena)?;

            let _left = read.next_u32()?;
            let _top = read.next_u32()?;
            let _width = read.next_u32()?;
            let _height = read.next_u32()?;
            let _show_border = read.next_bool()?;
            let _allow_resize = read.next_bool()?;
            let _topmost = read.next_bool()?;
            let _freeze = read.next_bool()?;
        }

        if version == 800 && !exe { let _time = read.next_f64()?; }

        let mut info = &[][..];
        read.read_blob(&mut info, arena)?;

        Ok(())
    }

    // library initialization

    let version = read.next_u32()?;
    if version != 500 {
        return Err(io::Error::from(io::ErrorKind::InvalidData));
    }

    for _ in 0..read.next_u32()? {
        read.skip_blob()?;
    }

    // room order

    let version = read.next_u32()?;
    if version != 500 && version != 700 {
        return Err(io::Error::from(io::ErrorKind::InvalidData));
    }

    for _ in 0..read.next_u32()? {
        game.room_order.push(read.next_u32()?);
    }

    // resource tree

    Ok(())
}

fn read_settings<'a>(
    read: &mut &[u8], exe: bool, version: u32, game: &mut Game<'a>, arena: &'a Arena
) -> io::Result<()> {
    let settings = &mut game.settings;

    read.read_bool(&mut settings.fullscreen)?;
    if version == 800 { read.read_bool(&mut settings.interpolation)?; }
    read.read_bool(&mut settings.hide_border)?;
    read.read_bool(&mut settings.show_cursor)?;
    read.read_i32(&mut settings.scaling)?;
    if version == 530 {
        let _fullscreen_scale = read.next_u32()?;
        let _hardware_scale_only = read.next_bool()?;
    }
    if version == 800 {
        read.read_bool(&mut settings.allow_resize)?;
        read.read_bool(&mut settings.topmost)?;
        read.read_u32(&mut settings.background_color)?;
    }
    read.read_bool(&mut settings.set_resolution)?;
    read.read_u32(&mut settings.color_depth)?;
    if version == 530 {
        let _exclusive = read.next_bool()?;
    }
    read.read_u32(&mut settings.resolution)?;
    read.read_u32(&mut settings.frequency)?;
    if version == 530 {
        read.read_bool(&mut settings.vsync)?;
        let _fullscreen_caption = read.next_bool()?;
    }
    read.read_bool(&mut settings.hide_buttons)?;
    if version == 800 {
        read.read_bool(&mut settings.vsync)?;
        read.read_bool(&mut settings.disable_screensaver)?;
    }
    read.read_bool(&mut settings.default_f4)?;
    read.read_bool(&mut settings.default_f1)?;
    read.read_bool(&mut settings.default_esc)?;
    read.read_bool(&mut settings.default_f5)?;
    if version == 800 {
        read.read_bool(&mut settings.default_f9)?;
        read.read_bool(&mut settings.close_as_esc)?;
    }
    read.read_u32(&mut settings.priority)?;
    if version == 530 {
        read.next_u32()?;
        read.next_u32()?;
    }
    read.read_bool(&mut settings.freeze)?;

    // 0 => no loading bar
    // 1 => default loading bar
    // 2 => own loading bar
    read.read_u32(&mut settings.load_bar)?;
    if (exe && settings.load_bar == 1) || settings.load_bar == 2 {
        let mut back = false;
        if version == 530 { back = read.next_i32()? != -1; }
        if version == 800 { back = read.next_i32()? != -1; }
        if back {
            let mut back = vec![];
            read.read_blob_zlib(&mut back)?;
        }

        let mut front = false;
        if version == 530 { front = read.next_i32()? != -1; }
        if version == 800 { front = read.next_i32()? != -1; }
        if front {
            let mut front = vec![];
            read.read_blob_zlib(&mut front)?;
        }
    }

    read.read_bool(&mut settings.load_image)?;
    if settings.load_image {
        let mut exists = false;
        if version == 530 { exists = read.next_i32()? != -1; }
        if version == 800 { exists = if !exe { read.next_bool()? } else { true }; }
        if exists {
            let mut image = vec![];
            read.read_blob_zlib(&mut image)?;
        }
    }

    read.read_bool(&mut settings.load_transparent)?;
    read.read_u32(&mut settings.load_alpha)?;
    read.read_bool(&mut settings.load_scale)?;

    if version == 530 || (version == 800 && !exe) {
        let mut icon = &[][..];
        read.read_blob(&mut icon, arena)?;
    }

    read.read_bool(&mut settings.error_display)?;
    read.read_bool(&mut settings.error_log)?;
    read.read_bool(&mut settings.error_abort)?;
    read.read_bool(&mut settings.uninitialized_zero)?;

    if version == 530 || (version == 800 && !exe) {
        let mut author = &[][..];
        read.read_blob(&mut author, arena)?;
        if version == 530 {
            let _version = read.next_u32()?;
        }
        if version == 800 {
            let mut version = &[][..];
            read.read_blob(&mut version, arena)?;
        }
        let _time = read.next_f64()?;
        let mut information = &[][..];
        read.read_blob(&mut information, arena)?;

        if version == 530 {
            read_constants(read, game, arena)?;
        }

        if version == 800 {
            let _major = read.next_u32()?;
            let _minor = read.next_u32()?;
            let _release = read.next_u32()?;
            let _build = read.next_u32()?;
            let mut company = &[][..];
            read.read_blob(&mut company, arena)?;
            let mut product = &[][..];
            read.read_blob(&mut product, arena)?;
            let mut copyright = &[][..];
            read.read_blob(&mut copyright, arena)?;
            let mut description = &[][..];
            read.read_blob(&mut description, arena)?;
            let _time = read.next_f64()?;
        }
    }

    Ok(())
}

fn read_constants<'a>(read: &mut &[u8], game: &mut Game<'a>, arena: &'a Arena) -> io::Result<()> {
    let len = read.next_u32()? as usize;
    game.constants.reserve(len);
    for id in 0..len {
        game.constants.push(Constant::default());

        let constant = &mut game.constants[id];
        read.read_blob(&mut constant.name, arena)?;
        read.read_blob(&mut constant.value, arena)?;
    }

    Ok(())
}

fn read_action<'a, R: BufRead>(read: &mut R, action: &mut Action<'a>, arena: &'a Arena) ->
    io::Result<()>
{
    let _version = read.next_u32()?;

    read.read_u32(&mut action.library)?;
    read.read_u32(&mut action.action)?;
    read.read_u32(&mut action.action_kind)?;
    read.read_bool(&mut action.has_relative)?;
    read.read_bool(&mut action.is_question)?;
    read.read_bool(&mut action.has_target)?;
    read.read_u32(&mut action.action_type)?;
    read.read_blob(&mut action.name, arena)?;
    read.read_blob(&mut action.code, arena)?;
    read.read_u32(&mut action.parameters_used)?;

    let len = read.next_u32()?;
    action.parameters.reserve(len as usize);
    for id in 0..len as usize {
        action.parameters.push(u32::default());

        let parameter = &mut action.parameters[id];
        read.read_u32(parameter)?;
    }

    read.read_i32(&mut action.target)?;
    read.read_bool(&mut action.relative)?;

    let len = read.next_u32()?;
    action.arguments.reserve(len as usize);
    for id in 0..len as usize {
        action.arguments.push(&[][..]);

        let argument = &mut action.arguments[id];
        read.read_blob(argument, arena)?;
    }

    read.read_bool(&mut action.negate)?;

    Ok(())
}

pub fn read_gex<'a, R: BufRead>(read: &mut R, extension: &mut Extension<'a>, arena: &'a Arena) ->
    io::Result<()>
{
    if read.next_u32()? != GM_MAGIC {
        return Err(io::Error::from(io::ErrorKind::InvalidData));
    }

    let _version = read.next_u32()?;

    // swap tables

    let seed = read.next_u32()? as usize;
    let a = seed % 250 + 6;
    let b = seed / 250;

    let mut forward = [0u8; 256];
    for i in 0..256 { forward[i] = i as u8; }
    for i in 1..10001 {
        let j = (i * a + b) % 254 + 1;
        forward.swap(j, j + 1);
    }

    let mut reverse = [0u8; 256];
    for i in 0..256 { reverse[forward[i] as usize] = i as u8; }

    // decrypt

    let mut data = Vec::default();
    read.read_to_end(&mut data)?;
    for x in &mut data[..] { *x = reverse[*x as usize]; }

    // extension data

    let read = &mut &data[..];

    read_ged(read, false, extension, arena)?;

    let buf = &mut Vec::default();
    for file in &mut extension.files[..] {
        read.read_blob_zlib(buf)?;
        file.contents = alloc_buf(arena, &buf[..]);
    }

    assert!(read.fill_buf()?.is_empty());
    Ok(())
}

pub fn read_ged<'a>(read: &mut &[u8], exe: bool, extension: &mut Extension<'a>, arena: &'a Arena) ->
    io::Result<()>
{
    let _version = read.next_u32()?;
    if !exe { let _editable = read.next_u32()?; }
    read.read_blob(&mut extension.name, arena)?;
    read.read_blob(&mut extension.folder, arena)?;
    if !exe {
        read.read_blob(&mut extension.version, arena)?;
        read.read_blob(&mut extension.author, arena)?;
        read.read_blob(&mut extension.date, arena)?;
        read.read_blob(&mut extension.license, arena)?;
        read.read_blob(&mut extension.description, arena)?;
        read.read_blob(&mut extension.help_file, arena)?;
        read.read_u32(&mut extension.hidden)?;

        let len = read.next_u32()? as usize;
        for id in 0..len {
            extension.uses.push(&[][..]);

            let line = &mut extension.uses[id];
            read.read_blob(line, arena)?;
        }
    }

    let len = read.next_u32()? as usize;
    for id in 0..len {
        extension.files.push(ExtensionFile::default());

        let file = &mut extension.files[id];
        let _version = read.next_u32()?;
        read.read_blob(&mut file.file_name, arena)?;
        if !exe { read.read_blob(&mut file.original_name, arena)?; }
        read.read_u32(&mut file.kind)?;
        read.read_blob(&mut file.initialization, arena)?;
        read.read_blob(&mut file.finalization, arena)?;

        let len = read.next_u32()? as usize;
        for id in 0..len {
            file.functions.push(ExtensionFunction::default());

            let function = &mut file.functions[id];
            let _version = read.next_u32()?;
            read.read_blob(&mut function.name, arena)?;
            read.read_blob(&mut function.external_name, arena)?;
            read.read_u32(&mut function.calling_convention)?;
            if !exe { read.read_blob(&mut function.help_line, arena)?; }
            read.read_u32(&mut function.hidden)?;
            read.read_u32(&mut function.parameters_used)?;
            for parameter in &mut function.parameters[..] {
                read.read_u32(parameter)?;
            }
            read.read_u32(&mut function.result)?;
        }

        let len = read.next_u32()? as usize;
        for id in 0..len {
            file.constants.push(ExtensionConstant::default());

            let constant = &mut file.constants[id];
            if !exe { let _version = read.next_u32()?; }
            if exe { read.read_u32(&mut constant.hidden)?; }
            read.read_blob(&mut constant.name, arena)?;
            read.read_blob(&mut constant.value, arena)?;
            if !exe { read.read_u32(&mut constant.hidden)?; }
        }
    }

    Ok(())
}

fn alloc_buf<'a>(arena: &'a Arena, buf: &[u8]) -> &'a mut [u8] {
    // Safety: 0 < len < isize::MAX; allocation is checked and initialized before use.
    unsafe {
        let layout = Layout::for_value(buf);
        let data = arena.alloc(layout);
        if data == ptr::null_mut() { handle_alloc_error(layout); }
        ptr::copy_nonoverlapping(buf.as_ptr(), data, buf.len());
        slice::from_raw_parts_mut(data, buf.len())
    }
}

trait GmRead {
    fn read_u32(&mut self, buf: &mut u32) -> io::Result<usize>;
    fn read_i32(&mut self, buf: &mut i32) -> io::Result<usize>;
    fn read_bool(&mut self, buf: &mut bool) -> io::Result<usize>;
    fn read_f64(&mut self, buf: &mut f64) -> io::Result<usize>;

    fn read_blob_mut<'a>(&mut self, buf: &mut &'a mut [u8], arena: &'a Arena) -> io::Result<usize>;
    fn read_blob<'a>(&mut self, buf: &mut &'a [u8], arena: &'a Arena) -> io::Result<usize> {
        let mut blob = &mut [][..];
        let nread = self.read_blob_mut(&mut blob, arena)?;
        *buf = &*blob;
        Ok(nread)
    }

    fn next_u32(&mut self) -> io::Result<u32> {
        let mut buf = 0;
        self.read_u32(&mut buf)?;
        Ok(buf)
    }

    fn next_i32(&mut self) -> io::Result<i32> {
        let mut buf = 0;
        self.read_i32(&mut buf)?;
        Ok(buf)
    }

    fn next_bool(&mut self) -> io::Result<bool> {
        let mut buf = false;
        self.read_bool(&mut buf)?;
        Ok(buf)
    }

    fn next_f64(&mut self) -> io::Result<f64> {
        let mut buf = 0.0;
        self.read_f64(&mut buf)?;
        Ok(buf)
    }
}

impl<R: Read> GmRead for R {
    fn read_u32(&mut self, buf: &mut u32) -> io::Result<usize> {
        let mut bytes = [0u8; 4];
        self.read_exact(&mut bytes)?;
        *buf = u32::from_le_bytes(bytes);
        Ok(bytes.len())
    }

    fn read_i32(&mut self, buf: &mut i32) -> io::Result<usize> {
        let mut bytes = [0u8; 4];
        self.read_exact(&mut bytes)?;
        *buf = i32::from_le_bytes(bytes);
        Ok(bytes.len())
    }

    fn read_bool(&mut self, buf: &mut bool) -> io::Result<usize> {
        let mut value = 0;
        let nread = self.read_u32(&mut value)?;
        *buf = value != 0;
        Ok(nread)
    }

    fn read_f64(&mut self, buf: &mut f64) -> io::Result<usize> {
        let mut bytes = [0u8; 8];
        self.read_exact(&mut bytes)?;
        *buf = f64::from_le_bytes(bytes);
        Ok(bytes.len())
    }

    fn read_blob_mut<'a>(&mut self, buf: &mut &'a mut [u8], arena: &'a Arena) -> io::Result<usize> {
        let mut nread = 0;
        let mut len = 0;
        nread += self.read_u32(&mut len)?;
        if len == 0 {
            *buf = &mut [];
            return Ok(nread);
        }
        // Safety: 0 < len < isize::MAX; allocation is checked and initialized before use.
        *buf = unsafe {
            let layout = Layout::from_size_align_unchecked(len as usize, 1);
            let buf = arena.alloc(layout);
            if buf == ptr::null_mut() { handle_alloc_error(layout); }
            ptr::write_bytes(buf, 0, len as usize);
            slice::from_raw_parts_mut(buf, len as usize)
        };
        self.read_exact(*buf)?;
        nread += len as usize;
        Ok(nread)
    }
}

trait GmBufRead {
    fn read_blob_zlib<'a>(&mut self, buf: &'a mut Vec<u8>) -> io::Result<&'a [u8]>;
}

impl<R: BufRead> GmBufRead for R {
    fn read_blob_zlib<'a>(&mut self, buf: &'a mut Vec<u8>) -> io::Result<&'a [u8]> {
        let len = self.next_u32()?;
        let mut read = ZlibDecoder::new(self.take(len as u64));

        buf.clear();
        let len = read.read_to_end(buf)?;

        if !read.into_inner().fill_buf()?.is_empty() {
            return Err(io::Error::from(io::ErrorKind::InvalidData));
        }
        Ok(&buf[buf.len() - len..])
    }
}

trait GmSeek {
    fn skip_blob(&mut self) -> io::Result<usize>;
}

impl<R: Read + Seek> GmSeek for R {
    fn skip_blob(&mut self) -> io::Result<usize> {
        let mut nread = 0;
        let mut len = 0;
        nread += self.read_u32(&mut len)?;
        self.seek(SeekFrom::Current(len as i64))?;
        nread += len as usize;
        Ok(nread)
    }
}

trait GmSliceExt {
    fn skip_blob(&mut self) -> io::Result<usize>;
}

impl GmSliceExt for &[u8] {
    fn skip_blob(&mut self) -> io::Result<usize> {
        let mut nread = 0;
        let mut len = 0;
        nread += self.read_u32(&mut len)?;
        *self = self.get(len as usize..).ok_or_else(|| io::ErrorKind::UnexpectedEof)?;
        nread += len as usize;
        Ok(nread)
    }
}
