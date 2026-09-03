//! Asynchronous stream parser for Lisp-RPC S-expressions.

use std::io::Cursor;
use std::pin::Pin;
use std::task::{Context, Poll};

use futures_core::Stream;

use super::*;

/// An asynchronous streaming parser that wraps a byte stream and produces parsed [`Expr`]s.
pub struct StreamParser<S> {
    /// Inner parser instance holding token queue and parsing status.
    pub p: Parser,
    /// Inner byte stream.
    pub s: S,
}

impl<S, B, E> StreamParser<S>
where
    S: Stream<Item = Result<B, E>> + Unpin,
    B: AsRef<[u8]>,
    E: std::fmt::Display,
{
    /// Creates a new `StreamParser` wrapping the given stream with a default parser.
    pub fn new(s: S) -> Self {
        Self {
            p: Parser::new(),
            s,
        }
    }

    /// Creates a new `StreamParser` with a pre-configured parser.
    pub fn with_parser(s: S, p: Parser) -> Self {
        Self { p, s }
    }
}

impl<S, B, E> Stream for StreamParser<S>
where
    S: Stream<Item = Result<B, E>> + Unpin,
    B: AsRef<[u8]>,
    E: std::fmt::Display,
{
    type Item = Result<Expr, ParserError>;

    fn size_hint(&self) -> (usize, Option<usize>) {
        let lower = self.p.exprs.len();
        let (_, upper) = self.s.size_hint();
        (lower, upper)
    }

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = &mut *self;

        if this.p.status.is_error() {
            return Poll::Ready(None);
        }

        loop {
            // 1. If we already have a completed expr queued in `p.exprs`, yield it immediately
            if let Some(e) = this.p.pop_expr() {
                return Poll::Ready(Some(Ok(e)));
            }

            // 2. Try to parse any tokens currently in the parser
            if let Err(e) = this.p.parse() {
                return Poll::Ready(Some(Err(e)));
            }

            // If parsing produced complete expression(s), yield the first one
            if let Some(e) = this.p.pop_expr() {
                return Poll::Ready(Some(Ok(e)));
            }

            // 3. Need more data: poll the inner stream for the next byte chunk
            match Pin::new(&mut this.s).poll_next(cx) {
                Poll::Ready(Some(Ok(chunk))) => {
                    // Tokenize the incoming chunk (chunk.as_ref() implements std::io::Read)
                    if let Err(_) = this.p.tokenize(Cursor::new(chunk.as_ref())) {
                        this.p.status = ParsingStatus::Error;
                        return Poll::Ready(Some(Err(ParserError::CorruptData(
                            "failed to tokenize chunk from stream",
                        ))));
                    }
                    // Loop back to try parsing with the newly appended tokens
                }
                Poll::Ready(Some(Err(_err))) => {
                    this.p.status = ParsingStatus::Error;
                    return Poll::Ready(Some(Err(ParserError::InvalidToken("stream read error"))));
                }
                Poll::Ready(None) => {
                    // Inner stream reached EOF
                    // If parser is left in an incomplete state, that's an unexpected EOF
                    if this.p.status.is_incomplete() {
                        this.p.status = ParsingStatus::Error;
                        this.p.tokens.clear();
                        return Poll::Ready(Some(Err(ParserError::InvalidToken(
                            "unexpected EOF in expression",
                        ))));
                    }
                    return Poll::Ready(None);
                }
                Poll::Pending => {
                    return Poll::Pending;
                }
            }
        }
    }
}
