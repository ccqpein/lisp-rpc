use std::collections::VecDeque;
use std::io::Cursor;
use std::pin::Pin;
use std::task::{Context, Poll, RawWaker, RawWakerVTable, Waker};

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use futures_core::Stream;
use lisp_rpc_rust_parser::*;
use lisp_rpc_rust_raw_data::{Data, FromStr as RpcFromStr};
use lisp_rpc_rust_serializer::{lisp_rpc_from_str, lisp_rpc_to_str};
use serde::{Deserialize, Serialize};

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

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct SearchProducts {
    query: String,
    category: Category,
    sort_by: SortBy,
    min_price: f64,
    max_price: f64,
    in_stock_only: bool,
    discount_code: Option<String>,
    tags: Vec<String>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct GetBookBench {
    title: String,
    version: String,
    min_price: f64,
    in_stock_only: bool,
    discount_code: Option<String>,
    lang: GetBookLangBench,
    authors: AuthorsBench,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct GetBookLangBench {
    lang_code: String,
    encoding_id: i64,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct AuthorsBench {
    names: Vec<String>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
enum Category {
    Electronics,
    Books,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
enum SortBy {
    PriceAsc,
    PriceDesc,
}

const SERIALIZER_RPC: &str = r#"(search-products :query "mechanical keyboard" :category 'electronics :sort-by 'price-asc :min-price 49.99 :max-price 199.99 :in-stock-only t :discount-code nil :tags '("ergonomic" "wireless" "hot-swap"))"#;

fn bench_raw_data_from_str(c: &mut Criterion) {
    let mut parser = Parser::new().config_read_number(true);
    c.bench_function("bench_raw_data_from_str", |b| {
        b.iter(|| {
            parser.clear().unwrap();
            black_box(Data::from_str(&mut parser, black_box(REALISTIC_RPC)).unwrap())
        })
    });
}

fn bench_raw_data_to_string(c: &mut Criterion) {
    let mut parser = Parser::new().config_read_number(true);
    let data = Data::from_str(&mut parser, REALISTIC_RPC).unwrap();
    c.bench_function("bench_raw_data_to_string", |b| {
        b.iter(|| black_box(black_box(&data).to_string()))
    });
}

fn bench_serializer_to_str(c: &mut Criterion) {
    let req = GetBookBench {
        title: "mechanical keyboard".to_string(),
        version: "1984".to_string(),
        min_price: 49.99,
        in_stock_only: true,
        discount_code: None,
        lang: GetBookLangBench {
            lang_code: "english".to_string(),
            encoding_id: 64,
        },
        authors: AuthorsBench {
            names: vec![
                "ergonomic".to_string(),
                "wireless".to_string(),
                "hot-swap".to_string(),
            ],
        },
    };
    c.bench_function("bench_serializer_to_str", |b| {
        b.iter(|| black_box(lisp_rpc_to_str(black_box(&req)).unwrap()))
    });
}

fn bench_deserializer_from_str(c: &mut Criterion) {
    c.bench_function("bench_deserializer_from_str", |b| {
        b.iter(|| black_box(lisp_rpc_from_str::<SearchProducts>(black_box(SERIALIZER_RPC)).unwrap()))
    });
}

criterion_group!(
    bench_advanced,
    bench_parse_schema_defs,
    bench_parse_batch_20,
    bench_stream_parser_chunked_32b,
    bench_expr_to_string,
    bench_raw_data_from_str,
    bench_raw_data_to_string,
    bench_serializer_to_str,
    bench_deserializer_from_str,
);

criterion_main!(bench_advanced);
