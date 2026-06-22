use crate::{error::FluxError, utils::impl_serde_repr_str};

use super::resource_count::ResourceCount;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(try_from = "RawResourceVertex")]
pub struct ResourceVertex {
    #[serde(rename = "type")]
    pub resource_type: String,
    pub count: ResourceCount,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exclusive: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub with: Option<Vec<ResourceVertex>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}

impl_serde_repr_str!(ResourceVertex);

/// Internal duplicate of ResourceVertex for validation
#[derive(Deserialize)]
pub struct RawResourceVertex {
    resource_type: String,
    count: ResourceCount,
    unit: Option<String>,
    exclusive: Option<bool>,
    with: Option<Vec<ResourceVertex>>,
    label: Option<String>,
    id: Option<String>,
}

impl TryFrom<RawResourceVertex> for ResourceVertex {
    type Error = FluxError;

    fn try_from(value: RawResourceVertex) -> std::result::Result<Self, Self::Error> {
        if let Some(with_vec) = &value.with {
            if with_vec.is_empty() {
                return Err(FluxError::Logic(
                    "The 'with' field must contain at least one element if present".to_string(),
                ));
            }
        }
        if value.resource_type == "slot" && value.label.is_none() {
            return Err(FluxError::Logic(
                "When the 'type' is 'slot', the 'label' field must be provided".to_string(),
            ));
        }
        Ok(Self {
            resource_type: value.resource_type,
            count: value.count,
            unit: value.unit,
            exclusive: value.exclusive,
            with: value.with,
            label: value.label,
            id: value.id,
        })
    }
}
