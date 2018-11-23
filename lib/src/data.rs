use std::{fmt, error};
use std::collections::{hash_map, HashMap};

use gml::symbol::Symbol;
use gml::{self, vm};

#[derive(Default)]
pub struct State {
    lists: HashMap<i32, List>,
    next_list: i32,
}

type List = Vec<vm::Value>;

#[derive(Debug)]
pub enum Error {
    /// The resource does not exist.
    Resource(Type, i32),
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum Type {
    List,
}

impl From<Error> for vm::ErrorKind {
    fn from(error: Error) -> Self {
        vm::ErrorKind::Other(Box::new(error))
    }
}

impl fmt::Display for Error {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::Resource(kind, id) => {
                write!(fmt, "The {:?} with id {} does not exist.", kind, id)?;
            }
        }
        Ok(())
    }
}

impl error::Error for Error {}

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
        use self::hash_map::Entry;
        let entry = match self.lists.entry(id) {
            Entry::Occupied(entry) => entry,
            Entry::Vacant(_) => Err(Error::Resource(Type::List, id))?,
        };
        entry.remove();
        Ok(())
    }

    #[gml::function]
    pub fn ds_list_clear(&mut self, id: i32) -> Result<(), vm::ErrorKind> {
        let list = self.lists.get_mut(&id).ok_or(Error::Resource(Type::List, id))?;
        list.clear();
        Ok(())
    }

    #[gml::function]
    pub fn ds_list_empty(&mut self, id: i32) -> Result<bool, vm::ErrorKind> {
        let list = self.lists.get(&id).ok_or(Error::Resource(Type::List, id))?;
        let empty = list.is_empty();
        Ok(empty)
    }

    #[gml::function]
    pub fn ds_list_size(&mut self, id: i32) -> Result<i32, vm::ErrorKind> {
        let list = self.lists.get(&id).ok_or(Error::Resource(Type::List, id))?;
        let size = list.len() as i32;
        Ok(size)
    }

    #[gml::function]
    pub fn ds_list_add(&mut self, id: i32, vals: &[vm::Value]) -> Result<(), vm::ErrorKind> {
        let list = self.lists.get_mut(&id).ok_or(Error::Resource(Type::List, id))?;
        list.extend_from_slice(vals);
        Ok(())
    }

    #[gml::function]
    pub fn ds_list_delete(&mut self, id: i32, pos: i32) -> Result<(), vm::ErrorKind> {
        let list = self.lists.get_mut(&id).ok_or(Error::Resource(Type::List, id))?;
        if pos < 0 || list.len() <= pos as usize {
            return Ok(());
        }
        list.remove(pos as usize);
        Ok(())
    }

    #[gml::function]
    pub fn ds_list_find_index(&mut self, id: i32, val: vm::Value) -> Result<i32, vm::ErrorKind> {
        let list = self.lists.get(&id).ok_or(Error::Resource(Type::List, id))?;
        let pos = list.iter()
            .position(move |&e| e == val)
            .map_or(-1, |i| i as i32);
        Ok(pos)
    }

    #[gml::function]
    pub fn ds_list_find_value(&mut self, id: i32, pos: i32) -> Result<vm::Value, vm::ErrorKind> {
        let list = self.lists.get(&id).ok_or(Error::Resource(Type::List, id))?;
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
        let list = self.lists.get_mut(&id).ok_or(Error::Resource(Type::List, id))?;
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
        let list = self.lists.get_mut(&id).ok_or(Error::Resource(Type::List, id))?;
        if pos < 0 || list.len() <= pos as usize {
            return Ok(());
        }
        list[pos as usize] = val;
        Ok(())
    }
}
