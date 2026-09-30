# README

The generator for generating Rust code from Lisp-RPC protocol files.

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

Check the [example](../../examples/rust/spec-mode-generate-lib/) for more details.
