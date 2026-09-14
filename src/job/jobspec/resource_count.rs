use serde::{Deserialize, Serialize};

use crate::error::{FluxError, Result};
use crate::idset::Idset;
use crate::utils::impl_serde_repr_str;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceCountOperator {
    #[serde(rename = "+")]
    Plus,
    #[serde(rename = "*")]
    Multipy,
    #[serde(rename = "^")]
    Exponentiate,
}

#[derive(Serialize, Deserialize, Copy, Clone)]
#[serde(try_from = "RawResourceCountDict")]
pub struct ResourceCountDict {
    pub min: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operator: Option<ResourceCountOperator>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operand: Option<usize>,
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
pub enum ResourceCount {
    Integer(usize),
    Idset(Idset),
    Dict(ResourceCountDict),
}

impl ResourceCount {
    pub fn try_clone(&self) -> Result<ResourceCount> {
        match self {
            ResourceCount::Integer(i) => Ok(ResourceCount::Integer(*i)),
            ResourceCount::Idset(idset) => Ok(ResourceCount::Idset(idset.try_clone()?)),
            ResourceCount::Dict(d) => Ok(ResourceCount::Dict(*d)),
        }
    }
}

impl Clone for ResourceCount {
    fn clone(&self) -> Self {
        self.try_clone()
            .expect("Failed to clone the ResourceCount. This means that cloning an Idset failed.")
    }
}

impl_serde_repr_str!(no_debug ResourceCountOperator);
impl_serde_repr_str!(ResourceCountDict);
impl_serde_repr_str!(ResourceCount);

/// Internal duplicate of ResourceCountDict for validation
#[derive(Deserialize)]
struct RawResourceCountDict {
    min: usize,
    max: Option<usize>,
    operator: Option<ResourceCountOperator>,
    operand: Option<usize>,
}

impl TryFrom<RawResourceCountDict> for ResourceCountDict {
    type Error = FluxError;

    fn try_from(raw: RawResourceCountDict) -> std::result::Result<Self, Self::Error> {
        // Max must be >= min if  specified
        if let Some(max_val) = &raw.max
            && *max_val < raw.min
        {
            return Err(FluxError::Logic("The 'max' value for the 'count' field must be greater than or equal to the 'min' value".to_string()));
        }
        // Several rules apply if 'operator' is specified
        if let Some(operator) = &raw.operator {
            match operator {
                ResourceCountOperator::Exponentiate => {
                    // If 'operator' is '^', 'min' must be >= 2
                    if raw.min < 2 {
                        return Err(FluxError::Logic("The 'min' value for the 'count' field must be greater than or equal to 2 when 'operator' is '^'".to_string()));
                    }
                    // If 'operator' is '^', 'operand' must be >= 2
                    if let Some(operand) = &raw.operand
                        && *operand < 2
                    {
                        return Err(FluxError::Logic("The 'operand' value for the 'count' field must be greater than or equal to 2 when 'operator' is '^'".to_string()));
                    }
                }
                ResourceCountOperator::Multipy => {
                    // If 'operator' is '*', 'operand' must be >= 2
                    if let Some(operand) = &raw.operand
                        && *operand < 2
                    {
                        return Err(FluxError::Logic("The 'operand' value for the 'count' field must be greater than or equal to 2 when 'operator' is '*'".to_string()));
                    }
                }
                ResourceCountOperator::Plus => {}
            }
        }
        Ok(Self {
            min: raw.min,
            max: raw.max,
            operator: raw.operator,
            operand: raw.operand,
        })
    }
}
