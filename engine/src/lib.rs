#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
#[macro_use]
extern crate wasm_host;

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
    fn receivers(&mut self) -> &mut vm::World { &mut self.world }
}

impl real::Api for Engine {
    fn receivers(&mut self) -> (&mut real::State,) { (&mut self.real,) }
}

impl string::Api for Engine {
    fn receivers(&mut self) -> () {}
}

impl show::Api for Engine {
    fn receivers(&mut self) -> (&mut show::State,) { (&mut self.show,) }
}

impl instance::Api for Engine {
    fn receivers(&mut self) -> (&mut instance::State, &mut vm::World) {
        (&mut self.instance, &mut self.world)
    }
}

impl data::Api for Engine {
    fn receivers(&mut self) -> (&mut data::State,) { (&mut self.data,) }
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
