use bstr::BStr;

#[cfg(feature = "wasm")]
use wasm::Reflect;

#[cfg(feature = "read")]
pub use read::{read_project, read_exe, read_ged, read_gex, read_bmp};

#[cfg(feature = "read")]
mod read;

#[cfg_attr(feature = "wasm", derive(Reflect))]
pub struct Game<'a> {
    pub version: u32,
    pub debug: u32,

    pub settings: Settings,

    pub pro: bool,
    pub id: u32,
    pub guid: [u32; 4],

    pub constants: Vec<Constant<'a>>,

    pub sounds: Vec<Sound<'a>>,
    pub sprites: Vec<Sprite<'a>>,
    pub backgrounds: Vec<Background<'a>>,
    pub paths: Vec<Path<'a>>,
    pub scripts: Vec<Script<'a>>,
    pub objects: Vec<Object<'a>>,
    pub rooms: Vec<Room<'a>>,

    pub last_instance: i32,
    pub last_tile: i32,

    pub extensions: Vec<&'a BStr>,

    pub room_order: Vec<u32>,
}

#[cfg_attr(feature = "wasm", derive(Reflect))]
pub struct Settings {
    pub fullscreen: bool,
    pub scaling: i32,
    pub interpolation: bool,
    pub background_color: u32,
    pub allow_resize: bool,
    pub topmost: bool,
    pub hide_border: bool,
    pub hide_buttons: bool,
    pub show_cursor: bool,
    pub freeze: bool,
    pub disable_screensaver: bool,

    pub set_resolution: bool,
    pub color_depth: u32,
    pub resolution: u32,
    pub frequency: u32,
    pub vsync: bool,

    pub default_esc: bool,
    pub close_as_esc: bool,
    pub default_f1: bool,
    pub default_f4: bool,
    pub default_f5: bool,
    pub default_f9: bool,
    pub priority: u32,

    pub load_image: bool,
    pub load_transparent: bool,
    pub load_alpha: u32,
    pub load_bar: u32,
    pub load_scale: bool,

    pub error_display: bool,
    pub error_log: bool,
    pub error_abort: bool,
    pub uninitialized_zero: bool,
}

#[derive(Default)]
#[cfg_attr(feature = "wasm", derive(Reflect))]
pub struct Constant<'a> {
    pub name: &'a BStr,
    pub value: &'a BStr,
}

#[derive(Default)]
#[cfg_attr(feature = "wasm", derive(Reflect))]
pub struct Sound<'a> {
    pub name: &'a BStr,
    pub kind: u32,
    pub file_type: &'a BStr,
    pub file_name: &'a BStr,
    pub data: &'a [u8],
    pub effects: u32,
    pub volume: f64,
    pub pan: f64,
    pub preload: bool,
}

#[derive(Default)]
#[cfg_attr(feature = "wasm", derive(Reflect))]
pub struct Sprite<'a> {
    pub name: &'a BStr,
    pub version: u32,

    pub size: (u32, u32),
    pub transparent: bool,
    pub precise: bool,
    pub use_vram: bool,
    pub lazy_load: bool,

    pub origin: (u32, u32),
    pub images: Vec<Image<'a>>,

    pub shape: u32,
    pub alpha_tolerance: u32,
    pub separate_collision: bool,
    pub bounds_kind: u32,
    pub bounds: Bounds,

    pub masks: Vec<Mask>,
}

#[derive(Default)]
#[cfg_attr(feature = "wasm", derive(Reflect))]
pub struct Image<'a> {
    pub size: (u32, u32),
    pub data: &'a [u8],
}

pub mod shape {
    pub const PRECISE: u32 = 0;
    pub const RECTANGLE: u32 = 1;
    pub const DISK: u32 = 2;
    pub const DIAMOND: u32 = 3;
}

pub mod bounds_kind {
    pub const AUTOMATIC: u32 = 0;
    pub const FULL: u32 = 1;
    pub const MANUAL: u32 = 2;
}

#[derive(Default)]
#[cfg_attr(feature = "wasm", derive(Reflect))]
pub struct Bounds {
    pub left: i32,
    pub right: i32,
    pub bottom: i32,
    pub top: i32,
}

#[derive(Default)]
#[cfg_attr(feature = "wasm", derive(Reflect))]
pub struct Mask {
    pub size: (u32, u32),
    pub bounds: Bounds,
    pub data: Vec<u32>,
}

#[derive(Default)]
#[cfg_attr(feature = "wasm", derive(Reflect))]
pub struct Background<'a> {
    pub name: &'a BStr,
    pub version: u32,

    pub size: (u32, u32),
    pub transparent: bool,
    pub use_vram: bool,
    pub lazy_load: bool,
    pub data: &'a [u8],
}

#[derive(Default)]
#[cfg_attr(feature = "wasm", derive(Reflect))]
pub struct Path<'a> {
    pub name: &'a BStr,
    pub smooth: bool,
    pub closed: bool,
    pub precision: u32,
    pub points: Vec<Point>,
}

#[derive(Default)]
#[cfg_attr(feature = "wasm", derive(Reflect))]
pub struct Point {
    pub position: (f64, f64),
    pub speed: f64,
}

#[derive(Default)]
#[cfg_attr(feature = "wasm", derive(Reflect))]
pub struct Script<'a> {
    pub name: &'a BStr,
    pub body: &'a BStr,
}

#[derive(Default)]
#[cfg_attr(feature = "wasm", derive(Reflect))]
pub struct Object<'a> {
    pub name: &'a BStr,
    pub sprite: i32,
    pub solid: bool,
    pub visible: bool,
    pub depth: i32,
    pub persistent: bool,
    pub parent: i32,
    pub mask: i32,
    pub events: Vec<Event<'a>>,
}

#[derive(Default)]
#[cfg_attr(feature = "wasm", derive(Reflect))]
pub struct Event<'a> {
    pub event_type: u32,
    pub event_kind: i32,
    pub actions: Vec<Action<'a>>,
}

pub mod event_type {
    pub const CREATE: u32 = 0;
    pub const DESTROY: u32 = 1;
    pub const ALARM: u32 = 2;
    pub const STEP: u32 = 3;
    pub const COLLISION: u32 = 4;
    pub const KEYBOARD: u32 = 5;
    pub const MOUSE: u32 = 6;
    pub const OTHER: u32 = 7;
    pub const DRAW: u32 = 8;
    pub const KEY_PRESS: u32 = 9;
    pub const KEY_RELEASE: u32 = 10;
    pub const TRIGGER: u32 = 11;
}

pub mod event_kind {
    // Step events
    pub const STEP: i32 = 0;
    pub const BEGIN_STEP: i32 = 1;
    pub const END_STEP: i32 = 2;

    // Mouse events
    pub const LEFT_BUTTON: i32 = 0;
    pub const RIGHT_BUTTON: i32 = 1;
    pub const MIDDLE_BUTTON: i32 = 2;
    pub const NO_BUTTON: i32 = 3;
    pub const LEFT_PRESS: i32 = 4;
    pub const RIGHT_PRESS: i32 = 5;
    pub const MIDDLE_PRESS: i32 = 6;
    pub const LEFT_RELEASE: i32 = 7;
    pub const RIGHT_RELEASE: i32 = 8;
    pub const MIDDLE_RELEASE: i32 = 9;
    pub const MOUSE_ENTER: i32 = 10;
    pub const MOUSE_LEAVE: i32 = 11;
    pub const GLOBAL_LEFT_BUTTON: i32 = 50;
    pub const GLOBAL_RIGHT_BUTTON: i32 = 51;
    pub const GLOBAL_MIDDLE_BUTTON: i32 = 52;
    pub const GLOBAL_LEFT_PRESS: i32 = 53;
    pub const GLOBAL_RIGHT_PRESS: i32 = 54;
    pub const GLOBAL_MIDDLE_PRESS: i32 = 55;
    pub const GLOBAL_LEFT_RELEASE: i32 = 56;
    pub const GLOBAL_RIGHT_RELEASE: i32 = 57;
    pub const GLOBAL_MIDDLE_RELEASE: i32 = 58;
    pub const MOUSE_WHEEL_UP: i32 = 60;
    pub const MOUSE_WHEEL_DOWN: i32 = 61;

    // Other events
    pub const OUTSIDE_ROOM: i32 = 0;
    pub const INTERSECT_BOUNDARY: i32 = 1;
    pub const GAME_START: i32 = 2;
    pub const GAME_END: i32 = 3;
    pub const ROOM_START: i32 = 4;
    pub const ROOM_END: i32 = 5;
    pub const NO_MORE_LIVES: i32 = 6;
    pub const ANIMATION_END: i32 = 7;
    pub const PATH_END: i32 = 8;
    pub const NO_MORE_HEALTH: i32 = 9;
    pub const USER: [i32; 16] = [10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25];
    pub const CLOSE_BUTTON: i32 = 30;
    pub const OUTSIDE_VIEW: [i32; 8] = [40, 41, 42, 43, 44, 45, 46, 47];
    pub const BOUNDARY_VIEW: [i32; 8] = [50, 51, 52, 53, 54, 55, 56, 57];

    // Draw events
    pub const DRAW: i32 = 0;
    pub const DRAW_GUI: i32 = 64;
    pub const DRAW_RESIZE: i32 = 65;
}

#[derive(Default)]
#[cfg_attr(feature = "wasm", derive(Reflect))]
pub struct Action<'a> {
    pub library: u32,
    pub action: u32,
    pub action_kind: u32,
    pub has_relative: bool,
    pub is_question: bool,
    pub has_target: bool,
    pub action_type: u32,
    pub name: &'a BStr,
    pub code: &'a BStr,
    pub parameters_used: u32,
    pub parameters: Vec<u32>,
    pub target: i32,
    pub relative: bool,
    pub arguments: Vec<&'a BStr>,
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
#[cfg_attr(feature = "wasm", derive(Reflect))]
pub struct Room<'a> {
    pub name: &'a BStr,
    pub caption: &'a BStr,

    pub width: u32,
    pub height: u32,
    pub speed: u32,
    pub persistent: bool,
    pub clear_color: u32,
    pub clear: bool,

    pub code: &'a BStr,

    pub backgrounds: Vec<RoomBackground>,

    pub enable_views: bool,
    pub views: Vec<View>,

    pub instances: Vec<Instance<'a>>,
    pub tiles: Vec<Tile>,
}

#[derive(Default)]
#[cfg_attr(feature = "wasm", derive(Reflect))]
pub struct RoomBackground {
    pub visible: bool,
    pub foreground: bool,
    pub background: i32,
    pub x: i32,
    pub y: i32,
    pub htiled: bool,
    pub vtiled: bool,
    pub hspeed: i32,
    pub vspeed: i32,
    pub stretch: bool,
}

#[derive(Default)]
#[cfg_attr(feature = "wasm", derive(Reflect))]
pub struct View {
    pub visible: bool,
    pub view_x: i32,
    pub view_y: i32,
    pub view_w: u32,
    pub view_h: u32,
    pub port_x: i32,
    pub port_y: i32,
    pub port_w: u32,
    pub port_h: u32,
    pub h_border: i32,
    pub v_border: i32,
    pub h_speed: i32,
    pub v_speed: i32,
    pub target: i32,
}

#[derive(Default)]
#[cfg_attr(feature = "wasm", derive(Reflect))]
pub struct Instance<'a> {
    pub x: i32,
    pub y: i32,
    pub object_index: i32,
    pub id: i32,
    pub code: &'a BStr,
}

#[derive(Default)]
#[cfg_attr(feature = "wasm", derive(Reflect))]
pub struct Tile {
    pub x: i32,
    pub y: i32,
    pub background: i32,
    pub tile_x: i32,
    pub tile_y: i32,
    pub width: u32,
    pub height: u32,
    pub depth: i32,
    pub id: i32,
}

#[derive(Default)]
#[cfg_attr(feature = "wasm", derive(Reflect))]
pub struct Extension<'a> {
    pub name: &'a BStr,
    pub folder: &'a BStr,
    pub version: &'a BStr,
    pub author: &'a BStr,
    pub date: &'a BStr,
    pub license: &'a BStr,
    pub description: &'a BStr,
    pub help_file: &'a BStr,
    pub hidden: u32,
    pub uses: Vec<&'a BStr>,
    pub files: Vec<ExtensionFile<'a>>,
}

#[derive(Default)]
#[cfg_attr(feature = "wasm", derive(Reflect))]
pub struct ExtensionFile<'a> {
    pub file_name: &'a BStr,
    pub original_name: &'a BStr,
    pub kind: u32,
    pub initialization: &'a BStr,
    pub finalization: &'a BStr,
    pub functions: Vec<ExtensionFunction<'a>>,
    pub constants: Vec<ExtensionConstant<'a>>,
    pub contents: &'a [u8],
}

pub mod extension_kind {
    pub const DLL: u32 = 1;
    pub const GML: u32 = 2;
    pub const LIB: u32 = 3;
    pub const OTHER: u32 = 4;
}

#[derive(Default)]
#[cfg_attr(feature = "wasm", derive(Reflect))]
pub struct ExtensionFunction<'a> {
    pub name: &'a BStr,
    pub external_name: &'a BStr,
    pub calling_convention: u32,
    pub help_line: &'a BStr,
    pub hidden: u32,
    pub parameters_used: u32,
    pub parameters: [u32; 17],
    pub result: u32,
}

pub mod calling_convention {
    pub const GML: u32 = 2;
    pub const STDCALL: u32 = 11;
    pub const CDECL: u32 = 12;
}

pub mod parameter_type {
    pub const STRING: u32 = 1;
    pub const REAL: u32 = 2;
}

#[derive(Default)]
#[cfg_attr(feature = "wasm", derive(Reflect))]
pub struct ExtensionConstant<'a> {
    pub name: &'a BStr,
    pub value: &'a BStr,
    pub hidden: u32,
}

impl<'a> Default for Game<'a> {
    fn default() -> Game<'a> {
        Game {
            version: 800,
            debug: 0,

            settings: Settings::default(),

            pro: true,
            id: 0,
            guid: [0; 4],

            constants: Vec::default(),

            sounds: Vec::default(),
            sprites: Vec::default(),
            backgrounds: Vec::default(),
            paths: Vec::default(),
            scripts: Vec::default(),
            objects: Vec::default(),
            rooms: Vec::default(),

            last_instance: 100000,
            last_tile: 10000000,

            extensions: Vec::default(),

            room_order: Vec::default(),
        }
    }
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            fullscreen: false,
            scaling: -1,
            interpolation: false,
            background_color: 0,
            allow_resize: false,
            topmost: false,
            hide_border: false,
            hide_buttons: false,
            show_cursor: true,
            freeze: false,
            disable_screensaver: true,

            set_resolution: false,
            color_depth: 0,
            resolution: 0,
            frequency: 0,
            vsync: false,

            default_esc: true,
            close_as_esc: true,
            default_f1: true,
            default_f4: true,
            default_f5: true,
            default_f9: true,
            priority: 0,

            load_image: false,
            load_transparent: false,
            load_alpha: 255,
            load_bar: 1,
            load_scale: true,

            error_display: true,
            error_log: false,
            error_abort: false,
            uninitialized_zero: false,
        }
    }
}
