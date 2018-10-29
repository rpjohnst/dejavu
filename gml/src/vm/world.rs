use std::collections::{HashMap, HashSet};

use index_map::IndexMap;
use symbol::Symbol;
use vm;

pub struct World {
    pub entities: vm::EntityAllocator,
    pub members: vm::EntityMap<HashMap<Symbol, vm::Value>>,

    pub objects: HashMap<i32, Vec<vm::Entity>>,
    pub instances: IndexMap<i32, vm::Entity>,

    pub globals: HashSet<Symbol>,
}

pub const GLOBAL: vm::Entity = vm::Entity(0);

impl Default for World {
    fn default() -> Self {
        let mut world = World {
            entities: vm::EntityAllocator::default(),
            members: vm::EntityMap::default(),

            objects: HashMap::default(),
            instances: IndexMap::default(),

            globals: HashSet::default(),
        };

        let global = world.entities.create();
        world.members.insert(global, HashMap::default());

        world
    }
}

impl World {
    pub fn create_instance(&mut self, id: i32) -> vm::Entity {
        let entity = self.entities.create();
        self.members.insert(entity, HashMap::default());
        self.instances.insert(id, entity);
        entity
    }
}

pub trait Api {
    fn state(&self) -> &World;
    fn state_mut(&mut self) -> &mut World;
}
