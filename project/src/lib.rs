#[derive(Default)]
pub struct Event<'a> {
    pub event_type: u32,
    pub event_kind: i32,
    pub actions: Vec<Action<'a>>,
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
