use std::{cmp, fmt, error};
use std::collections::{hash_map, HashMap};
use std::collections::{btree_map, BTreeMap};

use gml::symbol::Symbol;
use gml::{self, vm};

#[derive(Default)]
pub struct State {
    lists: HashMap<i32, List>,
    next_list: i32,

    maps: HashMap<i32, Map>,
    next_map: i32,
}

type List = Vec<vm::Value>;

type Map = BTreeMap<MapKey, vm::Value>;

#[derive(Copy, Clone, PartialEq, Eq)]
struct MapKey(vm::Value);

impl cmp::PartialOrd for MapKey {
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        Some(MapKey::cmp(self, other))
    }
}

impl cmp::Ord for MapKey {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        let MapKey(a) = *self;
        let MapKey(b) = *other;
        match (a.data(), b.data()) {
            // This unwrap is fine because vm::Value should never be NaN.
            (vm::Data::Real(a), vm::Data::Real(b)) => f64::partial_cmp(&a, &b).unwrap(),
            (vm::Data::Real(_), vm::Data::Array(_)) => cmp::Ordering::Less,
            (vm::Data::Real(_), vm::Data::String(_)) => cmp::Ordering::Less,

            (vm::Data::Array(_), vm::Data::Real(_)) => cmp::Ordering::Greater,
            (vm::Data::Array(a), vm::Data::Array(b)) => {
                <*const _>::cmp(&(a.as_ref() as *const _), &(b.as_ref() as *const _))
            }
            (vm::Data::Array(_), vm::Data::String(_)) => cmp::Ordering::Less,

            (vm::Data::String(_), vm::Data::Real(_)) => cmp::Ordering::Greater,
            (vm::Data::String(_), vm::Data::Array(_)) => cmp::Ordering::Greater,
            (vm::Data::String(a), vm::Data::String(b)) => Symbol::cmp(&a, &b),
        }
    }
}

#[derive(Debug)]
pub enum Error {
    /// The resource does not exist.
    Resource(Type, i32),
    /// The key already exists in a map.
    KeyExists(vm::Value),
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum Type {
    List,
    Map,
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
            Error::KeyExists(key) => {
                write!(fmt, "An entry with key {:?} already exists in the map.", key)?;
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
        self.lists.insert(id, List::default());
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

    #[gml::function]
    pub fn ds_map_create(&mut self) -> i32 {
        let id = self.next_map;
        self.next_map += 1;
        self.maps.insert(id, Map::default());
        id
    }

    #[gml::function]
    pub fn ds_map_destroy(&mut self, id: i32) -> Result<(), vm::ErrorKind> {
        use self::hash_map::Entry;
        let entry = match self.maps.entry(id) {
            Entry::Occupied(entry) => entry,
            Entry::Vacant(_) => Err(Error::Resource(Type::Map, id))?,
        };
        entry.remove();
        Ok(())
    }

    #[gml::function]
    pub fn ds_map_clear(&mut self, id: i32) -> Result<(), vm::ErrorKind> {
        let map = self.maps.get_mut(&id).ok_or(Error::Resource(Type::Map, id))?;
        map.clear();
        Ok(())
    }

    #[gml::function]
    pub fn ds_map_size(&mut self, id: i32) -> Result<i32, vm::ErrorKind> {
        let map = self.maps.get(&id).ok_or(Error::Resource(Type::Map, id))?;
        let size = map.len() as i32;
        Ok(size)
    }

    #[gml::function]
    pub fn ds_map_empty(&mut self, id: i32) -> Result<bool, vm::ErrorKind> {
        let map = self.maps.get(&id).ok_or(Error::Resource(Type::Map, id))?;
        let empty = map.is_empty();
        Ok(empty)
    }

    #[gml::function]
    pub fn ds_map_add(&mut self, id: i32, key: vm::Value, val: vm::Value) ->
        Result<(), vm::ErrorKind>
    {
        use self::btree_map::Entry;
        let map = self.maps.get_mut(&id).ok_or(Error::Resource(Type::Map, id))?;
        let entry = match map.entry(MapKey(key)) {
            Entry::Occupied(_) => Err(Error::KeyExists(key))?,
            Entry::Vacant(entry) => entry,
        };
        entry.insert(val);
        Ok(())
    }

    #[gml::function]
    pub fn ds_map_replace(&mut self, id: i32, key: vm::Value, val: vm::Value) ->
        Result<(), vm::ErrorKind>
    {
        use self::btree_map::Entry;
        let map = self.maps.get_mut(&id).ok_or(Error::Resource(Type::Map, id))?;
        let mut entry = match map.entry(MapKey(key)) {
            Entry::Occupied(entry) => entry,
            Entry::Vacant(_) => return Ok(()),
        };
        entry.insert(val);
        Ok(())
    }

    #[gml::function]
    pub fn ds_map_delete(&mut self, id: i32, key: vm::Value) -> Result<(), vm::ErrorKind> {
        let map = self.maps.get_mut(&id).ok_or(Error::Resource(Type::Map, id))?;
        map.remove(&MapKey(key));
        Ok(())
    }

    #[gml::function]
    pub fn ds_map_exists(&mut self, id: i32, key: vm::Value) -> Result<(), vm::ErrorKind> {
        let map = self.maps.get_mut(&id).ok_or(Error::Resource(Type::Map, id))?;
        map.contains_key(&MapKey(key));
        Ok(())
    }

    #[gml::function]
    pub fn ds_map_find_value(&mut self, id: i32, key: vm::Value) ->
        Result<vm::Value, vm::ErrorKind>
    {
        let map = self.maps.get(&id).ok_or(Error::Resource(Type::Map, id))?;
        let val = map.get(&MapKey(key)).map_or(vm::Value::from(0), |&val| val);
        Ok(val)
    }

    #[gml::function]
    pub fn ds_map_find_next(&mut self, id: i32, key: vm::Value) ->
        Result<vm::Value, vm::ErrorKind>
    {
        let map = self.maps.get(&id).ok_or(Error::Resource(Type::Map, id))?;
        let key = map.range(MapKey(key)..).nth(1)
            .map_or(vm::Value::from(0.0), |(&MapKey(key), _)| key);
        Ok(key)
    }

    #[gml::function]
    pub fn ds_map_find_previous(&mut self, id: i32, key: vm::Value) ->
        Result<vm::Value, vm::ErrorKind>
    {
        let map = self.maps.get(&id).ok_or(Error::Resource(Type::Map, id))?;
        let key = map.range(..=MapKey(key)).rev().nth(1)
            .map_or(vm::Value::from(0.0), |(&MapKey(key), _)| key);
        Ok(key)
    }

    #[gml::function]
    pub fn ds_map_find_first(&mut self, id: i32) ->
        Result<vm::Value, vm::ErrorKind>
    {
        let map = self.maps.get(&id).ok_or(Error::Resource(Type::Map, id))?;
        let key = map.keys().nth(0).map_or(vm::Value::from(0.0), |&MapKey(key)| key);
        Ok(key)
    }

    #[gml::function]
    pub fn ds_map_find_last(&mut self, id: i32) ->
        Result<vm::Value, vm::ErrorKind>
    {
        let map = self.maps.get(&id).ok_or(Error::Resource(Type::Map, id))?;
        let key = map.keys().rev().nth(0).map_or(vm::Value::from(0.0), |&MapKey(key)| key);
        Ok(key)
    }
}
