use std::collections::{HashMap, HashSet};

use index_map::IndexMap;
use symbol::Symbol;
use vm;

pub struct World {
    next_entity: u64,
    pub(in vm) entities: HashMap<Entity, Hash>,

    pub(in vm) globals: HashSet<Symbol>,
    pub(in vm) objects: HashMap<i32, Vec<Entity>>,
    pub(in vm) instances: IndexMap<i32, Entity>,
}

#[derive(Copy, Clone, Default, PartialEq, Eq, Hash, Debug)]
pub struct Entity(u64);

pub const GLOBAL: Entity = Entity(0);

pub type Hash = HashMap<Symbol, vm::Value>;

pub trait Api {
    fn state(&mut self) -> &mut World;
}

impl Default for World {
    fn default() -> Self {
        let Entity(global) = GLOBAL;

        let mut world = World {
            next_entity: global,
            entities: HashMap::default(),

            globals: HashSet::default(),
            objects: HashMap::default(),
            instances: IndexMap::default(),
        };

        let global = world.create_entity();
        world.entities.insert(global, Hash::new());

        world
    }
}

impl World {
    fn create_entity(&mut self) -> Entity {
        let entity = self.next_entity;
        self.next_entity += 1;
        Entity(entity)
    }

    pub fn create_instance(&mut self, id: i32) -> Entity {
        let entity = self.create_entity();
        self.instances.insert(id, entity);
        self.entities.insert(entity, Hash::new());
        entity
    }
}
