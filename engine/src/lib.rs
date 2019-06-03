use std::collections::HashMap;

use gml::{self, symbol::Symbol, vm};

pub mod real;
pub mod string;
pub mod show;
pub mod instance;
pub mod data;

#[derive(Default)]
pub struct Engine {
    pub world: vm::World,
    pub real: real::State,
    pub string: string::State,
    pub show: show::State,
    pub instance: instance::State,
    pub data: data::State,
}

impl vm::world::Api for Engine {
    fn state(&self) -> &vm::World { &self.world }
    fn state_mut(&mut self) -> &mut vm::World { &mut self.world }
}

impl real::Api for Engine {
    fn state(&self) -> (&real::State, &vm::World) { (&self.real, &self.world) }
    fn state_mut(&mut self) -> (&mut real::State, &mut vm::World) {
        (&mut self.real, &mut self.world)
    }
}

impl string::Api for Engine {
    fn state(&self) -> (&string::State, &vm::World) { (&self.string, &self.world) }
    fn state_mut(&mut self) -> (&mut string::State, &mut vm::World) {
        (&mut self.string, &mut self.world)
    }
}

impl show::Api for Engine {
    fn state(&self) -> (&show::State, &vm::World) { (&self.show, &self.world) }
    fn state_mut(&mut self) -> (&mut show::State, &mut vm::World) {
        (&mut self.show, &mut self.world)
    }
}

impl instance::Api for Engine {
    fn state(&self) -> (&instance::State, &vm::World) { (&self.instance, &self.world) }
    fn state_mut(&mut self) -> (&mut instance::State, &mut vm::World) {
        (&mut self.instance, &mut self.world)
    }
}

impl data::Api for Engine {
    fn state(&self) -> (&data::State, &vm::World) { (&self.data, &self.world) }
    fn state_mut(&mut self) -> (&mut data::State, &mut vm::World) {
        (&mut self.data, &mut self.world)
    }
}

impl Engine {
    pub fn register(items: &mut HashMap<Symbol, gml::Item<Self>>) {
        real::Api::register(items);
        string::Api::register(items);
        show::Api::register(items);
        instance::Api::register(items);
        data::Api::register(items);
    }
}
