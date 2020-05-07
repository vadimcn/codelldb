use std::fmt;
use std::marker::PhantomData;

use serde::de::{Deserialize, Deserializer, MapAccess, Visitor};
use serde::ser::{Serialize, SerializeMap, SerializeSeq, Serializer};

// Preserves the order of entries when deserializing from JSON.
#[derive(Clone, Debug)]
pub struct VecMap<K, V>(Vec<(K, V)>);

impl<K, V> VecMap<K, V> {
    pub fn iter(&self) -> impl Iterator<Item = &(K, V)> {
        self.0.iter()
    }
}

struct VecMapVisitor<K, V>(PhantomData<(K, V)>);

impl<'de, K, V> Visitor<'de> for VecMapVisitor<K, V>
where
    K: Deserialize<'de>,
    V: Deserialize<'de>,
{
    type Value = VecMap<K, V>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a map")
    }

    fn visit_map<M>(self, mut access: M) -> Result<Self::Value, M::Error>
    where
        M: MapAccess<'de>,
    {
        let mut vec = Vec::with_capacity(access.size_hint().unwrap_or(0));
        while let Some((key, value)) = access.next_entry()? {
            vec.push((key, value));
        }
        Ok(VecMap(vec))
    }
}

impl<'de, K, V> Deserialize<'de> for VecMap<K, V>
where
    K: Deserialize<'de>,
    V: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(VecMapVisitor(PhantomData))
    }
}

impl<K, V> Serialize for VecMap<K, V>
where
    K: Serialize,
    V: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(self.0.len()))?;
        for (k, v) in self.iter() {
            map.serialize_entry(k, v)?;
        }
        map.end()
    }
}
