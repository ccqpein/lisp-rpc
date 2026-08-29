//! Serde-based serialization and deserialization for Lisp-RPC S-expressions.

use anyhow::Result;
use serde::{Deserialize, Serialize};

pub mod deserializer;
pub mod serializer;

pub use deserializer::*;
pub use serializer::*;

/// Serializes a value into a Lisp-RPC S-expression string.
pub fn lisp_rpc_to_str(v: impl Serialize) -> Result<String> {
    let mut buf = Vec::with_capacity(1024);

    let mut _s = serializer::LispRPCSerializer::new(&mut buf);
    v.serialize(&mut _s)?;
    let pos = _s.pos;

    Ok(String::from_utf8(buf[..pos].to_vec())?)
}

/// Serializes a value into a byte buffer in Lisp-RPC format, returning the number of bytes written.
pub fn lisp_rpc_to_buf(v: impl Serialize, buffer: &mut Vec<u8>) -> Result<usize> {
    let mut _s = serializer::LispRPCSerializer::new(buffer);
    v.serialize(&mut _s)?;
    Ok(_s.pos)
}

/// Deserializes a value from a Lisp-RPC S-expression string.
pub fn lisp_rpc_from_str<'s, T: Deserialize<'s>>(s: &'s str) -> Result<T> {
    let mut _de = deserializer::LispRPCDeserializer::from_str(s);
    Ok(T::deserialize(&mut _de)?)
}
