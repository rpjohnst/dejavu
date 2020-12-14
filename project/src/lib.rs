pub struct Game<'a> {
    pub sprites: Vec<Sprite<'a>>,
    pub scripts: Vec<Script<'a>>,
    pub objects: Vec<Object<'a>>,
    pub rooms: Vec<Room<'a>>,

    pub last_instance: i32,
}

#[derive(Default)]
pub struct Sprite<'a> {
    pub name: &'a [u8],
    pub origin: (u32, u32),
    pub frames: Vec<Frame<'a>>,
    pub masks: Vec<Mask>,
}

#[derive(Default)]
pub struct Frame<'a> {
    pub size: (u32, u32),
    pub data: &'a [u8],
}

#[derive(Default, Debug)]
pub struct Mask {
}

#[derive(Default)]
pub struct Script<'a> {
    pub name: &'a [u8],
    pub body: &'a [u8],
}

#[derive(Default)]
pub struct Object<'a> {
    pub name: &'a [u8],
    pub sprite: i32,
    pub persistent: bool,
    pub events: Vec<Event<'a>>,
}

#[derive(Default)]
pub struct Event<'a> {
    pub event_type: u32,
    pub event_kind: i32,
    pub actions: Vec<Action<'a>>,
}

pub mod event_type {
    pub const CREATE: u32 = 0;
    pub const DESTROY: u32 = 1;
}

#[derive(Default)]
pub struct Action<'a> {
    pub library: u32,
    pub action: u32,
    pub action_kind: u32,
    pub has_relative: bool,
    pub is_question: bool,
    pub has_target: bool,
    pub action_type: u32,
    pub name: &'a [u8],
    pub code: &'a [u8],
    pub parameters_used: u32,
    pub parameters: Vec<u32>,
    pub target: i32,
    pub relative: bool,
    pub arguments: Vec<&'a [u8]>,
    pub negate: bool,
}

pub mod action_kind {
    pub const NORMAL: u32 = 0;
    pub const BEGIN: u32 = 1;
    pub const END: u32 = 2;
    pub const ELSE: u32 = 3;
    pub const EXIT: u32 = 4;
    pub const REPEAT: u32 = 5;
    pub const VARIABLE: u32 = 6;
    pub const CODE: u32 = 7;
    pub const PLACEHOLDER: u32 = 8;
    pub const SEPARATOR: u32 = 9;
    pub const LABEL: u32 = 10;
}

pub mod action_type {
    pub const NONE: u32 = 0;
    pub const FUNCTION: u32 = 1;
    pub const CODE: u32 = 2;
}

pub mod argument_type {
    pub const EXPR: u32 = 0;
    pub const STRING: u32 = 1;
    pub const BOTH: u32 = 2;
    pub const BOOL: u32 = 3;
    pub const MENU: u32 = 4;
    pub const SPRITE: u32 = 5;
    pub const SOUND: u32 = 6;
    pub const BACKGROUND: u32 = 7;
    pub const PATH: u32 = 8;
    pub const SCRIPT: u32 = 9;
    pub const OBJECT: u32 = 10;
    pub const ROOM: u32 = 11;
    pub const FONT: u32 = 12;
    pub const COLOR: u32 = 13;
    pub const TIMELINE: u32 = 14;
    pub const FONT_STRING: u32 = 15;
}

#[derive(Default)]
pub struct Room<'a> {
    pub name: &'a [u8],

    pub code: &'a [u8],

    pub instances: Vec<Instance<'a>>,
}

#[derive(Default)]
pub struct Instance<'a> {
    pub x: i32,
    pub y: i32,
    pub object_index: i32,
    pub id: i32,
    pub code: &'a [u8],
}

impl<'a> Default for Game<'a> {
    fn default() -> Game<'a> {
        Game {
            sprites: Vec::default(),
            scripts: Vec::default(),
            objects: Vec::default(),
            rooms: Vec::default(),

            last_instance: 100000,
        }
    }
}
