use std::collections::HashMap;
use gml::{self, symbol::Symbol, vm};
use crate::Context;

pub mod real;
pub mod string;
pub mod motion;
pub mod instance;
pub mod room;
pub mod score;
pub mod debug;
pub mod draw;
pub mod ini;
pub mod data;
pub mod external;
pub mod control;

#[derive(Default)]
pub struct World {
    pub world: vm::World,
    pub real: real::State,
    pub string: string::State,
    pub motion: motion::State,
    pub instance: instance::State,
    pub room: room::State,
    pub score: score::State,
    pub debug: debug::State,
    pub draw: draw::State,
    pub ini: ini::State,
    pub data: data::State,
    pub external: external::State,
    pub control: control::State,
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

impl<'r> vm::Project<'r, (&'r mut room::State,)> for Context {
    fn fields(&'r mut self) -> (&'r mut room::State,) {
        let Context { world, .. } = self;
        (&mut world.room,)
    }
}

impl<'r> vm::Project<'r, (&'r mut score::State,)> for Context {
    fn fields(&'r mut self) -> (&'r mut score::State,) {
        let Context { world, .. } = self;
        (&mut world.score,)
    }
}

impl<'r> vm::Project<'r, (&'r mut debug::State,)> for Context {
    fn fields(&'r mut self) -> (&'r mut debug::State,) {
        let Context { world, .. } = self;
        (&mut world.debug,)
    }
}

impl<'r> vm::Project<'r, (&'r mut draw::State,)> for Context {
    fn fields(&'r mut self) -> (&'r mut draw::State,) {
        let Context { world, .. } = self;
        (&mut world.draw,)
    }
}

impl<'r> vm::Project<'r, (&'r mut ini::State,)> for Context {
    fn fields(&'r mut self) -> (&'r mut ini::State,) {
        let Context { world, .. } = self;
        (&mut world.ini,)
    }
}

impl<'r> vm::Project<'r, (&'r mut data::State,)> for Context {
    fn fields(&'r mut self) -> (&'r mut data::State,) {
        let Context { world, .. } = self;
        (&mut world.data,)
    }
}

impl<'r> vm::Project<'r, (&'r mut external::State,)> for Context {
    fn fields(&'r mut self) -> (&'r mut external::State,) {
        let Context { world, .. } = self;
        (&mut world.external,)
    }
}

impl World {
    pub fn from_assets(assets: &crate::Assets, debug: vm::Debug) -> Self {
        let mut world = Self::default();
        world.instance.next_id = assets.next_instance;
        world.debug.debug = debug;
        world
    }

    pub fn register(items: &mut HashMap<Symbol, gml::Item<Context>>) {
        real::State::register(items);
        string::State::register(items);
        motion::State::register(items);
        instance::State::register(items);
        room::State::register(items);
        score::State::register(items);
        debug::State::register(items);
        draw::State::register(items);
        ini::State::register(items);
        data::State::register(items);
        external::State::register(items);
        control::State::register(items);
    }
}
