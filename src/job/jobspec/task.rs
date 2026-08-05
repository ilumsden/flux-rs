use std::collections::HashMap;

use serde::de::{self, MapAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::utils::impl_serde_repr_str;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct TaskCountPerResource {
    #[serde(rename = "type")]
    pub resource_type: String,
    pub count: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TaskCount {
    PerSlot(u64),
    PerResource(TaskCountPerResource),
    Total(u64),
    Extension(String, Value),
}

#[derive(Serialize, Deserialize)]
pub struct Task {
    pub command: Vec<String>,
    pub slot: String,
    pub count: TaskCount,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attributes: Option<HashMap<String, Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub distribution: Option<String>,
}

impl<'de> Deserialize<'de> for TaskCount {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct CountVisitor;

        impl<'de> Visitor<'de> for CountVisitor {
            type Value = TaskCount;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a dictionary with exactly 1 key")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let key: String = match map.next_key()? {
                    Some(k) => k,
                    None => {
                        return Err(de::Error::custom(
                            "'count' field must have exactly one key set",
                        ))
                    }
                };
                let result = match key.as_str() {
                    "per_slot" => {
                        let val = map.next_value()?;
                        if val == 0 {
                            return Err(de::Error::custom(
                                "When 'per_slot' is specified in 'count', its value must be > 0",
                            ));
                        }
                        TaskCount::PerSlot(val)
                    }
                    "per_resource" => TaskCount::PerResource(map.next_value()?),
                    "total" => {
                        let val = map.next_value()?;
                        if val == 0 {
                            return Err(de::Error::custom(
                                "When 'total' is specified in 'count', its value must be > 0",
                            ));
                        }
                        TaskCount::Total(val)
                    }
                    _ => TaskCount::Extension(key, map.next_value()?),
                };
                if map.next_key::<serde::de::IgnoredAny>()?.is_some() {
                    return Err(de::Error::custom(
                        "'count' field must have exactly one key set",
                    ));
                }
                Ok(result)
            }
        }

        deserializer.deserialize_map(CountVisitor)
    }
}

impl Serialize for TaskCount {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut map = serializer.serialize_map(Some(1))?;
        match self {
            TaskCount::PerSlot(v) => map.serialize_entry("per_slot", v)?,
            TaskCount::PerResource(v) => map.serialize_entry("per_resource", v)?,
            TaskCount::Total(v) => map.serialize_entry("total", v)?,
            TaskCount::Extension(k, v) => map.serialize_entry(k, v)?,
        }
        map.end()
    }
}

impl_serde_repr_str!(no_debug TaskCountPerResource);
impl_serde_repr_str!(no_debug TaskCount);
impl_serde_repr_str!(Task);
