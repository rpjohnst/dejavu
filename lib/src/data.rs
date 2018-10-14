use std::collections::{HashMap, hash_map::Entry};
use std::convert::TryFrom;

use gml::symbol::Symbol;
use gml::{self, vm};

#[derive(Default)]
pub struct State {
    lists: HashMap<i32, List>,
    next_list: i32,
}

type List = Vec<vm::Value>;

pub trait Api {
    fn state(&mut self) -> &mut State;

    fn register(items: &mut HashMap<Symbol, gml::Item<Self>>) where Self: Sized {
        let ds_list_create = Symbol::intern("ds_list_create");
        items.insert(ds_list_create, gml::Item::Native(Self::ds_list_create, 0, false));

        let ds_list_destroy = Symbol::intern("ds_list_destroy");
        items.insert(ds_list_destroy, gml::Item::Native(Self::ds_list_destroy, 1, false));

        let ds_list_clear = Symbol::intern("ds_list_clear");
        items.insert(ds_list_clear, gml::Item::Native(Self::ds_list_clear, 1, false));

        let ds_list_empty = Symbol::intern("ds_list_empty");
        items.insert(ds_list_empty, gml::Item::Native(Self::ds_list_empty, 1, false));

        let ds_list_size = Symbol::intern("ds_list_size");
        items.insert(ds_list_size, gml::Item::Native(Self::ds_list_size, 1, false));

        let ds_list_add = Symbol::intern("ds_list_add");
        items.insert(ds_list_add, gml::Item::Native(Self::ds_list_add, 2, true));

        let ds_list_delete = Symbol::intern("ds_list_delete");
        items.insert(ds_list_delete, gml::Item::Native(Self::ds_list_delete, 2, false));

        let ds_list_find_index = Symbol::intern("ds_list_find_index");
        items.insert(ds_list_find_index, gml::Item::Native(Self::ds_list_find_index, 2, false));

        let ds_list_find_value = Symbol::intern("ds_list_find_value");
        items.insert(ds_list_find_value, gml::Item::Native(Self::ds_list_find_value, 2, false));

        let ds_list_insert = Symbol::intern("ds_list_insert");
        items.insert(ds_list_insert, gml::Item::Native(Self::ds_list_insert, 3, false));

        let ds_list_replace = Symbol::intern("ds_list_replace");
        items.insert(ds_list_replace, gml::Item::Native(Self::ds_list_replace, 3, false));
    }

    fn ds_list_create(&mut self, _arguments: &[vm::Value]) -> Result<vm::Value, vm::ErrorKind> {
        let state = self.state();

        let id = state.next_list;
        state.next_list += 1;
        state.lists.insert(id, Vec::new());

        Ok(vm::Value::from(id))
    }

    fn ds_list_destroy(&mut self, arguments: &[vm::Value]) -> Result<vm::Value, vm::ErrorKind> {
        let state = self.state();
        let id = i32::try_from(arguments[0]).unwrap_or(0);

        let entry = match state.lists.entry(id) {
            Entry::Occupied(entry) => entry,
            Entry::Vacant(_) => return Err(vm::ErrorKind::Resource(id)),
        };
        entry.remove();

        Ok(vm::Value::from(0))
    }

    fn ds_list_clear(&mut self, arguments: &[vm::Value]) -> Result<vm::Value, vm::ErrorKind> {
        let state = self.state();
        let id = i32::try_from(arguments[0]).unwrap_or(0);

        let list = state.lists.get_mut(&id).ok_or(vm::ErrorKind::Resource(id))?;
        list.clear();

        Ok(vm::Value::from(0))
    }

    fn ds_list_empty(&mut self, arguments: &[vm::Value]) -> Result<vm::Value, vm::ErrorKind> {
        let state = self.state();
        let id = i32::try_from(arguments[0]).unwrap_or(0);

        let list = state.lists.get(&id).ok_or(vm::ErrorKind::Resource(id))?;
        let empty = list.is_empty();

        Ok(vm::Value::from(empty))
    }

    fn ds_list_size(&mut self, arguments: &[vm::Value]) -> Result<vm::Value, vm::ErrorKind> {
        let state = self.state();
        let id = i32::try_from(arguments[0]).unwrap_or(0);

        let list = state.lists.get(&id).ok_or(vm::ErrorKind::Resource(id))?;
        let size = list.len() as i32;

        Ok(vm::Value::from(size))
    }

    fn ds_list_add(&mut self, arguments: &[vm::Value]) -> Result<vm::Value, vm::ErrorKind> {
        let state = self.state();
        let id = i32::try_from(arguments[0]).unwrap_or(0);
        let vals = &arguments[1..];

        let list = state.lists.get_mut(&id).ok_or(vm::ErrorKind::Resource(id))?;
        list.extend_from_slice(vals);

        Ok(vm::Value::from(0))
    }

    fn ds_list_delete(&mut self, arguments: &[vm::Value]) -> Result<vm::Value, vm::ErrorKind> {
        let state = self.state();
        let id = i32::try_from(arguments[0]).unwrap_or(0);
        let pos = i32::try_from(arguments[1]).unwrap_or(0);

        let list = state.lists.get_mut(&id).ok_or(vm::ErrorKind::Resource(id))?;
        if pos < 0 || list.len() <= pos as usize {
            return Ok(vm::Value::from(0));
        }
        list.remove(pos as usize);

        Ok(vm::Value::from(0))
    }

    fn ds_list_find_index(&mut self, arguments: &[vm::Value]) -> Result<vm::Value, vm::ErrorKind> {
        let state = self.state();
        let id = i32::try_from(arguments[0]).unwrap_or(0);
        let val = arguments[1];

        let list = state.lists.get(&id).ok_or(vm::ErrorKind::Resource(id))?;
        let pos = list.iter()
            .position(move |&e| e == val)
            .map_or(-1, |i| i as i32);

        Ok(vm::Value::from(pos))
    }

    fn ds_list_find_value(&mut self, arguments: &[vm::Value]) -> Result<vm::Value, vm::ErrorKind> {
        let state = self.state();
        let id = i32::try_from(arguments[0]).unwrap_or(0);
        let pos = i32::try_from(arguments[1]).unwrap_or(0);

        let list = state.lists.get(&id).ok_or(vm::ErrorKind::Resource(id))?;
        if pos < 0 || list.len() <= pos as usize {
            return Ok(vm::Value::from(0));
        }
        let val = list.get(pos as usize).map_or(vm::Value::from(0), |&val| val);

        Ok(val)
    }

    fn ds_list_insert(&mut self, arguments: &[vm::Value]) -> Result<vm::Value, vm::ErrorKind> {
        let state = self.state();
        let id = i32::try_from(arguments[0]).unwrap_or(0);
        let pos = i32::try_from(arguments[1]).unwrap_or(0);
        let val = arguments[2];

        let list = state.lists.get_mut(&id).ok_or(vm::ErrorKind::Resource(id))?;
        if pos < 0 || list.len() < pos as usize {
            return Ok(vm::Value::from(0));
        }
        list.insert(pos as usize, val);

        Ok(vm::Value::from(0))
    }

    fn ds_list_replace(&mut self, arguments: &[vm::Value]) -> Result<vm::Value, vm::ErrorKind> {
        let state = self.state();
        let id = i32::try_from(arguments[0]).unwrap_or(0);
        let pos = i32::try_from(arguments[1]).unwrap_or(0);
        let val = arguments[2];

        let list = state.lists.get_mut(&id).ok_or(vm::ErrorKind::Resource(id))?;
        if pos < 0 || list.len() <= pos as usize {
            return Ok(vm::Value::from(0));
        }
        list[pos as usize] = val;

        Ok(vm::Value::from(0))
    }
}
