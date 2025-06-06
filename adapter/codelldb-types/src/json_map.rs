use std::borrow::Cow;
use std::convert::TryFrom;
use std::fmt;
use std::marker::PhantomData;

use schemars::{json_schema, JsonSchema, Schema, SchemaGenerator};
use serde::de::{Deserialize, Deserializer, MapAccess, Visitor};
use serde::ser::{Serialize, SerializeMap, Serializer};

// Preserves the order of entries when deserializing from JSON.
#[derive(Clone, Debug)]
pub struct JsonMap<V>(Vec<(String, V)>);

impl<V> JsonMap<V> {
    pub fn iter(&self) -> impl Iterator<Item = &(String, V)> {
        self.0.iter()
    }
}

struct VecMapVisitor<V>(PhantomData<(String, V)>);

impl<'de, V> Visitor<'de> for VecMapVisitor<V>
where
    V: Deserialize<'de>,
{
    type Value = JsonMap<V>;

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
        Ok(JsonMap(vec))
    }
}

impl<'de, V> Deserialize<'de> for JsonMap<V>
where
    V: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(VecMapVisitor(PhantomData))
    }
}

impl<V> Serialize for JsonMap<V>
where
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

impl<V: JsonSchema> JsonSchema for JsonMap<V> {
    fn schema_name() -> Cow<'static, str> {
        format!("JsonMap_of_{}", <V as JsonSchema>::schema_name()).into()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": "object",
            "additionalProperties": generator.subschema_for::<V>()
        })
    }
}
