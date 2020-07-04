use std::collections::HashMap;

use gml::{self, symbol::Symbol, vm};

use crate::*;

#[derive(Default)]
pub struct World {
    pub world: vm::World,
    pub real: real::State,
    pub string: string::State,
    pub motion: motion::State,
    pub instance: instance::State,
    pub show: show::State,
    pub data: data::State,
}

impl vm::Api<'_, Assets> for World {
    fn fields<'r>(&'r mut self, assets: &'r mut Assets) ->
        (&'r mut vm::World, &'r mut vm::Assets<World, Assets>)
    { (&mut self.world, &mut assets.code) }
}

impl real::Api<'_, Assets> for World {
    fn fields(&mut self, _: &mut Assets) -> (&mut real::State,) { (&mut self.real,) }
}

impl string::Api<'_, Assets> for World {
    fn fields(&mut self, _: &mut Assets) -> () {}
}

impl motion::Api<'_, Assets> for World {
    fn fields(&mut self, _: &mut Assets) -> (&mut motion::State,) { (&mut self.motion,) }
}

impl instance::Api<'_, Assets> for World {
    fn fields<'r>(&'r mut self, _: &'r mut Assets) -> (
        &'r mut instance::State, &'r mut vm::World, &'r mut motion::State,
    ) { (
        &mut self.instance, &mut self.world, &mut self.motion,
    ) }
}

impl show::Api<'_, Assets> for World {
    fn fields(&mut self, _: &mut Assets) -> (&mut show::State,) { (&mut self.show,) }
}

impl data::Api<'_, Assets> for World {
    fn fields(&mut self, _: &mut Assets) -> (&mut data::State,) { (&mut self.data,) }
}

impl World {
    pub fn register(items: &mut HashMap<Symbol, gml::Item<Self, Assets>>) {
        real::Api::register(items);
        string::Api::register(items);
        motion::Api::register(items);
        instance::Api::register(items);
        show::Api::register(items);
        data::Api::register(items);
    }
}
