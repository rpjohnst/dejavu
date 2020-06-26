use gml::{self, vm};

#[derive(Default)]
pub struct State {
    pub instances: vm::EntityMap<Instance>,
}

pub struct Instance {
    pub x: f32,
    pub y: f32,
    pub xprevious: f32,
    pub yprevious: f32,
    pub xstart: f32,
    pub ystart: f32,
    pub hspeed: f32,
    pub vspeed: f32,
    pub direction: f32,
    pub speed: f32,
    pub friction: f32,
    pub gravity: f32,
    pub gravity_direction: f32,
}

impl Instance {
    pub fn from_pos(x: f32, y: f32) -> Self {
        Instance {
            x, y, xprevious: x, yprevious: y, xstart: x, ystart: y,
            hspeed: 0.0, vspeed: 0.0, direction: 0.0, speed: 0.0,
            friction: 0.0, gravity: 0.0, gravity_direction: 0.0,
        }
    }
}

#[gml::bind(Api)]
impl State {
    #[gml::get(x)]
    pub fn get_x(&self, entity: vm::Entity) -> f32 { self.instances[entity].x }
    #[gml::set(x)]
    pub fn set_x(&mut self, entity: vm::Entity, value: f32) { self.instances[entity].x = value }

    #[gml::get(y)]
    pub fn get_y(&self, entity: vm::Entity) -> f32 { self.instances[entity].y }
    #[gml::set(y)]
    pub fn set_y(&mut self, entity: vm::Entity, value: f32) { self.instances[entity].y = value }

    #[gml::function]
    pub fn action_move_to(&mut self, entity: vm::Entity, relative: bool, mut x: f32, mut y: f32) {
        if relative {
            x += self.instances[entity].x;
            y += self.instances[entity].y;
        }
        self.instances[entity].x = x;
        self.instances[entity].y = y;
    }
}
