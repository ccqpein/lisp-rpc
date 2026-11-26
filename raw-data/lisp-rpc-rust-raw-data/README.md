# Lisp-RPC Rust Raw Data

[![crates.io](https://img.shields.io/crates/v/lisp-rpc-rust-raw-data.svg)](https://crates.io/crates/lisp-rpc-rust-raw-data)
[![docs.rs](https://docs.rs/lisp-rpc-rust-raw-data/badge.svg)](https://docs.rs/lisp-rpc-rust-raw-data)

Core raw data structures for dynamic, schema-free S-expression parsing and manipulation in Lisp-RPC Plain Mode.

## Overview

`lisp-rpc-rust-raw-data` provides dynamic representations for Lisp-RPC S-expressions without requiring predefined schemas or generated types.

## Key Types

- `Data<'a>`: Root enum representing any valid Lisp-RPC data element (expressions, lists, maps, atoms/values).
- `ExprData<'a>`: Named S-expression data structure: `(name :key value ...)`.
- `ListData<'a>`: Quoted sequence: `'(1 2 "three")`.
- `MapData<'a>`: Quoted key-value pairs: `'(:key value)`.
- `DataFile`: Parses and indexes multiple `ExprData` records from a file or string (skipping `;` comments).
- `RawDataGenerator`: Asynchronous stream adapter converting byte streams into `Data<'static>`.

## Keyword Lookup & Typed Getters

- **Keyword lookup rule**: Pass key names **without** a leading colon (`data.get("port")`, **not** `data.get(":port")`).
- **Tag inspection**: `data.name()` returns `Some("server-config")` for `(server-config :port 3000)`.
- **Value extractors on `Data`**: `as_str()`, `as_symbol()`, `as_i64()`, `as_u64()`, `as_f64()`, `as_bool()`, `as_slice()`, `as_map()`, `as_expr()`.
- **Shorthand field getters on `Data`, `ExprData`, `MapData`**: `get_str(k)`, `get_symbol(k)`, `get_i64(k)`, `get_u64(k)`, `get_f64(k)`, `get_bool(k)`, `get_slice(k)`, `get_map(k)`, `get_expr(k)`.

## Usage Example

```rust
use lisp_rpc_rust_raw_data::Data;

let raw = r#"
;; Server configuration
(server-config
  :host "127.0.0.1"
  :port 3000
  :tls 'false
  :bind '("127.0.0.1" "0.0.0.0")
  :database (db-config :url "postgres://localhost/app" :pool-size 16))
"#;

let cfg: Data<'static> = raw.parse().unwrap();
assert_eq!(cfg.name(), Some("server-config"));
assert_eq!(cfg.get_str("host"), Some("127.0.0.1"));
assert_eq!(cfg.get_u64("port"), Some(3000));
assert_eq!(cfg.get_bool("tls"), Some(false));
assert_eq!(cfg.get_slice("bind").unwrap()[0].as_str(), Some("127.0.0.1"));
assert_eq!(cfg.get_expr("database").unwrap().get_u64("pool-size"), Some(16));
```

See [examples/config.rs](examples/config.rs) for a complete multi-section `.lisp` configuration example.
