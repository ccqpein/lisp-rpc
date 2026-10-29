//! Serde-based serialization and deserialization for Lisp-RPC S-expressions.
//!
//! # Key Functions
//!
//! - [`lisp_rpc_to_str`]: Serializes any [`serde::Serialize`] type into a Lisp-RPC S-expression `String`.
//! - [`lisp_rpc_to_buf`]: Serializes any [`serde::Serialize`] type directly into a `Vec<u8>` buffer.
//! - [`lisp_rpc_from_str`]: Deserializes any [`serde::Deserialize`] type from a Lisp-RPC S-expression `&str`.
//!
//! # Quick Start
//!
//! ```rust
//! use lisp_rpc_rust_serializer::{lisp_rpc_from_str, lisp_rpc_to_str};
//! use serde::{Deserialize, Serialize};
//!
//! #[derive(Debug, PartialEq, Serialize, Deserialize)]
//! struct BookInfo {
//!     title: String,
//!     year: i32,
//! }
//!
//! let book = BookInfo {
//!     title: "The Hobbit".to_string(),
//!     year: 1937,
//! };
//!
//! let encoded = lisp_rpc_to_str(&book).unwrap();
//! assert_eq!(encoded, r#"(book-info :title "The Hobbit" :year 1937)"#);
//!
//! let decoded: BookInfo = lisp_rpc_from_str(&encoded).unwrap();
//! assert_eq!(decoded, book);
//! ```

use anyhow::Result;
use serde::{Deserialize, Serialize};

pub mod deserializer;
pub mod serializer;

pub use deserializer::*;
pub use serializer::*;

/// Serializes a value into a Lisp-RPC S-expression string.
pub fn lisp_rpc_to_str(v: impl Serialize) -> Result<String> {
    let mut buf = Vec::with_capacity(256);
    let pos = lisp_rpc_to_buf(v, &mut buf)?;
    buf.truncate(pos);

    Ok(String::from_utf8(buf)?)
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
