use std::collections::VecDeque;
use std::io::Cursor;
use std::pin::Pin;
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use futures_core::Stream;
use lisp_rpc_rust_parser::*;

const SCHEMA_DEFS: &str = r#";; Book service schema definitions
(def-msg language-perfer :lang 'string)

;; Book information structure
(def-msg book-info
  :id 'string
  :title 'string
  :version 'string
  :price 'number
  :lang 'language-perfer)

;; RPC endpoints
(def-rpc get-book
  '(:title 'string :version 'string :lang 'language-perfer)
  'book-info)

(def-rpc search-products
  '(:query 'string :category 'symbol :min-price 'number :max-price 'number :in-stock-only 'boolean)
  '(:total-count 'number :page 'number :items 'list))"#;

const REALISTIC_RPC: &str = r#"(execute-agent-workflow
  :workflow-id "wf-98765"
  :execution-mode 'async
  :timeout-seconds 300
  :context '(:session-id "sess-123" :user-tier 'premium)
  :steps '('(:step-id 1 :action "code_search" :params '(:pattern "def-rpc" :path "src/"))
          '(:step-id 2 :action "summarize" :params '(:max-tokens 500 :language "en"))))"#;

fn dummy_waker() -> Waker {
    unsafe fn clone(_: *const ()) -> RawWaker {
        RawWaker::new(std::ptr::null(), &VTABLE)
    }
    unsafe fn wake(_: *const ()) {}
    unsafe fn wake_by_ref(_: *const ()) {}
    unsafe fn drop(_: *const ()) {}

    static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, wake, wake_by_ref, drop);
    unsafe { Waker::from_raw(RawWaker::new(std::ptr::null(), &VTABLE)) }
}

struct SliceChunkStream<'a> {
    chunks: VecDeque<&'a [u8]>,
}

impl<'a> SliceChunkStream<'a> {
    fn new(data: &'a [u8], chunk_size: usize) -> Self {
        Self {
            chunks: data.chunks(chunk_size).collect(),
        }
    }
}

impl<'a> Stream for SliceChunkStream<'a> {
    type Item = Result<&'a [u8], std::io::Error>;

    fn poll_next(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Poll::Ready(self.chunks.pop_front().map(Ok))
    }
}

fn bench_parse_schema_defs(c: &mut Criterion) {
    c.bench_function("bench_parse_schema_defs", |b| {
        b.iter(|| {
            let mut parser = Parser::new();
            parser
                .tokenize(Cursor::new(black_box(SCHEMA_DEFS.as_bytes())))
                .unwrap();
            parser.parse().unwrap();
            black_box(parser.exprs.len())
        })
    });
}

fn bench_parse_batch_20(c: &mut Criterion) {
    let batch = std::iter::repeat(REALISTIC_RPC)
        .take(20)
        .collect::<Vec<_>>()
        .join("\n");

    c.bench_function("bench_parse_batch_20", |b| {
        b.iter(|| {
            let mut parser = Parser::new();
            parser
                .tokenize(Cursor::new(black_box(batch.as_bytes())))
                .unwrap();
            parser.parse().unwrap();
            black_box(parser.exprs.len())
        })
    });
}

fn bench_stream_parser_chunked_32b(c: &mut Criterion) {
    let waker = dummy_waker();
    let bytes = REALISTIC_RPC.as_bytes();

    c.bench_function("bench_stream_parser_chunked_32b", |b| {
        b.iter(|| {
            let mut cx = Context::from_waker(&waker);
            let stream = SliceChunkStream::new(black_box(bytes), 32);
            let mut stream_parser = StreamParser::new(stream);
            let mut count = 0usize;
            while let Poll::Ready(Some(res)) = Pin::new(&mut stream_parser).poll_next(&mut cx) {
                black_box(res.unwrap());
                count += 1;
            }
            black_box(count)
        })
    });
}

fn bench_expr_to_string(c: &mut Criterion) {
    let mut parser = Parser::new();
    parser
        .tokenize(Cursor::new(REALISTIC_RPC.as_bytes()))
        .unwrap();
    parser.parse().unwrap();
    let expr = parser.pop_expr().unwrap();

    c.bench_function("bench_expr_to_string", |b| {
        b.iter(|| black_box(black_box(&expr).to_string()))
    });
}

criterion_group!(
    bench_advanced,
    bench_parse_schema_defs,
    bench_parse_batch_20,
    bench_stream_parser_chunked_32b,
    bench_expr_to_string,
);

criterion_main!(bench_advanced);
