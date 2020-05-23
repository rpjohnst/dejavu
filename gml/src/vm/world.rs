use std::collections::{HashMap, HashSet};

use crate::rc_vec::RcVec;
use crate::symbol::Symbol;
use crate::vm;

pub struct World {
    pub entities: vm::EntityAllocator,
    pub members: vm::EntityMap<HashMap<Symbol, vm::Value>>,

    pub objects: HashMap<i32, RcVec<vm::Entity>>,
    pub instances: vm::InstanceMap<i32, vm::Entity>,

    pub globals: HashSet<Symbol>,
}

pub const GLOBAL: vm::Entity = vm::Entity(0);

impl Default for World {
    fn default() -> Self {
        let mut world = World {
            entities: vm::EntityAllocator::default(),
            members: vm::EntityMap::default(),

            objects: HashMap::default(),
            instances: vm::InstanceMap::default(),

            globals: HashSet::default(),
        };

        let global = world.entities.create();
        world.members.insert(global, HashMap::default());

        world
    }
}

impl World {
    pub fn create_entity(&mut self, object_index: i32, id: i32) -> vm::Entity {
        let entity = self.entities.create();
        self.members.insert(entity, HashMap::default());
        self.objects.entry(object_index).or_default().push(entity);
        self.instances.insert(id, entity);
        entity
    }

    // Remove an instance from the world, but retain its entity, to be destroyed or re-added later.
    pub fn remove_entity(&mut self, object_index: i32, id: i32, entity: vm::Entity) {
        self.instances.remove(id);

        if let Some(object_instances) = self.objects.get_mut(&object_index) {
            if let Some(position) = object_instances.iter().position(move |&e| e == entity) {
                object_instances.remove(position);
            }
        }
    }

    pub fn destroy_entity(&mut self, entity: vm::Entity) {
        self.members.remove(entity);
        self.entities.destroy(entity);
    }
}

pub trait Api {
    fn receivers(&mut self) -> &mut World;
}
