# Lisp-RPC Rust Server

[![crates.io](https://img.shields.io/crates/v/lisp-rpc-rust-server.svg)](https://crates.io/crates/lisp-rpc-rust-server)
[![docs.rs](https://docs.rs/lisp-rpc-rust-server/badge.svg)](https://docs.rs/lisp-rpc-rust-server)

Server implementation for Lisp-RPC, providing synchronous and asynchronous request dispatching and RPC type traits.

## Overview

`lisp-rpc-rust-server` provides the `RPCServer` dispatch engine and traits for routing incoming Lisp-RPC S-expression requests to typed synchronous or asynchronous Rust handler functions. It is transport-agnostic and integrates easily with HTTP frameworks (such as Actix Web) or raw TCP/socket servers.

## Key Types & APIs

- `RPCServer`: Thread-safe (`Arc`-backed) RPC dispatch engine.
  - `register::<T, _>(func)`: Registers a synchronous handler for RPC command type `T`.
  - `register_async::<T, _>(func)`: Registers an asynchronous handler for RPC command type `T`.
  - `handle(raw_data)`: Dispatches a raw S-expression string synchronously and returns the serialized S-expression response.
  - `handle_async(raw_data)`: Dispatches a raw S-expression string asynchronously and returns the serialized S-expression response.
- `RPCType`: Classifies types into `RPC(String)`, `Msg(String)`, `Map`, `List`, or `V` (primitive value).
- `ToRPCType` / `ToRPCReturn`: Traits associating Rust structs with their Lisp-RPC command/message type and return type.
- `impl_to_rpc!` / `impl_to_rpc_return!`: Helper macros to implement `ToRPCType` and `ToRPCReturn` for custom structs.

## Usage Example

```rust
use lisp_rpc_rust_server::{
    impl_to_rpc, impl_to_rpc_return, lisp_rpc_to_str, RPCServer, RPCType, ToRPCReturn, ToRPCType,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct GetBook {
    title: String,
}

// Register `GetBook` as the `get-book` RPC command returning a `String`
impl_to_rpc!(GetBook, RPCType::RPC("get-book".to_string()));
impl_to_rpc_return!(GetBook, String);

let server = RPCServer::new()
    .register::<GetBook, _>(|req: GetBook| {
        Ok(format!("Found book: {}", req.title))
    })
    .unwrap();

let response = server
    .handle(r#"(get-book :title "The Hobbit")"#)
    .unwrap();

assert_eq!(response, r#""Found book: The Hobbit""#);
```
