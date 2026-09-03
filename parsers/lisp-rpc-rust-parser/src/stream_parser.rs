use anyhow::Result;
use std::str::Bytes;

use futures_core::Stream;

use super::*;

pub struct StreamParser<S: Stream> {
    pub p: Parser,
    pub s: S,
}

impl<S> Stream for StreamParser<S>
where
    S: Stream<Item = Result<Bytes>> + Unpin,
{
    type Item = Result<Expr, ParserError>;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let this = &mut *self;

        loop {
            // Try to get the expr first
            match this.p.parse() {
                Ok(_) => match this.p.exprs.pop_front() {
                    Some(e) => return Poll::Ready(Some(Ok(e))),
                    None => Poll::Ready(None),
                },

                Err(e) => return Poll::Ready(Some(Err(e))),
            }

            //:= TODO: Read from inner stream and add it to token

            //:= TODO: try to parse it again
        }
    }
}
