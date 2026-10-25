use std::io::Cursor;

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use lisp_rpc_rust_parser::*;

const SMALL_RPC: &str = r#"(get-book :title "hello \"world" :version "1984" :id 1984)"#;

const REALISTIC_RPC: &str = r#"(execute-agent-workflow
  :workflow-id "wf-98765"
  :execution-mode 'async
  :timeout-seconds 300
  :context '(:session-id "sess-123" :user-tier 'premium)
  :steps '('(:step-id 1 :action "code_search" :params '(:pattern "def-rpc" :path "src/"))
          '(:step-id 2 :action "summarize" :params '(:max-tokens 500 :language "en"))))"#;

fn bench_tokenize_small(c: &mut Criterion) {
    c.bench_function("bench_tokenize_small", |b| {
        b.iter(|| {
            let mut parser = Parser::new();
            parser
                .tokenize(Cursor::new(black_box(SMALL_RPC.as_bytes())))
                .unwrap();
            black_box(parser.tokens.len())
        })
    });
}

fn bench_parse_small(c: &mut Criterion) {
    c.bench_function("bench_parse_small", |b| {
        b.iter(|| {
            let mut parser = Parser::new();
            parser
                .tokenize(Cursor::new(black_box(SMALL_RPC.as_bytes())))
                .unwrap();
            parser.parse().unwrap();
            black_box(parser.pop_expr())
        })
    });
}

fn bench_tokenize_realistic(c: &mut Criterion) {
    c.bench_function("bench_tokenize_realistic", |b| {
        b.iter(|| {
            let mut parser = Parser::new();
            parser
                .tokenize(Cursor::new(black_box(REALISTIC_RPC.as_bytes())))
                .unwrap();
            black_box(parser.tokens.len())
        })
    });
}

fn bench_parse_realistic_new_parser(c: &mut Criterion) {
    c.bench_function("bench_parse_realistic_new_parser", |b| {
        b.iter(|| {
            let mut parser = Parser::new();
            parser
                .tokenize(Cursor::new(black_box(REALISTIC_RPC.as_bytes())))
                .unwrap();
            parser.parse().unwrap();
            black_box(parser.pop_expr())
        })
    });
}

fn bench_parse_realistic_reused_parser(c: &mut Criterion) {
    let mut parser = Parser::new();
    c.bench_function("bench_parse_realistic_reused_parser", |b| {
        b.iter(|| {
            parser.clear().unwrap();
            parser
                .tokenize(Cursor::new(black_box(REALISTIC_RPC.as_bytes())))
                .unwrap();
            parser.parse().unwrap();
            black_box(parser.pop_expr())
        })
    });
}

fn bench_parse_str_small(c: &mut Criterion) {
    let mut parser = Parser::new();
    c.bench_function("bench_parse_str_small", |b| {
        b.iter(|| {
            parser.parse_str(black_box(SMALL_RPC)).unwrap();
            black_box(parser.pop_expr())
        })
    });
}

fn bench_parse_str_realistic(c: &mut Criterion) {
    let mut parser = Parser::new();
    c.bench_function("bench_parse_str_realistic", |b| {
        b.iter(|| {
            parser.parse_str(black_box(REALISTIC_RPC)).unwrap();
            black_box(parser.pop_expr())
        })
    });
}

fn bench_parse_borrowed_small(c: &mut Criterion) {
    let parser = Parser::new();
    c.bench_function("bench_parse_borrowed_small", |b| {
        b.iter(|| black_box(parser.parse_str_one(black_box(SMALL_RPC)).unwrap()))
    });
}

fn bench_parse_borrowed_realistic(c: &mut Criterion) {
    let parser = Parser::new();
    c.bench_function("bench_parse_borrowed_realistic", |b| {
        b.iter(|| black_box(parser.parse_str_one(black_box(REALISTIC_RPC)).unwrap()))
    });
}

criterion_group!(
    bench_core,
    bench_tokenize_small,
    bench_parse_small,
    bench_tokenize_realistic,
    bench_parse_realistic_new_parser,
    bench_parse_realistic_reused_parser,
    bench_parse_str_small,
    bench_parse_str_realistic,
    bench_parse_borrowed_small,
    bench_parse_borrowed_realistic,
);

criterion_main!(bench_core);
