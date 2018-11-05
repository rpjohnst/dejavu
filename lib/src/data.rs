use std::collections::{HashMap, hash_map::Entry};

use gml::symbol::Symbol;
use gml::{self, vm};

#[derive(Default)]
pub struct State {
    lists: HashMap<i32, List>,
    next_list: i32,
}

type List = Vec<vm::Value>;

#[gml::bind(Api)]
impl State {
    #[gml::function]
    pub fn ds_list_create(&mut self) -> i32 {
        let id = self.next_list;
        self.next_list += 1;
        self.lists.insert(id, Vec::new());
        id
    }

    #[gml::function]
    pub fn ds_list_destroy(&mut self, id: i32) -> Result<(), vm::ErrorKind> {
        let entry = match self.lists.entry(id) {
            Entry::Occupied(entry) => entry,
            Entry::Vacant(_) => return Err(vm::ErrorKind::Resource(id)),
        };
        entry.remove();
        Ok(())
    }

    #[gml::function]
    pub fn ds_list_clear(&mut self, id: i32) -> Result<(), vm::ErrorKind> {
        let list = self.lists.get_mut(&id).ok_or(vm::ErrorKind::Resource(id))?;
        list.clear();
        Ok(())
    }

    #[gml::function]
    pub fn ds_list_empty(&mut self, id: i32) -> Result<bool, vm::ErrorKind> {
        let list = self.lists.get(&id).ok_or(vm::ErrorKind::Resource(id))?;
        let empty = list.is_empty();
        Ok(empty)
    }

    #[gml::function]
    pub fn ds_list_size(&mut self, id: i32) -> Result<i32, vm::ErrorKind> {
        let list = self.lists.get(&id).ok_or(vm::ErrorKind::Resource(id))?;
        let size = list.len() as i32;
        Ok(size)
    }

    #[gml::function]
    pub fn ds_list_add(&mut self, id: i32, vals: &[vm::Value]) -> Result<(), vm::ErrorKind> {
        let list = self.lists.get_mut(&id).ok_or(vm::ErrorKind::Resource(id))?;
        list.extend_from_slice(vals);
        Ok(())
    }

    #[gml::function]
    pub fn ds_list_delete(&mut self, id: i32, pos: i32) -> Result<(), vm::ErrorKind> {
        let list = self.lists.get_mut(&id).ok_or(vm::ErrorKind::Resource(id))?;
        if pos < 0 || list.len() <= pos as usize {
            return Ok(());
        }
        list.remove(pos as usize);
        Ok(())
    }

    #[gml::function]
    pub fn ds_list_find_index(&mut self, id: i32, val: vm::Value) -> Result<i32, vm::ErrorKind> {
        let list = self.lists.get(&id).ok_or(vm::ErrorKind::Resource(id))?;
        let pos = list.iter()
            .position(move |&e| e == val)
            .map_or(-1, |i| i as i32);
        Ok(pos)
    }

    #[gml::function]
    pub fn ds_list_find_value(&mut self, id: i32, pos: i32) -> Result<vm::Value, vm::ErrorKind> {
        let list = self.lists.get(&id).ok_or(vm::ErrorKind::Resource(id))?;
        if pos < 0 || list.len() <= pos as usize {
            return Ok(vm::Value::from(0));
        }
        let val = list.get(pos as usize).map_or(vm::Value::from(0), |&val| val);
        Ok(val)
    }

    #[gml::function]
    pub fn ds_list_insert(&mut self, id: i32, pos: i32, val: vm::Value) ->
        Result<(), vm::ErrorKind>
    {
        let list = self.lists.get_mut(&id).ok_or(vm::ErrorKind::Resource(id))?;
        if pos < 0 || list.len() < pos as usize {
            return Ok(());
        }
        list.insert(pos as usize, val);
        Ok(())
    }

    #[gml::function]
    pub fn ds_list_replace(&mut self, id: i32, pos: i32, val: vm::Value) ->
        Result<(), vm::ErrorKind>
    {
        let list = self.lists.get_mut(&id).ok_or(vm::ErrorKind::Resource(id))?;
        if pos < 0 || list.len() <= pos as usize {
            return Ok(());
        }
        list[pos as usize] = val;
        Ok(())
    }
}
