//! Asynchronous stream adapter converting a stream of raw bytes into [`Data`].

use std::pin::Pin;
use std::task::{Context, Poll};

use super::*;
use futures_core::Stream;
use lisp_rpc_rust_parser::StreamParser;

/// An asynchronous stream adapter wrapping a [`StreamParser`] that transforms parsed [`Expr`]s into [`Data`].
pub struct RawDataGenerator<S> {
    /// Inner stream parser instance.
    pub p: StreamParser<S>,
}

impl<S, B, E> RawDataGenerator<S>
where
    S: Stream<Item = Result<B, E>> + Unpin,
    B: AsRef<[u8]>,
    E: std::fmt::Display,
{
    /// Creates a new `RawDataGenerator` wrapping the given byte stream.
    pub fn new(s: S) -> Self {
        Self {
            p: StreamParser::new(s),
        }
    }

    /// Creates a new `RawDataGenerator` wrapping an existing [`StreamParser`].
    pub fn with_parser(p: StreamParser<S>) -> Self {
        Self { p }
    }
}

impl<S, B, E> Stream for RawDataGenerator<S>
where
    S: Stream<Item = Result<B, E>> + Unpin,
    B: AsRef<[u8]>,
    E: std::fmt::Display,
{
    type Item = Result<Data, anyhow::Error>;

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.p.size_hint()
    }

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = &mut *self;

        match Pin::new(&mut this.p).poll_next(cx) {
            Poll::Ready(Some(expr_res)) => match expr_res {
                Ok(expr) => Poll::Ready(Some(Data::from_expr(&expr))),
                Err(e) => Poll::Ready(Some(Err(anyhow::anyhow!(e)))),
            },
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}
