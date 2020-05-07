use log::{debug, error, info};
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::mem;
use std::num::NonZeroU32;
use std::rc::Rc;

use crate::error::Error;

pub type Handle = NonZeroU32;

pub fn to_i64(h: Option<Handle>) -> i64 {
    match h {
        None => 0,
        Some(v) => v.get() as i64,
    }
}

pub fn from_i64(v: i64) -> Result<Handle, Error> {
    match Handle::new(v as u32) {
        Some(h) => Ok(h),
        None => Err("Expected non-zero handle value".into()),
    }
}

pub struct HandleTree<Value> {
    obj_by_handle: HashMap<Handle, (Option<Handle>, Rc<String>, Value)>,
    handle_tree: HashMap<(Option<Handle>, Rc<String>), Handle>,
    prev_handle_tree: HashMap<(Option<Handle>, Rc<String>), Handle>,
    next_handle_value: u32,
}

impl<Value> HandleTree<Value> {
    pub fn new() -> Self {
        HandleTree {
            obj_by_handle: HashMap::new(),
            handle_tree: HashMap::new(),
            prev_handle_tree: HashMap::new(),
            next_handle_value: 1000,
        }
    }

    pub fn reset(&mut self) {
        self.obj_by_handle.clear();
        self.prev_handle_tree.clear();
        mem::swap(&mut self.handle_tree, &mut self.prev_handle_tree);
    }

    pub fn create(&mut self, parent_handle: Option<Handle>, key: &str, value: Value) -> Handle {
        if let Some(ph) = parent_handle {
            assert!(self.obj_by_handle.contains_key(&ph))
        }

        let key = Rc::new(key.to_owned());
        let mut new_handle = match self.prev_handle_tree.get(&(parent_handle, key.clone())).to_owned() {
            Some(h) => *h,
            None => self.create_new_handle(),
        };

        let triple = (parent_handle, key.clone(), value);
        match self.obj_by_handle.entry(new_handle) {
            Entry::Vacant(ent) => {
                ent.insert(triple);
            }
            Entry::Occupied(_) => {
                error!("Parent/key combination is not unique ({:?}/{})", parent_handle, key);
                new_handle = self.create_new_handle();
                self.obj_by_handle.insert(new_handle, triple);
            }
        }
        self.handle_tree.insert((parent_handle, key), new_handle);
        new_handle
    }

    fn create_new_handle(&mut self) -> Handle {
        self.next_handle_value += 1;
        Handle::new(self.next_handle_value).unwrap()
    }

    pub fn get(&self, handle: Handle) -> Option<&Value> {
        match self.obj_by_handle.get(&handle) {
            Some(v) => Some(&v.2),
            None => None,
        }
    }

    pub fn get_full_info(&self, handle: Handle) -> Option<(Option<Handle>, &str, &Value)> {
        match self.obj_by_handle.get(&handle) {
            Some(v) => Some((v.0, &v.1, &v.2)),
            None => None,
        }
    }
}

#[test]
fn test1() {
    let mut handles = HandleTree::new();
    let a1 = handles.create(None, "1", 0xa1);
    let a2 = handles.create(None, "2", 0xa2);
    let a11 = handles.create(Some(a1), "1.1", 0xa11);
    let a12 = handles.create(Some(a1), "1.2", 0xa12);
    let a121 = handles.create(Some(a12), "1.2.1", 0xa121);
    let a21 = handles.create(Some(a2), "2.1", 0xa21);

    assert_eq!(handles.get(a1).unwrap(), &0xa1);
    assert_eq!(handles.get(a12).unwrap(), &0xa12);
    assert_eq!(handles.get(a121).unwrap(), &0xa121);

    handles.reset();
    let b1 = handles.create(None, "1", 0xb1);
    let b3 = handles.create(None, "3", 0xb3);
    let b11 = handles.create(Some(b1), "1.1", 0xb11);
    let b12 = handles.create(Some(b1), "1.2", 0xb12);
    let b13 = handles.create(Some(b1), "1.3", 0xb13);
    let b121 = handles.create(Some(b12), "1.2.1", 0xb121);
    let b122 = handles.create(Some(b12), "1.2.2", 0xb122);

    assert_eq!(handles.get(a2), None);
    assert_eq!(handles.get(a21), None);

    assert_eq!(b1, a1);
    assert_eq!(b11, a11);
    assert_eq!(b12, a12);
    assert_eq!(b121, a121);

    assert_eq!(handles.get(b1).unwrap(), &0xb1);
    assert_eq!(handles.get(b122).unwrap(), &0xb122);
}

#[test]
#[should_panic]
fn test2() {
    let mut handles = HandleTree::new();
    let h1 = handles.create(None, "12345", 12345);
    // Should panic because parent handle is invalid
    let h2 = handles.create(Some(Handle::new(h1.get() + 1).unwrap()), "12345", 12345);
}
