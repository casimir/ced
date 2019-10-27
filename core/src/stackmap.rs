use std::collections::HashMap;
use std::fmt;
use std::hash::Hash;
use std::ops::{Deref, DerefMut};

#[derive(Debug)]
pub struct InvalidOp {
    reason: &'static str,
}

impl fmt::Display for InvalidOp {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "invalid operation: {}", self.reason)
    }
}

impl std::error::Error for InvalidOp {}

pub struct StackMap<K, V> {
    stack: Vec<K>,
    map: HashMap<K, V>,
}

impl<K, V> StackMap<K, V>
where
    K: Clone + Eq + Hash,
{
    pub fn new() -> StackMap<K, V> {
        StackMap {
            stack: Vec::new(),
            map: HashMap::new(),
        }
    }

    pub fn latest(&self) -> Option<&K> {
        self.stack.first()
    }

    pub fn latest_value(&self) -> Option<&V> {
        let idx = self.stack.first()?;
        self.map.get(idx)
    }

    pub fn set_last(&mut self, k: K) -> Result<(), InvalidOp> {
        if !self.map.contains_key(&k) {
            return Err(InvalidOp {
                reason: "key does not exist",
            });
        }
        if let Some(idx) = self.stack.iter().position(|ref e| **e == k) {
            self.stack.remove(idx);
        }
        self.stack.insert(0, k);
        Ok(())
    }

    pub fn insert(&mut self, k: K, v: V) -> Option<V> {
        let kp = k.clone();
        let old = self.map.insert(k, v);
        self.set_last(kp).unwrap();
        old
    }

    pub fn remove(&mut self, k: &K) -> Option<V> {
        if let Some(idx) = self.stack.iter().position(|ref e| **e == *k) {
            self.stack.remove(idx);
            self.map.remove(&k)
        } else {
            None
        }
    }
}

impl<K, V> Deref for StackMap<K, V> {
    type Target = HashMap<K, V>;

    fn deref(&self) -> &Self::Target {
        &self.map
    }
}

impl<K, V> DerefMut for StackMap<K, V> {
    fn deref_mut(&mut self) -> &mut HashMap<K, V> {
        &mut self.map
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lifecycle() {
        let mut sm: StackMap<usize, &str> = StackMap::new();
        assert!(sm.is_empty());
        assert!(sm.latest().is_none());
        assert!(sm.latest_value().is_none());
        assert!(sm.set_last(1).is_err());
        sm.insert(1, "1");
        assert!(sm.latest() == Some(&1));
        assert!(sm.latest_value() == Some(&"1"));
        sm.insert(2, "2");
        sm.insert(3, "3");
        assert!(sm.latest() == Some(&3));
        assert!(sm.set_last(2).is_ok());
        assert!(sm.latest() == Some(&2));
        sm.remove(&2);
        assert!(sm.latest() == Some(&3));
    }
}
