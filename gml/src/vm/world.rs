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
    pub constants: Vec<vm::Value>,
}

pub const GLOBAL: vm::Entity = vm::Entity::NULL;

impl Default for World {
    fn default() -> Self {
        let mut world = World {
            entities: vm::EntityAllocator::default(),
            members: vm::EntityMap::default(),

            objects: HashMap::default(),
            instances: vm::InstanceMap::default(),

            globals: HashSet::default(),
            constants: Vec::default(),
        };

        let global = world.entities.create();
        world.members.insert(global, HashMap::default());

        world
    }
}

impl World {
    pub fn load<W: for<'r> vm::Project<'r, (&'r mut World, &'r mut vm::Assets<W>)>>(
        cx: &mut W, thread: &mut vm::Thread
    ) -> vm::Result<()> {
        let (_, assets) = cx.fields();
        for id in 0..assets.constants {
            let value = thread.execute(cx, crate::Function::Constant { id }, vec![])?;

            let (world, _) = cx.fields();
            world.constants.push(value);
        }
        Ok(())
    }

    /// Create an entity with a scope, but do not add it to the instance lists.
    pub fn create_entity(&mut self) -> vm::Entity {
        let entity = self.entities.create();
        self.members.insert(entity, HashMap::default());
        entity
    }

    /// Remove an entity from the world. Should normally be called after `remove_entity`.
    pub fn destroy_entity(&mut self, entity: vm::Entity) {
        self.members.remove(entity);
        self.entities.destroy(entity);
    }

    /// Add an entity to the instance lists.
    pub fn add_entity(&mut self, entity: vm::Entity, object_index: i32, id: i32) {
        self.objects.entry(object_index).or_default().push(entity);
        self.instances.insert(id, entity);
    }

    /// Remove an entity from the instance lists, but retain its entity.
    pub fn remove_entity(&mut self, entity: vm::Entity, object_index: i32, id: i32) {
        self.instances.remove(id);

        if let Some(object_instances) = self.objects.get_mut(&object_index) {
            if let Some(position) = object_instances.iter().position(move |&e| e == entity) {
                object_instances.remove(position);
            }
        }
    }
}
