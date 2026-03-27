use anyhow::Result;
use serde::{Deserialize, Serialize};

pub mod deserializer;
pub mod serializer;

pub use deserializer::*;
pub use serializer::*;

/// entry function that serialize the lisp rpc struct
pub fn lisp_rpc_to_str(v: impl Serialize) -> Result<String> {
    let mut _s = serializer::LispRPCSerializer::new();
    v.serialize(&mut _s)?;
    Ok(_s.output)
}

/// entry function that deserialize the lisp rpc struct
pub fn lisp_rpc_from_str<'s, T: Deserialize<'s>>(s: &'s str) -> Result<T> {
    let mut _de = deserializer::LispRPCDeserializer::from_str(s);
    Ok(T::deserialize(&mut _de)?)
}
