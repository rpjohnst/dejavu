use gml::symbol::Symbol;
use gml::{self, vm};

#[derive(Default)]
pub struct State {
    pub instances: vm::EntityMap<Instance>,
}

pub struct Instance {
    pub object_index: i32,
    pub id: i32,
    pub persistent: bool,
}

#[gml::bind(Api)]
impl State {
    pub fn create_instance(&mut self, entity: vm::Entity, instance: Instance) {
        self.instances.insert(entity, instance);
    }

    #[gml::get(object_index)]
    pub fn get_object_index(&self, entity: vm::Entity) -> i32 {
        self.instances[entity].object_index
    }

    #[gml::get(id)]
    pub fn get_id(&self, entity: vm::Entity) -> i32 { self.instances[entity].id }

    #[gml::get(persistent)]
    pub fn get_persistent(&self, entity: vm::Entity) -> bool {
        self.instances[entity].persistent
    }
    #[gml::set(persistent)]
    pub fn set_persistent(&mut self, entity: vm::Entity, value: bool) {
        self.instances[entity].persistent = value;
    }

    #[gml::get(instance_count)]
    pub fn get_instance_count(&self, world: &vm::World) -> i32 {
        world.instances.len() as i32
    }

    #[gml::get(instance_id)]
    pub fn get_instance_id(&self, world: &vm::World, i: usize) -> i32 {
        let entity = match world.instances.values().get(i) {
            Some(&entity) => entity,
            None => return vm::NOONE,
        };
        self.instances[entity].id
    }

    #[gml::function]
    pub fn instance_find(&mut self, world: &mut vm::World, obj: i32, n: i32) ->
        Result<i32, vm::ErrorKind>
    {
        let n = n as usize;
        let entity = if obj == vm::ALL {
            match world.instances.values().get(n) {
                Some(&entity) => entity,
                None => return Ok(vm::NOONE),
            }
        } else {
            match world.objects.get(&obj) {
                Some(entities) => match entities.get(n) {
                    Some(&entity) => entity,
                    None => return Ok(vm::NOONE),
                },
                None => return Ok(vm::NOONE),
            }
        };
        Ok(self.instances[entity].id)
    }

    #[gml::function]
    pub fn instance_exists(&mut self, world: &mut vm::World, obj: i32) ->
        Result<bool, vm::ErrorKind>
    {
        if obj == vm::ALL {
            Ok(!world.instances.is_empty())
        } else if obj < 100000 {
            Ok(!world.objects.get(&obj).map_or(false, |entities| !entities.is_empty()))
        } else {
            Ok(world.instances.contains_key(obj))
        }
    }
}
