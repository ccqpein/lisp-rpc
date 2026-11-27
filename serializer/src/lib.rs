//! Serde-based serialization and deserialization for Lisp-RPC S-expressions.
//!
//! # Key Functions
//!
//! - [`lisp_rpc_to_str`]: Serializes any [`serde::Serialize`] type into a Lisp-RPC S-expression `String`.
//! - [`lisp_rpc_to_buf`]: Serializes any [`serde::Serialize`] type directly into a `Vec<u8>` buffer.
//! - [`lisp_rpc_from_str`]: Deserializes any [`serde::Deserialize`] type from a Lisp-RPC S-expression `&str`.
//! - [`LispRPCSerializer`] / [`LispRPCDeserializer`]: Underlying Serde serializer and deserializer engines.
//! - [`register_global_map_type`] / [`clear_global_map_types`]: Configure struct names to serialize as quoted anonymous maps `'(:key val ...)` instead of `(struct-name :key val ...)`.
//!
//! # Serde Mapping Rules
//!
//! | Rust Type | Serialized Lisp-RPC | Accepted on Deserialize |
//! | --- | --- | --- |
//! | `struct FooBar { max_conn: i32 }` | `(foo-bar :max-conn 10)` | `(foo-bar :max-conn 10)` or `'(:max-conn 10)` |
//! | `bool` (`true` / `false`) | `t` / `nil` | `t` / `nil` (unquoted) |
//! | `Option<T>` (`Some(x)` / `None`) | `x` / `nil` | `x`, `nil`, or omitted key |
//! | `#[serde(default)]` field | serialized normally | omitted key uses `Default` (avoid passing `nil` for non-`Option` types) |
//! | `Vec<T>` (scalars) | `'("a" "b")` | `'("a" "b")` |
//! | `Vec<Struct>` | `'((item :id 1) (item :id 2))` | `'((item :id 1) (item :id 2))` |
//! | Unit `enum` variant (`Status::InProgress`) | *(unsupported on serialize)* | Quoted kebab-case symbol `'in-progress` |
//!
//! **Important — Do NOT add `#[serde(rename_all = "kebab-case")]`**:
//! `LispRPCSerializer` converts `PascalCase` struct names to `kebab-case` and `snake_case`
//! fields to `:kebab-case` automatically, and `LispRPCDeserializer` normalizes `:kebab-case`
//! keys back to `snake_case` before handing field identifiers to Serde. Adding
//! `#[serde(rename_all = "kebab-case")]` causes Serde to expect kebab-case identifiers after
//! the deserializer has already converted them to `snake_case`.
//!
//! # Quick Start (Structs, Nested Structs, Options, Defaults & Anonymous Maps)
//!
//! ```rust
//! use lisp_rpc_rust_serializer::{lisp_rpc_from_str, lisp_rpc_to_str};
//! use serde::{Deserialize, Serialize};
//!
//! #[derive(Debug, PartialEq, Serialize, Deserialize)]
//! struct DatabaseConfig {
//!     host: String,
//!     port: u16,
//! }
//!
//! #[derive(Debug, PartialEq, Serialize, Deserialize)]
//! struct ServerConfig {
//!     listen_addr: String,
//!     enable_tls: bool,
//!     #[serde(default = "default_workers")]
//!     workers: usize,
//!     cert_path: Option<String>,
//!     allowed_origins: Vec<String>,
//!     database: DatabaseConfig,
//! }
//!
//! fn default_workers() -> usize { 4 }
//!
//! let cfg = ServerConfig {
//!     listen_addr: "0.0.0.0:8080".to_string(),
//!     enable_tls: false,
//!     workers: 4,
//!     cert_path: None,
//!     allowed_origins: vec!["https://example.com".to_string()],
//!     database: DatabaseConfig {
//!         host: "db.internal".to_string(),
//!         port: 5432,
//!     },
//! };
//!
//! let encoded = lisp_rpc_to_str(&cfg).unwrap();
//! assert_eq!(
//!     encoded,
//!     r#"(server-config :listen-addr "0.0.0.0:8080" :enable-tls nil :workers 4 :cert-path nil :allowed-origins '("https://example.com") :database (database-config :host "db.internal" :port 5432))"#
//! );
//!
//! // Named structures deserialize directly
//! let decoded: ServerConfig = lisp_rpc_from_str(&encoded).unwrap();
//! assert_eq!(decoded, cfg);
//!
//! // Quoted anonymous maps and omitted #[serde(default)] / Option<T> keys also deserialize
//! let input = r#"'(:listen-addr "0.0.0.0:8080"
//!                  :enable-tls nil
//!                  :allowed-origins '("https://example.com")
//!                  :database '(:host "db.internal" :port 5432))"#;
//! let from_map: ServerConfig = lisp_rpc_from_str(input).unwrap();
//! assert_eq!(from_map, cfg);
//! ```
//!
//! # Serializing a Struct as an Anonymous Map (`'(:k v)`)
//!
//! ```rust
//! use lisp_rpc_rust_serializer::LispRPCSerializer;
//! use serde::Serialize;
//!
//! #[derive(Serialize)]
//! struct Meta {
//!     env: String,
//!     debug: bool,
//! }
//!
//! let mut buf = Vec::new();
//! let mut ser = LispRPCSerializer::new(&mut buf);
//! ser.register_map_type("Meta");
//! Meta { env: "prod".into(), debug: false }.serialize(&mut ser).unwrap();
//!
//! assert_eq!(String::from_utf8(buf).unwrap(), r#"'(:env "prod" :debug nil)"#);
//! ```
//!
//! # Note on Comments in Config Files
//!
//! `lisp_rpc_from_str` parses raw S-expression tokens directly for maximum throughput and does
//! not strip `;` comments. When loading commented `.lisp` config files with Serde, strip `;`
//! line comments first (see `examples/config.rs`) or parse dynamically with
//! `lisp-rpc-rust-raw-data`.

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
