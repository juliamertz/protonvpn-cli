pub mod openvpn;

use crate::{
    api::types::LogicalServer,
    cache::{self, CachedObject},
};
use anyhow::Result;
use std::net::Ipv4Addr;

#[derive(Clone, Debug)]
pub struct Pid(u32);

impl Pid {
    pub fn as_u32(&self) -> u32 {
        self.0
    }
}

impl From<u32> for Pid {
    fn from(value: u32) -> Self {
        Self(value)
    }
}

impl TryFrom<String> for Pid {
    type Error = anyhow::Error;
    fn try_from(value: String) -> std::result::Result<Self, Self::Error> {
        Ok(Self(value.trim().parse::<u32>()?))
    }
}

impl std::fmt::Display for Pid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
