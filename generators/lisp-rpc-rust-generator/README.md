# Lisp-RPC Rust Generator

[![crates.io](https://img.shields.io/crates/v/lisp-rpc-rust-generator.svg)](https://crates.io/crates/lisp-rpc-rust-generator)
[![docs.rs](https://docs.rs/lisp-rpc-rust-generator/badge.svg)](https://docs.rs/lisp-rpc-rust-generator)

Command-line application for generating Rust code from Lisp-RPC protocol files.

## Project Structure

### templates

Templates used for code generation.

### src

Core logic that reads templates and specifications to generate code.

## Usage

After building:

```shell
lisp-rpc-rust-generator \
        --input-file spec.lisprpc \
        --templates-path ./templates/ \
        --output-path .
```

In most cases, you don't need to provide the templates folder, as the generator can use its embedded templates:

```shell
lisp-rpc-rust-generator \
        --input-file spec.lisprpc \
        --output-path .
```

To also generate RPC trait implementations for the RPC server, pass `--with-server` (or `-w`):

```shell
lisp-rpc-rust-generator \
        --input-file spec.lisprpc \
        --output-path . \
        --with-server
```

Check the [example](../../examples/rust/spec-mode-generate-lib/) for more details.
