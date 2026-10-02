# Lisp-RPC

Lisp-RPC is a lightweight, S-expression-based Remote Procedure Call protocol designed for simplicity and flexibility.

## Overview

Lisp-RPC combines the human readability of text-based formats with the strong typing of RPC protocols using Lisp S-expression syntax.

### Use Cases

- **Dynamic Communication (Plain Mode)**: Inspectable, schema-free messaging using raw S-expressions.
- **Typed Microservices (Spec Mode)**: Schema-defined client/server communication with code generation.
- **Inter-Process & Network RPC**: Lightweight alternative to JSON-RPC and gRPC without verbose nesting or complex IDL setups.

## Specification

Read the [Lisp-RPC Specification](https://lisp-rpc.ccqpein.me).

## Related Repositories

- [lisp-rpc-doc](https://github.com/ccqpein/lisp-rpc-doc): Specification documentation and website sources.
- [lisp-rpc.cl](https://github.com/ccqpein/lisp-rpc.cl): Common Lisp implementation of Lisp-RPC.
- [lisp-rpc-json-convertor](https://github.com/ccqpein/lisp-rpc-json-convertor): Bidirectional converter between JSON and Lisp-RPC.

## Project Structure

This repository is organized as a Rust workspace containing the core parser, serializer, and generators, along with examples and utilities.

### [examples/](examples/)

Practical examples demonstrating both Plain Mode and Spec Mode usage.

### [generators/](generators/)

Implementation of code generators that transform `.lisprpc` specs into library code.

### [parsers/](parsers/)

Core Lisp-RPC parsers written in Rust.

### [raw-data/](raw-data/)

Core raw data structures (like the `Data` enum) used for dynamic, plain-mode communication.

### [serializer/](serializer/)

Core serialization and deserialization logic.

### [server/](server/)

Server engine with Actix Web and asynchronous dispatch support.

### [utils/](utils/)

Helper scripts and Common Lisp utilities for protocol validation.

## Installation

To install the Lisp-RPC generator tool:

```shell
cargo install --path generators/lisp-rpc-rust-generator
```

## Usage Modes

### Plain Mode

In Plain Mode, clients send raw S-expressions as strings. The server parses these expressions dynamically.

```lisp
(get-book :title "The Hobbit" :year 1937)
```

### Spec Mode

Spec Mode uses a formal schema defined with `def-msg` and `def-rpc`. This enables generating typed libraries for various target languages.

```lisp
(def-msg book-info :title 'string :id 'number)
(def-rpc get-book '(:title 'string) 'book-info)
```

## Examples

### Serialize Structure

Structures that derive `Serialize` and `Deserialize` can use `lisp_rpc_from_str` and `lisp_rpc_to_str` from `lisp_rpc_rust_serializer` to serialize and deserialize between the structure and the Lisp-RPC format string.

Check [this example](examples/rust/serialize-struct-to-lisprpc/) for more details.

### Handling Plain Mode

`lisp_rpc_rust_raw_data::Data` can parse a string directly into a `Data` object, which can then be accessed using `.get(keyname)`.

Check [this example](examples/rust/plain-mode-read-and-write-data/) for more details.

### Spec & Code Generation Mode

[spec-mode-generate-lib](examples/rust/spec-mode-generate-lib/) shows how to generate the Lisp-RPC library code using the generator.

```shell
# Generate data structures only (default)
lisp-rpc-rust-generator --input-file spec.lisprpc --output-path .

# Generate data structures and implement server RPC traits
lisp-rpc-rust-generator --input-file spec.lisprpc --output-path . --with-server
```

### Using the Server with Custom Structures

Integrating custom structures is straightforward. If the structures derive `Serialize` and `Deserialize`, you need to implement `RPCType` with the `impl_to_rpc!` macro from the [server lib](server/).

Check [this example](examples/rust/server-with-custom-structure/) for more details.
