use std::mem;
use std::ptr;

use std::marker::Copy;
use std::clone::Clone;

use std::ops::Deref;
use std::ops::DerefMut;

use std::sync::{RwLock, RwLockWriteGuard};
use std::sync::atomic::{AtomicUsize, Ordering};

use std::fmt::{Debug, Formatter, Result};

use super::super::round_up_to_next_highest_power_of_two;

struct Bucket {
    key: Option<i32>,
    value: Option<i32>,
    next: Option<Link>
}

impl Bucket {

    fn empty() -> Bucket {
        Bucket {
            key: None,
            value: None,
            next: None
        }
    }
    
    fn new(key: i32, value: i32) -> Bucket {
        Bucket {
            key: Some(key),
            value: Some(value),
            next: None
        }
    }
}

impl Debug for Bucket {

    fn fmt(&self, fmt: &mut Formatter) -> Result {
        write!(fmt, "[ Key = {:?} Value = {:?} ]", self.key, self.value)
    }
}

struct Link {

    ptr: *mut Bucket
}

impl Link {

    fn new(bucket: Bucket) -> Link {
        Link {
            ptr: Box::into_raw(Box::new(bucket))
        }
    }
}

impl Deref for Link {
    type Target = Bucket;

    fn deref(&self) -> &Bucket {
        unsafe { mem::transmute(self.ptr) }
    }
}

impl DerefMut for Link {

    fn deref_mut(&mut self) -> &mut Bucket {
        unsafe { mem::transmute(self.ptr) }
    }
}

impl Clone for Link{

    fn clone(&self) -> Link {
        Link { ptr: self.ptr }
    }
}

impl Copy for Link { }
unsafe impl Send for Link { }

pub struct ConcurrentHashMap {
    table: Vec<RwLock<Link>>,
    size: AtomicUsize
}

impl ConcurrentHashMap {
    
    pub fn new() -> ConcurrentHashMap {
        ConcurrentHashMap::with_capacity(16)
    }

    pub fn with_capacity(capacity: usize) -> ConcurrentHashMap {
        let capacity = round_up_to_next_highest_power_of_two(capacity);
        let mut table = Vec::with_capacity(capacity);
        for _ in 0..capacity {
            table.push(RwLock::new(Link::new(Bucket::empty())));
        }
        ConcurrentHashMap {
            table: table,
            size: AtomicUsize::new(0)
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn len(&self) -> usize {
        self.size.load(Ordering::Relaxed)
    }

    pub fn capacity(&self) -> usize {
        self.table.capacity()
    }

    pub fn insert(&mut self, key: i32, val: i32) {
        let index = self.capacity() & key as usize;
        let mut guard = self.table[index].write().unwrap();
        if put(key, val, &mut guard) {
            self.size.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub fn remove(&mut self, key: i32) -> Option<i32> {
        let index = self.capacity() & key as usize;
        let mut guard = self.table[index].write().unwrap();
        let result = take(key, &mut guard);
        println!("{:?}", result);
        if result.is_some() {
            self.size.fetch_sub(1, Ordering::Relaxed);
        }
        result
    }
}

fn put(key: i32, val: i32, guard: &mut RwLockWriteGuard<Link>) -> bool {
    let contains = contains(key, guard);
    if contains {
        let mut link = **guard;
        loop {
            if (*link).key == Some(key) {
                link.value = Some(val);
                break;
            }
            match (*link).next {
                Some(next) => link = next,
                None => break,
            }
        }
    }
    else {
        let mut new_bucket = Link::new(Bucket::new(key, val));
        let link = **guard;
        new_bucket.next = Some(link);
        **guard = new_bucket;
    }
    !contains
}

fn contains(key: i32, guard: &RwLockWriteGuard<Link>) -> bool {
    let mut link = **guard;
    loop {
        if (*link).key == Some(key) {
            return true;
        }
        match (*link).next {
            Some(next) => link = next,
            None => return false,
        }
    }
}

fn take(key: i32, guard: &mut RwLockWriteGuard<Link>) -> Option<i32> {
    let mut link = **guard;
    if !link.ptr.is_null() {
        loop {
            if (*link).key == Some(key) {
                match (*link).next {
                    Some(next) => link.next = next.next,
                    None => link.ptr = ptr::null_mut(),
                }
                return (*link).value;
            }
            match (*link).next {
                Some(next) => link = next,
                None => break,
            }
        }
    }
    None
}
