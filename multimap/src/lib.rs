// This is just a hacked-up multimap. Eventually, we'll need to move to a fully persistent (in the
// functional-data-structure sense), on-disk multimap.

use serde::de::{SeqAccess, Visitor};
use serde::ser::SerializeSeq;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::borrow::Borrow;
use std::collections::{BTreeMap, BTreeSet};

// FIXME: the derived PartialEq is not correct, because of empty sets.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MMap<K: Ord, V: Ord> {
    map: BTreeMap<K, BTreeSet<V>>,
    // hackity
    empty_set: BTreeSet<V>,
}

impl<K: Ord, V: Ord> MMap<K, V> {
    pub fn new() -> MMap<K, V> {
        MMap {
            map: BTreeMap::new(),
            empty_set: BTreeSet::new(),
        }
    }

    /// Returns an iterator over all the values associated with this key.
    pub fn get<Q>(&self, key: &Q) -> impl Iterator<Item = &V>
    where
        K: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        self.map.get(key).unwrap_or(&self.empty_set).iter()
    }

    pub fn insert(&mut self, key: K, val: V) {
        self.map
            .entry(key)
            .or_insert_with(BTreeSet::new)
            .insert(val);
    }

    pub fn remove<Q, R>(&mut self, key: &Q, val: &R) -> bool
    where
        K: Borrow<Q>,
        Q: Ord + ?Sized,
        V: Borrow<R>,
        R: Ord + ?Sized,
    {
        if let Some(set) = self.map.get_mut(&key) {
            let ret = set.remove(val);
            // Remove empty sets entirely. Partly because it seems reasonable to get rid of unused
            // entries, but mostly because it makes the auto-derived PartialEq implementation
            // correct.
            if set.is_empty() {
                self.map.remove(key);
            }
            ret
        } else {
            false
        }
    }

    pub fn remove_all<Q>(&mut self, key: &Q)
    where
        K: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        self.map.remove(key);
    }

    pub fn contains<Q, R>(&self, key: &Q, val: &R) -> bool
    where
        K: Borrow<Q>,
        Q: Ord + ?Sized,
        V: Borrow<R>,
        R: Ord + ?Sized,
    {
        self.map
            .get(key)
            .and_then(|bindings| bindings.get(val))
            .is_some()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        self.map
            .iter()
            .flat_map(|(k, vs)| vs.iter().map(move |v| (k, v)))
    }
}

impl<K: Ord + Serialize, V: Ord + Serialize> Serialize for MMap<K, V> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut seq = serializer.serialize_seq(None)?;
        for (k, v) in self.iter() {
            seq.serialize_element(&(k, v))?;
        }
        seq.end()
    }
}

impl<'de, K: Ord + Deserialize<'de>, V: Ord + Deserialize<'de>> Deserialize<'de> for MMap<K, V> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_seq(MMapVisitor {
            x: std::marker::PhantomData,
        })
    }
}

struct MMapVisitor<K, V> {
    x: std::marker::PhantomData<(K, V)>,
}

impl<'de, K: Ord + Deserialize<'de>, V: Ord + Deserialize<'de>> Visitor<'de> for MMapVisitor<K, V> {
    type Value = MMap<K, V>;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "a sequence of tuples")
    }

    fn visit_seq<S: SeqAccess<'de>>(self, mut access: S) -> Result<Self::Value, S::Error> {
        let mut ret = MMap::new();
        while let Some((key, val)) = access.next_element()? {
            ret.insert(key, val);
        }
        Ok(ret)
    }
}

#[cfg(test)]
mod tests {
    use super::MMap;

    #[test]
    fn get_empty() {
        let mut map = MMap::new();
        assert!(map.get(&1).next().is_none());
        map.insert(1, 2);
        assert!(map.get(&1).next().is_some());
        assert!(map.get(&2).next().is_none());
    }

    #[test]
    fn get_many() {
        let mut map = MMap::new();
        map.insert(1, 2);
        map.insert(1, 3);
        map.insert(1, 2);
        map.insert(1, 1);
        assert_eq!(map.get(&1).cloned().collect::<Vec<_>>(), vec![1, 2, 3]);
    }

    #[test]
    fn contains() {
        let mut map = MMap::new();
        map.insert(1, 2);
        map.insert(1, 3);
        assert!(map.contains(&1, &2));
        assert!(!map.contains(&2, &1));
        assert!(!map.contains(&1, &4));
    }

    #[test]
    fn serde() {
        let mut map = MMap::new();
        map.insert(1, 2);
        map.insert(1, 3);

        let mut buf = Vec::new();
        serde_yaml::to_writer(&mut buf, &map).unwrap();
        let map2: MMap<_, _> = serde_yaml::from_reader(&buf[..]).unwrap();
        assert_eq!(map, map2);
    }
}
