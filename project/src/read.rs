use std::{ptr, slice};
use std::mem::MaybeUninit;
use std::alloc::{Layout, handle_alloc_error};
use std::io::{self, Read, Seek, SeekFrom, BufRead, BufReader, Cursor};
use flate2::bufread::ZlibDecoder;
use quickdry::Arena;

use crate::{
    Game, Settings, Constant,
    Sound,
    Sprite, Frame, SpriteMaskShape,
    Background,
    Path, Point,
    Script,
    Object, Action, Event,
    Room, RoomBackground, View, Instance, Tile
};

const GMK_MAGIC: u32 = 1234321;

pub fn read_gmk<'a, R: Read>(read: &mut R, game: &mut Game<'a>, arena: &'a Arena) ->
    io::Result<()>
{
    let mut buf = Vec::default();
    read.read_to_end(&mut buf)?;
    let read = &mut Cursor::new(buf);

    if read.next_u32()? != GMK_MAGIC {
        return Err(io::Error::from(io::ErrorKind::InvalidData));
    }
    read.read_u32(&mut game.version)?;

    read.read_u32(&mut game.id)?;
    read.read_u32(&mut game.guid[0])?;
    read.read_u32(&mut game.guid[1])?;
    read.read_u32(&mut game.guid[2])?;
    read.read_u32(&mut game.guid[3])?;

    read_settings(read, &mut game.settings, arena)?;

    // triggers

    let _version = read.next_u32()?;
    let _len = read.next_u32()?;
    assert_eq!(_len, 0);
    let _time = read.next_f64()?;

    // constants

    let _version = read.next_u32()?;
    let len = read.next_u32()? as usize;
    game.constants.reserve(len);
    for id in 0..len {
        game.constants.push(Constant::default());

        let constant = &mut game.constants[id];
        read.read_blob(&mut constant.name, arena)?;
        read.read_blob(&mut constant.value, arena)?;
    }
    let _time = read.next_f64()?;

    // sounds

    let _version = read.next_u32()?;
    let len = read.next_u32()? as usize;
    game.sounds.reserve(len);
    for id in 0..len {
        game.sounds.push(Sound::default());

        let read = &mut read.next_zlib()?;
        if !read.next_bool()? {
            assert_eq!(read.get_mut().get_mut().limit(), 0);
            continue;
        }

        let sound = &mut game.sounds[id];
        read.read_blob(&mut sound.name, arena)?;
        let _time = read.next_f64()?;
        let _version = read.next_u32()?;
        read.read_u32(&mut sound.kind)?;
        read.read_blob(&mut sound.file_type, arena)?;
        read.read_blob(&mut sound.file_name, arena)?;
        if read.next_bool()? {
            read.read_blob(&mut sound.data, arena)?;
        }
        read.read_u32(&mut sound.effects)?;
        read.read_f64(&mut sound.volume)?;
        read.read_f64(&mut sound.pan)?;
        read.read_bool(&mut sound.preload)?;

        assert_eq!(read.get_mut().get_mut().limit(), 0);
    }

    // sprites

    let _version = read.next_u32()?;
    let len = read.next_u32()? as usize;
    game.sprites.reserve(len);
    for id in 0..len {
        game.sprites.push(Sprite::default());

        let read = &mut read.next_zlib()?;
        if !read.next_bool()? {
            assert_eq!(read.get_mut().get_mut().limit(), 0);
            continue;
        }

        let sprite = &mut game.sprites[id];
        read.read_blob(&mut sprite.name, arena)?;
        let _time = read.next_f64()?;
        let _version = read.next_u32()?;
        read.read_u32(&mut sprite.origin.0)?;
        read.read_u32(&mut sprite.origin.1)?;

        let len = read.next_u32()? as usize;
        sprite.frames.reserve(len);
        for i in 0..len {
            sprite.frames.push(Frame::default());

            let frame = &mut sprite.frames[i];
            let _version = read.next_u32()?;
            read.read_u32(&mut frame.size.0)?;
            read.read_u32(&mut frame.size.1)?;
            read.read_blob(&mut frame.data, arena)?;
        }

        if len > 0 {
            sprite.mask_shape = match read.next_u32()? {
                0 => SpriteMaskShape::Precise,
                1 => SpriteMaskShape::Rectangle,
                2 => SpriteMaskShape::Disk,
                3 => SpriteMaskShape::Diamond,
                _ => panic!(),
            };
            read.read_u32(&mut sprite.mask_alpha_tolerance)?;
            read.read_bool(&mut sprite.separate_masks)?;
            read.read_u32(&mut sprite.mask_bounds.0)?;
            read.read_u32(&mut sprite.mask_bounds.1)?;
            read.read_u32(&mut sprite.mask_bounds.2)?;
            read.read_u32(&mut sprite.mask_bounds.3)?;
        }

        assert_eq!(read.get_mut().get_mut().limit(), 0);
    }

    // backgrounds

    let _version = read.next_u32()?;
    let len = read.next_u32()? as usize;
    game.backgrounds.reserve(len);
    for id in 0..len {
        game.backgrounds.push(Background::default());

        let read = &mut read.next_zlib()?;
        if !read.next_bool()? {
            assert_eq!(read.get_mut().get_mut().limit(), 0);
            continue;
        }

        let background = &mut game.backgrounds[id];
        read.read_blob(&mut background.name, arena)?;
        let _time = read.next_f64()?;
        let _version = read.next_u32()?;
        let _tileset = read.next_bool()?;
        let _tile_width = read.next_u32()?;
        let _tile_height = read.next_u32()?;
        let _tile_off_x = read.next_u32()?;
        let _tile_off_y = read.next_u32()?;
        let _tile_sep_x = read.next_u32()?;
        let _tile_sep_y = read.next_u32()?;
        let _version = read.next_u32()?;
        read.read_u32(&mut background.size.0)?;
        read.read_u32(&mut background.size.1)?;
        if background.size.0 > 0 && background.size.1 > 0 {
            read.read_blob(&mut background.data, arena)?;
        }

        assert_eq!(read.get_mut().get_mut().limit(), 0);
    }

    // paths

    let _version = read.next_u32()?;
    let len = read.next_u32()? as usize;
    game.paths.reserve(len);
    for id in 0..len {
        game.paths.push(Path::default());

        let read = &mut read.next_zlib()?;
        if !read.next_bool()? {
            assert_eq!(read.get_mut().get_mut().limit(), 0);
            continue;
        }

        let path = &mut game.paths[id];
        read.read_blob(&mut path.name, arena)?;
        let _time = read.next_f64()?;
        let _version = read.next_u32()?;
        read.read_u32(&mut path.kind)?;
        read.read_bool(&mut path.closed)?;
        read.read_u32(&mut path.precision)?;

        let len = read.next_u32()? as usize;
        path.points.reserve(len);
        for i in 0..len {
            path.points.push(Point::default());

            let point = &mut path.points[i];
            read.read_f64(&mut point.position.0)?;
            read.read_f64(&mut point.position.1)?;
            read.read_f64(&mut point.speed)?;
        }

        assert_eq!(read.get_mut().get_mut().limit(), 0);
    }

    // scripts

    let _version = read.next_u32()?;
    let len = read.next_u32()? as usize;
    game.scripts.reserve(len);
    for id in 0..len {
        game.scripts.push(Script::default());

        let read = &mut read.next_zlib()?;
        if !read.next_bool()? {
            assert_eq!(read.get_mut().get_mut().limit(), 0);
            continue;
        }

        let script = &mut game.scripts[id];
        read.read_blob(&mut script.name, arena)?;
        let _time = read.next_f64()?;
        let _version = read.next_u32()?;
        read.read_blob(&mut script.body, arena)?;

        assert_eq!(read.get_mut().get_mut().limit(), 0);
    }

    // fonts

    let _version = read.next_u32()?;
    for _id in 0..read.next_u32()? {
        let read = &mut read.next_zlib()?;
        if !read.next_bool()? {
            assert_eq!(read.get_mut().get_mut().limit(), 0);
            continue;
        }

        let mut name = &[][..];
        read.read_blob(&mut name, arena)?;

        let _time = read.next_f64()?;
        let _version = read.next_u32()?;

        let mut font = &[][..];
        read.read_blob(&mut font, arena)?;

        let _size = read.next_u32()?;
        let _bold = read.next_bool()?;
        let _italic = read.next_bool()?;
        let _start = read.next_u32()?;
        let _end = read.next_u32()?;

        assert_eq!(read.get_mut().get_mut().limit(), 0);
    }

    // timelines

    let _version = read.next_u32()?;
    for _id in 0..read.next_u32()? {
        let read = &mut read.next_zlib()?;
        if !read.next_bool()? {
            assert_eq!(read.get_mut().get_mut().limit(), 0);
            continue;
        }

        let mut name = &[][..];
        read.read_blob(&mut name, arena)?;

        let _time = read.next_f64()?;
        let _version = read.next_u32()?;

        for _ in 0..read.next_u32()? {
            let _time = read.next_u32()?;

            for _ in 0..read.next_u32()? {
                let mut action = Action::default();
                read_action(read, &mut action, arena)?;
            }
        }

        assert_eq!(read.get_mut().get_mut().limit(), 0);
    }

    // objects

    let _version = read.next_u32()?;
    let len = read.next_u32()? as usize;
    game.objects.reserve(len);
    for id in 0..len {
        game.objects.push(Object::default());

        let read = &mut read.next_zlib()?;
        if !read.next_bool()? {
            assert_eq!(read.get_mut().get_mut().limit(), 0);
            continue;
        }

        let object = &mut game.objects[id];
        read.read_blob(&mut object.name, arena)?;

        let _time = read.next_f64()?;
        let _version = read.next_u32()?;
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

                let _version = read.next_u32()?;
                let len = read.next_u32()? as usize;
                event.actions.reserve(len);
                for id in 0..len {
                    event.actions.push(Action::default());

                    let action = &mut event.actions[id];
                    read_action(read, action, arena)?;
                }
            }
        }

        assert_eq!(read.get_mut().get_mut().limit(), 0);
    }

    // rooms

    let _version = read.next_u32()?;
    let len = read.next_u32()? as usize;
    game.rooms.reserve(len);
    for id in 0..len {
        game.rooms.push(Room::default());

        let read = &mut read.next_zlib()?;
        if !read.next_bool()? {
            assert_eq!(read.get_mut().get_mut().limit(), 0);
            continue;
        }

        let room = &mut game.rooms[id];
        read.read_blob(&mut room.name, arena)?;
        let _time = read.next_f64()?;
        let _version = read.next_u32()?;
        read.read_blob(&mut room.caption, arena)?;

        read.read_u32(&mut room.width)?;
        read.read_u32(&mut room.height)?;
        let _snap_x = read.next_u32()?;
        let _snap_y = read.next_u32()?;
        let _isometric = read.next_bool()?;
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
            read.read_bool(&mut background.tile_h)?;
            read.read_bool(&mut background.tile_v)?;
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
            read.read_u32(&mut view.port_w)?;
            read.read_u32(&mut view.port_h)?;
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
            let _locked = read.next_bool()?;
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
            let _locked = read.next_bool()?;
        }

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
        let _editor_tab = read.next_u32()?;
        let _editor_x = read.next_u32()?;
        let _editor_y = read.next_u32()?;

        assert_eq!(read.get_mut().get_mut().limit(), 0);
    }

    read.read_i32(&mut game.last_instance)?;
    read.read_i32(&mut game.last_tile)?;

    // includes

    let _version = read.next_u32()?;
    for _id in 0..read.next_u32()? {
        let read = &mut read.next_zlib()?;

        let _time = read.next_f64()?;
        let _version = read.next_u32()?;

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

        assert_eq!(read.get_mut().get_mut().limit(), 0);
    }

    // extensions

    let _version = read.next_u32()?;
    let len = read.next_u32()? as usize;
    game.extensions.reserve(len);
    for _id in 0..len {
        let mut name: &[u8] = &[];
        read.read_blob(&mut name, arena)?;
        game.extensions.push(name);
    }

    // game info

    let _version = read.next_u32()?;
    {
        let read = &mut read.next_zlib()?;

        let _background = read.next_u32()?;
        let _window = read.next_bool()?;

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

        let _time = read.next_f64()?;

        let mut info = &[][..];
        read.read_blob(&mut info, arena)?;

        assert_eq!(read.get_mut().get_mut().limit(), 0);
    }

    // library initialization

    let _version = read.next_u32()?;
    for _ in 0..read.next_u32()? {
        read.skip_blob()?;
    }

    // room order

    let _version = read.next_u32()?;
    for _ in 0..read.next_u32()? {
        let _room = read.next_u32()?;
    }

    // resource tree

    Ok(())
}

fn read_settings<'a, R: BufRead>(read: &mut R, settings: &mut Settings, arena: &'a Arena) ->
    io::Result<()>
{
    let _version = read.next_u32()?;
    {
        let read = &mut read.next_zlib()?;

        read.read_bool(&mut settings.fullscreen)?;
        read.read_bool(&mut settings.interpolation)?;
        read.read_bool(&mut settings.hide_border)?;
        read.read_bool(&mut settings.show_cursor)?;
        read.read_i32(&mut settings.scaling)?;
        read.read_bool(&mut settings.allow_resize)?;
        read.read_bool(&mut settings.topmost)?;
        read.read_u32(&mut settings.background_color)?;
        read.read_bool(&mut settings.set_resolution)?;
        read.read_u32(&mut settings.color_depth)?;
        read.read_u32(&mut settings.resolution)?;
        read.read_u32(&mut settings.frequency)?;
        read.read_bool(&mut settings.hide_buttons)?;
        read.read_bool(&mut settings.vsync)?;
        read.read_bool(&mut settings.disable_screensaver)?;
        read.read_bool(&mut settings.default_f4)?;
        read.read_bool(&mut settings.default_f1)?;
        read.read_bool(&mut settings.default_esc)?;
        read.read_bool(&mut settings.default_f5)?;
        read.read_bool(&mut settings.default_f9)?;
        read.read_bool(&mut settings.close_as_esc)?;
        read.read_u32(&mut settings.priority)?;
        read.read_bool(&mut settings.freeze)?;

        // 0 => no loading bar
        // 1 => default loading bar
        // 2 => own loading bar
        read.read_u32(&mut settings.load_bar)?;

        if settings.load_bar == 2 {
            if read.next_bool()? {
                let mut back = vec![];
                read.read_blob_zlib(&mut back)?;
            }

            if read.next_bool()? {
                let mut front = vec![];
                read.read_blob_zlib(&mut front)?;
            }
        }

        read.read_bool(&mut settings.load_image)?;
        if settings.load_image {
            let exists = read.next_bool()?;
            if exists {
                let mut image = vec![];
                read.read_blob_zlib(&mut image)?;
            }
        }

        read.read_bool(&mut settings.load_transparent)?;
        read.read_u32(&mut settings.load_alpha)?;

        let icon_exists = read.next_bool()?;
        if icon_exists {
            let mut icon: &[u8] = &[];
            read.read_blob(&mut icon, arena)?;
        }

        read.read_bool(&mut settings.load_scale)?;
        read.read_bool(&mut settings.error_display)?;
        read.read_bool(&mut settings.error_log)?;
        read.read_bool(&mut settings.error_abort)?;
        read.read_u32(&mut settings.uninitialized)?;

        assert_eq!(read.get_mut().get_mut().limit(), 0);
    }

    Ok(())
}

fn read_action<'a, R: BufRead>(read: &mut R, action: &mut Action<'a>, arena: &'a Arena) ->
    io::Result<usize>
{
    let mut nread = 0;

    let mut version = 0;
    nread += read.read_u32(&mut version)?;

    nread += read.read_u32(&mut action.library)?;
    nread += read.read_u32(&mut action.action)?;
    nread += read.read_u32(&mut action.action_kind)?;
    nread += read.read_bool(&mut action.has_relative)?;
    nread += read.read_bool(&mut action.is_question)?;
    nread += read.read_bool(&mut action.has_target)?;
    nread += read.read_u32(&mut action.action_type)?;
    nread += read.read_blob(&mut action.name, arena)?;
    nread += read.read_blob(&mut action.code, arena)?;
    nread += read.read_u32(&mut action.parameters_used)?;

    let mut len = 0;
    nread += read.read_u32(&mut len)?;
    action.parameters.reserve(len as usize);
    for id in 0..len as usize {
        action.parameters.push(u32::default());

        let parameter = &mut action.parameters[id];
        nread += read.read_u32(parameter)?;
    }

    nread += read.read_i32(&mut action.target)?;
    nread += read.read_bool(&mut action.relative)?;

    let mut len = 0;
    read.read_u32(&mut len)?;
    action.arguments.reserve(len as usize);
    for id in 0..len as usize {
        action.arguments.push(&[][..]);

        let argument = &mut action.arguments[id];
        nread += read.read_blob(argument, arena)?;
    }

    nread += read.read_bool(&mut action.negate)?;

    Ok(nread)
}

trait GmkRead: Sized {
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

    unsafe fn read_zlib<'a, 'b>(
        &'a mut self, buf: *mut BufReader<ZlibDecoder<io::Take<&'b mut Self>>>
    ) -> io::Result<usize> where 'a: 'b;
    fn read_blob_zlib(&mut self, buf: &mut Vec<u8>) -> io::Result<usize>;

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

    fn next_zlib(&mut self) -> io::Result<BufReader<ZlibDecoder<io::Take<&mut Self>>>> {
        unsafe {
            let mut buf = MaybeUninit::uninit();
            self.read_zlib(buf.as_mut_ptr())?;
            Ok(buf.assume_init())
        }
    }
}

trait GmkSeek: Sized {
    fn skip_blob(&mut self) -> io::Result<usize>;
}

impl<R: BufRead> GmkRead for R {
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
        assert!(value == 0 || value == 1, "\"boolean\": {}", value);
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

    unsafe fn read_zlib<'a, 'b>(
        &'a mut self, buf: *mut BufReader<ZlibDecoder<io::Take<&'b mut Self>>>
    ) -> io::Result<usize> where 'a: 'b {
        let mut nread = 0;
        let mut len = 0;
        nread += self.read_u32(&mut len)?;
        buf.write(BufReader::new(ZlibDecoder::new(self.take(len as u64))));
        Ok(nread)
    }

    fn read_blob_zlib(&mut self, buf: &mut Vec<u8>) -> io::Result<usize> {
        let mut nread = 0;
        let mut len = 0;
        nread += self.read_u32(&mut len)?;
        let read = &mut ZlibDecoder::new(self.take(len as u64));
        nread += read.read_to_end(buf)?;
        if read.get_mut().limit() > 0 {
            return Err(io::Error::from(io::ErrorKind::InvalidData));
        }
        Ok(nread)
    }
}

impl<R: BufRead + Seek> GmkSeek for R {
    fn skip_blob(&mut self) -> io::Result<usize> {
        let mut nread = 0;
        let mut len = 0;
        nread += self.read_u32(&mut len)?;
        self.seek(SeekFrom::Current(len as i64))?;
        nread += len as usize;
        Ok(nread)
    }
}
