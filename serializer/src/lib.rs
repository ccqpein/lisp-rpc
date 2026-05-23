use anyhow::Result;
use serde::{Deserialize, Serialize};

pub mod deserializer;
pub mod serializer;

pub use deserializer::*;
pub use serializer::*;

/// entry function that serialize the lisp rpc struct
pub fn lisp_rpc_to_str(v: impl Serialize) -> Result<String> {
    let mut buf = Vec::with_capacity(1024);

    let mut _s = serializer::LispRPCSerializer::new(&mut buf);
    v.serialize(&mut _s)?;
    let pos = _s.pos;

    Ok(String::from_utf8(buf[..pos].to_vec())?)
}

/// entry function that serialize the lisp rpc struct with buffer
pub fn lisp_rpc_to_buf(v: impl Serialize, buffer: &mut Vec<u8>) -> Result<usize> {
    let mut _s = serializer::LispRPCSerializer::new(buffer);
    v.serialize(&mut _s)?;
    Ok(_s.pos)
}

/// entry function that deserialize the lisp rpc struct
pub fn lisp_rpc_from_str<'s, T: Deserialize<'s>>(s: &'s str) -> Result<T> {
    let mut _de = deserializer::LispRPCDeserializer::from_str(s);
    Ok(T::deserialize(&mut _de)?)
}
