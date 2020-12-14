use gml::{self, vm};
use crate::Context;

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

impl State {
    pub fn simulate(cx: &mut Context) {
        let Context { world, .. } = cx;
        let crate::World { world, motion, .. } = world;
        let entities = world.instances.values().clone();

        for &entity in entities.iter() {
            let instance = &mut motion.instances[entity];

            instance.xprevious = instance.x;
            instance.yprevious = instance.y;

            instance.x += instance.hspeed;
            instance.y += instance.vspeed;
        }
    }
}

#[gml::bind]
impl State {
    #[gml::get(x)]
    pub fn get_x(&self, entity: vm::Entity) -> f32 { self.instances[entity].x }
    #[gml::set(x)]
    pub fn set_x(&mut self, entity: vm::Entity, value: f32) { self.instances[entity].x = value }

    #[gml::get(y)]
    pub fn get_y(&self, entity: vm::Entity) -> f32 { self.instances[entity].y }
    #[gml::set(y)]
    pub fn set_y(&mut self, entity: vm::Entity, value: f32) { self.instances[entity].y = value }

    #[gml::get(hspeed)]
    pub fn get_hspeed(&self, entity: vm::Entity) -> f32 { self.instances[entity].hspeed }
    #[gml::set(hspeed)]
    pub fn set_hspeed(&mut self, entity: vm::Entity, value: f32) {
        let instance = &mut self.instances[entity];
        instance.hspeed = value;
    }

    #[gml::get(vspeed)]
    pub fn get_vspeed(&self, entity: vm::Entity) -> f32 { self.instances[entity].vspeed }
    #[gml::set(vspeed)]
    pub fn set_vspeed(&mut self, entity: vm::Entity, value: f32) {
        let instance = &mut self.instances[entity];
        instance.vspeed = value;
    }

    #[gml::api]
    pub fn action_move_to(&mut self, entity: vm::Entity, relative: bool, mut x: f32, mut y: f32) {
        if relative {
            x += self.instances[entity].x;
            y += self.instances[entity].y;
        }
        self.instances[entity].x = x;
        self.instances[entity].y = y;
    }
}
