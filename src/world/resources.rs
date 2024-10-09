use serde_repr::{Deserialize_repr, Serialize_repr};
use std::{collections::HashMap, fmt::Display, hash::Hash};

#[derive(Debug, Serialize_repr, Deserialize_repr, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Resource {
    SATOSHI,
    GOLD,
    SCRAPS,
    FUEL,
    RUM,
}

impl Display for Resource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Resource::SATOSHI => write!(f, "Satoshi"),
            Resource::GOLD => write!(f, "Gold"),
            Resource::SCRAPS => write!(f, "Scraps"),
            Resource::FUEL => write!(f, "Fuel"),
            Resource::RUM => write!(f, "Rum"),
        }
    }
}

impl Resource {
    pub fn base_price(&self) -> f32 {
        match self {
            Resource::SATOSHI => 1.0,
            Resource::GOLD => 1750.0,
            Resource::SCRAPS => 40.0,
            Resource::FUEL => 30.0,
            Resource::RUM => 125.0,
        }
    }

    pub fn to_storing_space(&self) -> u32 {
        match self {
            Resource::SATOSHI => 0,
            Resource::GOLD => 2,
            Resource::SCRAPS => 10,
            Resource::FUEL => 0, // Fuel is stored in the spaceship tank
            Resource::RUM => 1,
        }
    }

    pub fn used_storage_capacity(resources: &HashMap<Resource, u32>) -> u32 {
        resources
            .iter()
            .map(|(k, v)| k.to_storing_space() * v)
            .sum()
    }
}
