use std::collections::HashMap;
use gml::{self, symbol::Symbol, vm};
use crate::Context;

pub mod real;
pub mod string;
pub mod motion;
pub mod instance;
pub mod room;
pub mod draw;
pub mod show;
pub mod control;
pub mod data;

#[derive(Default)]
pub struct World {
    pub world: vm::World,
    pub real: real::State,
    pub string: string::State,
    pub motion: motion::State,
    pub instance: instance::State,
    pub room: room::State,
    pub draw: draw::State,
    pub show: show::State,
    pub control: control::State,
    pub data: data::State,
}

impl<'r> vm::Project<'r, (&'r mut vm::World,)> for Context {
    fn fields(&'r mut self) -> (&'r mut vm::World,) {
        let Context { world, .. } = self;
        (&mut world.world,)
    }
}
impl<'r> vm::Project<'r, (&'r mut vm::World, &'r mut vm::Assets<Self>)> for Context {
    fn fields(&'r mut self) -> (&'r mut vm::World, &'r mut vm::Assets<Self>) {
        let Context { world, assets } = self;
        (&mut world.world, &mut assets.code)
    }
}

impl<'r> vm::Project<'r, (&'r mut real::State,)> for Context {
    fn fields(&'r mut self) -> (&'r mut real::State,) {
        let Context { world, .. } = self;
        (&mut world.real,)
    }
}

impl<'r> vm::Project<'r, (&'r mut string::State,)> for Context {
    fn fields(&'r mut self) -> (&'r mut string::State,) {
        let Context { world, .. } = self;
        (&mut world.string,)
    }
}

impl<'r> vm::Project<'r, (&'r mut motion::State,)> for Context {
    fn fields(&'r mut self) -> (&'r mut motion::State,) {
        let Context { world, .. } = self;
        (&mut world.motion,)
    }
}

impl<'r> vm::Project<'r, (&'r mut instance::State,)> for Context {
    fn fields(&'r mut self) -> (&'r mut instance::State,) {
        let Context { world, .. } = self;
        (&mut world.instance,)
    }
}
impl<'r> vm::Project<'r, (&'r mut instance::State, &'r mut vm::World)> for Context {
    fn fields(&'r mut self) -> (&'r mut instance::State, &'r mut vm::World) {
        let Context { world, .. } = self;
        (&mut world.instance, &mut world.world)
    }
}

impl<'r> vm::Project<'r, (&'r mut draw::State,)> for Context {
    fn fields(&'r mut self) -> (&'r mut draw::State,) {
        let Context { world, .. } = self;
        (&mut world.draw,)
    }
}

impl<'r> vm::Project<'r, (&'r mut show::State,)> for Context {
    fn fields(&'r mut self) -> (&'r mut show::State,) {
        let Context { world, .. } = self;
        (&mut world.show,)
    }
}

impl<'r> vm::Project<'r, (&'r mut data::State,)> for Context {
    fn fields(&'r mut self) -> (&'r mut data::State,) {
        let Context { world, .. } = self;
        (&mut world.data,)
    }
}

impl World {
    pub fn from_assets(assets: &crate::Assets, debug: vm::Debug) -> Self {
        let mut world = Self::default();
        world.instance.next_id = assets.next_instance;
        world.show.debug = debug;
        world
    }

    pub fn register(items: &mut HashMap<Symbol, gml::Item<Context>>) {
        real::State::register(items);
        string::State::register(items);
        motion::State::register(items);
        instance::State::register(items);
        draw::State::register(items);
        show::State::register(items);
        control::State::register(items);
        data::State::register(items);
    }
}
