use std::{mem, cmp, fmt, error};
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

    priorities: HashMap<i32, Priority>,
    next_priority: i32,

    grids: HashMap<i32, Grid>,
    next_grid: i32,
}

type List = Vec<vm::Value>;

type Map = BTreeMap<OrderedValue, vm::Value>;

#[derive(Default)]
struct Priority {
    data: Vec<vm::Value>,
    priorities: Vec<OrderedValue>,
}

#[derive(Clone, Eq, PartialEq)]
#[repr(transparent)]
struct OrderedValue(vm::Value);

impl cmp::PartialOrd for OrderedValue {
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        Some(OrderedValue::cmp(self, other))
    }
}

impl cmp::Ord for OrderedValue {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        let OrderedValue(ref a) = *self;
        let OrderedValue(ref b) = *other;
        match (a.borrow().decode(), b.borrow().decode()) {
            // This unwrap is fine because vm::Value should never be NaN.
            // TODO: this may no longer be true in GMS
            (vm::Data::Real(a), vm::Data::Real(b)) => f64::partial_cmp(&a, &b).unwrap(),
            (vm::Data::Real(_), vm::Data::Array(_)) => cmp::Ordering::Less,
            (vm::Data::Real(_), vm::Data::String(_)) => cmp::Ordering::Less,

            (vm::Data::Array(_), vm::Data::Real(_)) => cmp::Ordering::Greater,
            (vm::Data::Array(a), vm::Data::Array(b)) => {
                <*const _>::cmp(&a.as_raw(), &b.as_raw())
            }
            (vm::Data::Array(_), vm::Data::String(_)) => cmp::Ordering::Less,

            (vm::Data::String(_), vm::Data::Real(_)) => cmp::Ordering::Greater,
            (vm::Data::String(_), vm::Data::Array(_)) => cmp::Ordering::Greater,
            (vm::Data::String(a), vm::Data::String(b)) => Symbol::cmp(&a, &b),
        }
    }
}

impl OrderedValue {
    fn borrowed<'a>(value: &'a vm::ValueRef<'_>) -> &'a OrderedValue {
        // Safety: `OrderedValue` is `#[repr(transparent)]` and contains a single `vm::Value`.
        unsafe { mem::transmute::<&vm::Value, &OrderedValue>(value.as_ref()) }
    }
}

struct Grid {
    data: Box<[vm::Value]>,
    width: usize,
}

impl Grid {
    fn height(&self) -> usize {
        self.data.len() / self.width
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
    Priority,
    Grid,
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Type::List => write!(f, "list"),
            Type::Map => write!(f, "map"),
            Type::Priority => write!(f, "priority queue"),
            Type::Grid => write!(f, "grid"),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Error::Resource(kind, id) => {
                write!(f, "the {} with id {} does not exist", kind, id)?;
            }
            Error::KeyExists(ref key) => {
                write!(f, "an entry with key {:?} already exists in the map", key)?;
            }
        }
        Ok(())
    }
}

impl error::Error for Error {}

#[gml::bind]
impl State {
    // ds_list

    #[gml::api]
    pub fn ds_list_create(&mut self) -> i32 {
        let id = self.next_list;
        self.next_list += 1;
        self.lists.insert(id, List::default());
        id
    }

    #[gml::api]
    pub fn ds_list_destroy(&mut self, id: i32) -> vm::Result<()> {
        use self::hash_map::Entry;
        let entry = match self.lists.entry(id) {
            Entry::Occupied(entry) => entry,
            Entry::Vacant(_) => Err(Error::Resource(Type::List, id))?,
        };
        entry.remove();
        Ok(())
    }

    #[gml::api]
    pub fn ds_list_clear(&mut self, id: i32) -> vm::Result<()> {
        let list = self.lists.get_mut(&id).ok_or(Error::Resource(Type::List, id))?;
        list.clear();
        Ok(())
    }

    #[gml::api]
    pub fn ds_list_empty(&mut self, id: i32) -> vm::Result<bool> {
        let list = self.lists.get(&id).ok_or(Error::Resource(Type::List, id))?;
        let empty = list.is_empty();
        Ok(empty)
    }

    #[gml::api]
    pub fn ds_list_size(&mut self, id: i32) -> vm::Result<i32> {
        let list = self.lists.get(&id).ok_or(Error::Resource(Type::List, id))?;
        let size = list.len() as i32;
        Ok(size)
    }

    #[gml::api]
    pub fn ds_list_add(&mut self, id: i32, vals: &[vm::Value]) -> vm::Result<()> {
        let list = self.lists.get_mut(&id).ok_or(Error::Resource(Type::List, id))?;
        list.extend_from_slice(vals);
        Ok(())
    }

    #[gml::api]
    pub fn ds_list_delete(&mut self, id: i32, pos: i32) -> vm::Result<()> {
        let list = self.lists.get_mut(&id).ok_or(Error::Resource(Type::List, id))?;
        if pos < 0 || list.len() <= pos as usize {
            return Ok(());
        }
        list.remove(pos as usize);
        Ok(())
    }

    #[gml::api]
    pub fn ds_list_find_index(&mut self, id: i32, val: vm::ValueRef) -> vm::Result<i32> {
        let list = self.lists.get(&id).ok_or(Error::Resource(Type::List, id))?;
        let pos = list.iter()
            .position(move |e| e.borrow() == val)
            .map_or(-1, |i| i as i32);
        Ok(pos)
    }

    #[gml::api]
    pub fn ds_list_find_value(&mut self, id: i32, pos: i32) -> vm::Result<vm::Value> {
        let list = self.lists.get(&id).ok_or(Error::Resource(Type::List, id))?;
        if pos < 0 || list.len() <= pos as usize {
            return Ok(vm::Value::from(0));
        }
        let val = list.get(pos as usize).map_or(vm::Value::from(0), |val| val.clone());
        Ok(val)
    }

    #[gml::api]
    pub fn ds_list_insert(&mut self, id: i32, pos: i32, val: vm::ValueRef) -> vm::Result<()> {
        let list = self.lists.get_mut(&id).ok_or(Error::Resource(Type::List, id))?;
        if pos < 0 || list.len() < pos as usize {
            return Ok(());
        }
        list.insert(pos as usize, val.clone());
        Ok(())
    }

    #[gml::api]
    pub fn ds_list_replace(&mut self, id: i32, pos: i32, val: vm::ValueRef) -> vm::Result<()> {
        let list = self.lists.get_mut(&id).ok_or(Error::Resource(Type::List, id))?;
        if pos < 0 || list.len() <= pos as usize {
            return Ok(());
        }
        list[pos as usize] = val.clone();
        Ok(())
    }

    // ds_map

    #[gml::api]
    pub fn ds_map_create(&mut self) -> i32 {
        let id = self.next_map;
        self.next_map += 1;
        self.maps.insert(id, Map::default());
        id
    }

    #[gml::api]
    pub fn ds_map_destroy(&mut self, id: i32) -> vm::Result<()> {
        use self::hash_map::Entry;
        let entry = match self.maps.entry(id) {
            Entry::Occupied(entry) => entry,
            Entry::Vacant(_) => Err(Error::Resource(Type::Map, id))?,
        };
        entry.remove();
        Ok(())
    }

    #[gml::api]
    pub fn ds_map_clear(&mut self, id: i32) -> vm::Result<()> {
        let map = self.maps.get_mut(&id).ok_or(Error::Resource(Type::Map, id))?;
        map.clear();
        Ok(())
    }

    #[gml::api]
    pub fn ds_map_size(&mut self, id: i32) -> vm::Result<i32> {
        let map = self.maps.get(&id).ok_or(Error::Resource(Type::Map, id))?;
        let size = map.len() as i32;
        Ok(size)
    }

    #[gml::api]
    pub fn ds_map_empty(&mut self, id: i32) -> vm::Result<bool> {
        let map = self.maps.get(&id).ok_or(Error::Resource(Type::Map, id))?;
        let empty = map.is_empty();
        Ok(empty)
    }

    #[gml::api]
    pub fn ds_map_add(&mut self, id: i32, key: vm::ValueRef, val: vm::ValueRef) -> vm::Result<()> {
        use self::btree_map::Entry;
        let map = self.maps.get_mut(&id).ok_or(Error::Resource(Type::Map, id))?;
        let entry = match map.entry(OrderedValue(key.clone())) {
            Entry::Occupied(_) => Err(Error::KeyExists(key.clone()))?,
            Entry::Vacant(entry) => entry,
        };
        entry.insert(val.clone());
        Ok(())
    }

    #[gml::api]
    pub fn ds_map_replace(&mut self, id: i32, key: vm::ValueRef, val: vm::ValueRef) ->
        vm::Result<()>
    {
        use self::btree_map::Entry;
        let map = self.maps.get_mut(&id).ok_or(Error::Resource(Type::Map, id))?;
        let mut entry = match map.entry(OrderedValue(key.clone())) {
            Entry::Occupied(entry) => entry,
            Entry::Vacant(_) => return Ok(()),
        };
        entry.insert(val.clone());
        Ok(())
    }

    #[gml::api]
    pub fn ds_map_delete(&mut self, id: i32, key: vm::ValueRef) -> vm::Result<()> {
        let map = self.maps.get_mut(&id).ok_or(Error::Resource(Type::Map, id))?;
        map.remove(OrderedValue::borrowed(&key));
        Ok(())
    }

    #[gml::api]
    pub fn ds_map_exists(&mut self, id: i32, key: vm::ValueRef) -> vm::Result<()> {
        let map = self.maps.get(&id).ok_or(Error::Resource(Type::Map, id))?;
        map.contains_key(OrderedValue::borrowed(&key));
        Ok(())
    }

    #[gml::api]
    pub fn ds_map_find_value(&mut self, id: i32, key: vm::ValueRef) -> vm::Result<vm::Value> {
        let map = self.maps.get(&id).ok_or(Error::Resource(Type::Map, id))?;
        let val = map.get(OrderedValue::borrowed(&key)).map_or(vm::Value::from(0), |val| val.clone());
        Ok(val)
    }

    #[gml::api]
    pub fn ds_map_find_next(&mut self, id: i32, key: vm::ValueRef) -> vm::Result<vm::Value> {
        let map = self.maps.get(&id).ok_or(Error::Resource(Type::Map, id))?;
        let key = map.range(OrderedValue::borrowed(&key)..).nth(1)
            .map_or(vm::Value::from(0.0), |(&OrderedValue(ref key), _)| key.clone());
        Ok(key)
    }

    #[gml::api]
    pub fn ds_map_find_previous(&mut self, id: i32, key: vm::ValueRef) -> vm::Result<vm::Value> {
        let map = self.maps.get(&id).ok_or(Error::Resource(Type::Map, id))?;
        let key = map.range(..=OrderedValue::borrowed(&key)).rev().nth(1)
            .map_or(vm::Value::from(0.0), |(&OrderedValue(ref key), _)| key.clone());
        Ok(key)
    }

    #[gml::api]
    pub fn ds_map_find_first(&mut self, id: i32) -> vm::Result<vm::Value> {
        let map = self.maps.get(&id).ok_or(Error::Resource(Type::Map, id))?;
        let key = map.keys().nth(0).map_or(vm::Value::from(0.0), |&OrderedValue(ref key)| key.clone());
        Ok(key)
    }

    #[gml::api]
    pub fn ds_map_find_last(&mut self, id: i32) -> vm::Result<vm::Value> {
        let map = self.maps.get(&id).ok_or(Error::Resource(Type::Map, id))?;
        let key = map.keys().rev().nth(0).map_or(vm::Value::from(0.0), |&OrderedValue(ref key)| key.clone());
        Ok(key)
    }

    // ds_priority

    #[gml::api]
    pub fn ds_priority_create(&mut self) -> i32 {
        let id = self.next_priority;
        self.next_priority += 1;
        self.priorities.insert(id, Priority::default());
        id
    }

    #[gml::api]
    pub fn ds_priority_destroy(&mut self, id: i32) -> vm::Result<()> {
        use self::hash_map::Entry;
        let entry = match self.priorities.entry(id) {
            Entry::Occupied(entry) => entry,
            Entry::Vacant(_) => Err(Error::Resource(Type::Priority, id))?,
        };
        entry.remove();
        Ok(())
    }

    #[gml::api]
    pub fn ds_priority_clear(&mut self, id: i32) -> vm::Result<()> {
        let queue = self.priorities.get_mut(&id).ok_or(Error::Resource(Type::Priority, id))?;
        queue.data.clear();
        queue.priorities.clear();
        Ok(())
    }

    #[gml::api]
    pub fn ds_priority_empty(&mut self, id: i32) -> vm::Result<bool> {
        let queue = self.priorities.get(&id).ok_or(Error::Resource(Type::Priority, id))?;
        let empty = queue.data.is_empty();
        Ok(empty)
    }

    #[gml::api]
    pub fn ds_priority_size(&mut self, id: i32) -> vm::Result<i32> {
        let queue = self.priorities.get(&id).ok_or(Error::Resource(Type::Priority, id))?;
        let size = queue.data.len() as i32;
        Ok(size)
    }

    #[gml::api]
    pub fn ds_priority_add(
        &mut self, id: i32, val: vm::ValueRef<'_>, prio: vm::ValueRef<'_>
    ) -> vm::Result<()> {
        let queue = self.priorities.get_mut(&id).ok_or(Error::Resource(Type::Priority, id))?;
        queue.data.push(val.clone());
        queue.priorities.push(OrderedValue(prio.clone()));
        Ok(())
    }

    #[gml::api]
    pub fn ds_priority_change_priority(
        &mut self, id: i32, val: vm::ValueRef<'_>, prio: vm::ValueRef<'_>
    ) -> vm::Result<()> {
        let queue = self.priorities.get_mut(&id).ok_or(Error::Resource(Type::Priority, id))?;
        let Some(i) = queue.data.iter().position(move |e| e.borrow() == val) else {
            return Ok(())
        };
        queue.priorities[i] = OrderedValue(prio.clone());
        Ok(())
    }

    #[gml::api]
    pub fn ds_priority_find_priority(
        &mut self, id: i32, val: vm::ValueRef<'_>
    ) -> vm::Result<vm::Value> {
        let queue = self.priorities.get_mut(&id).ok_or(Error::Resource(Type::Priority, id))?;
        let val = queue.data.iter()
            .find(move |e| e.borrow() == val)
            .map(|e| e.clone())
            .unwrap_or_else(vm::Value::default);
        Ok(val)
    }

    #[gml::api]
    pub fn ds_priority_delete_value(&mut self, id: i32, val: vm::ValueRef<'_>) -> vm::Result<()> {
        let queue = self.priorities.get_mut(&id).ok_or(Error::Resource(Type::Priority, id))?;
        let Some(i) = queue.data.iter().position(move |e| e.borrow() == val) else {
            return Ok(())
        };
        queue.data.remove(i);
        queue.priorities.remove(i);
        Ok(())
    }

    #[gml::api]
    pub fn ds_priority_delete_min(&mut self, id: i32) -> vm::Result<vm::Value> {
        let queue = self.priorities.get_mut(&id).ok_or(Error::Resource(Type::Priority, id))?;
        let Some((i, _)) = queue.priorities.iter().enumerate().min_by_key(|&(_, e)| e) else {
            return Ok(vm::Value::default())
        };
        let val = queue.data.remove(i);
        queue.priorities.remove(i);
        Ok(val)
    }

    #[gml::api]
    pub fn ds_priority_find_min(&mut self, id: i32) -> vm::Result<vm::Value> {
        let queue = self.priorities.get_mut(&id).ok_or(Error::Resource(Type::Priority, id))?;
        let Some((_, &OrderedValue(ref val))) = queue.priorities.iter().enumerate()
            .min_by_key(|&(_, e)| e)
        else {
            return Ok(vm::Value::default())
        };
        Ok(val.clone())
    }

    #[gml::api]
    pub fn ds_priority_delete_max(&mut self, id: i32) -> vm::Result<vm::Value> {
        let queue = self.priorities.get_mut(&id).ok_or(Error::Resource(Type::Priority, id))?;
        let Some((i, _)) = queue.priorities.iter().enumerate().max_by_key(|&(_, e)| e) else {
            return Ok(vm::Value::default())
        };
        let val = queue.data.remove(i);
        queue.priorities.remove(i);
        Ok(val)
    }

    #[gml::api]
    pub fn ds_priority_find_max(&mut self, id: i32) -> vm::Result<vm::Value> {
        let queue = self.priorities.get_mut(&id).ok_or(Error::Resource(Type::Priority, id))?;
        let Some((_, &OrderedValue(ref val))) = queue.priorities.iter().enumerate()
            .max_by_key(|&(_, e)| e)
        else {
            return Ok(vm::Value::default())
        };
        Ok(val.clone())
    }

    // ds_grid

    #[gml::api]
    pub fn ds_grid_create(&mut self, w: u32, h: u32) -> i32 {
        let id = self.next_grid;
        self.next_grid += 1;
        let data = vec![vm::Value::from(0.0); w as usize * h as usize].into_boxed_slice();
        let width = w as usize;
        self.grids.insert(id, Grid { data, width });
        id
    }

    #[gml::api]
    pub fn ds_grid_destroy(&mut self, id: i32) -> vm::Result<()> {
        use self::hash_map::Entry;
        let entry = match self.grids.entry(id) {
            Entry::Occupied(entry) => entry,
            Entry::Vacant(_) => Err(Error::Resource(Type::Grid, id))?,
        };
        entry.remove();
        Ok(())
    }

    #[gml::api]
    pub fn ds_grid_resize(&mut self, id: i32, w: u32, h: u32) -> vm::Result<()> {
        let grid = self.grids.get_mut(&id).ok_or(Error::Resource(Type::Grid, id))?;

        let new_data = vec![vm::Value::from(0.0); w as usize * h as usize].into_boxed_slice();
        let new_width = w as usize;

        let old_data = mem::replace(&mut grid.data, new_data);
        let old_width = mem::replace(&mut grid.width, new_width);

        let new_rows = grid.data.chunks_exact_mut(grid.width);
        let old_rows = old_data.chunks_exact(old_width);

        let copy_width = cmp::min(new_width, old_width);
        for (new, old) in Iterator::zip(new_rows, old_rows) {
            new[..copy_width].clone_from_slice(&old[..copy_width]);
        }

        Ok(())
    }

    #[gml::api]
    pub fn ds_grid_width(&mut self, id: i32) -> vm::Result<u32> {
        let grid = self.grids.get(&id).ok_or(Error::Resource(Type::Grid, id))?;
        Ok(grid.width as u32)
    }

    #[gml::api]
    pub fn ds_grid_height(&mut self, id: i32) -> vm::Result<u32> {
        let grid = self.grids.get(&id).ok_or(Error::Resource(Type::Grid, id))?;
        Ok(grid.height() as u32)
    }

    #[gml::api]
    pub fn ds_grid_clear(&mut self, id: i32, val: vm::ValueRef) -> vm::Result<()> {
        let grid = self.grids.get_mut(&id).ok_or(Error::Resource(Type::Grid, id))?;
        for cell in &mut *grid.data {
            *cell = val.clone();
        }
        Ok(())
    }

    #[gml::api]
    pub fn ds_grid_set(&mut self, id: i32, x: u32, y: u32, val: vm::ValueRef) -> vm::Result<()> {
        let grid = self.grids.get_mut(&id).ok_or(Error::Resource(Type::Grid, id))?;
        if grid.width <= x as usize {
            return Ok(());
        }
        if grid.height() <= y as usize {
            return Ok(());
        }
        let index = y as usize * grid.width + x as usize;
        grid.data[index] = val.clone();
        Ok(())
    }

    #[gml::api]
    pub fn ds_grid_get(&mut self, id: i32, x: u32, y: u32) -> vm::Result<vm::Value> {
        let grid = self.grids.get(&id).ok_or(Error::Resource(Type::Grid, id))?;
        if grid.width <= x as usize {
            return Ok(vm::Value::from(0.0));
        }
        if grid.height() <= y as usize {
            return Ok(vm::Value::from(0.0));
        }
        let index = y as usize * grid.width + x as usize;
        Ok(grid.data[index].clone())
    }
}
