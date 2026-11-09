//! Byte-counting stream wrapper for control-plane sockets (Phase 64, `TMON-01`, hook H19).
//!
//! `Metered<S>` adds the number of bytes actually read/written to a [`TrafficTap`]; it never
//! inspects or stores payload bytes. With the monitor disabled the cost is one relaxed load.

use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

use crate::traffic::TrafficTap;

/// An `AsyncRead + AsyncWrite` stream that counts bytes into a traffic cell.
#[derive(Debug)]
pub struct Metered<S> {
    inner: S,
    tap: TrafficTap,
}

impl<S> Metered<S> {
    pub fn new(inner: S, tap: TrafficTap) -> Self {
        Self { inner, tap }
    }

    /// Redirects subsequent counts to another channel (e.g. HTTP -> SSE).
    pub fn set_tap(&mut self, tap: TrafficTap) {
        self.tap = tap;
    }

    /// Unwraps the stream (later bytes are counted by the new owner).
    pub fn into_inner(self) -> S {
        self.inner
    }
}

impl<S: AsyncRead + Unpin> AsyncRead for Metered<S> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let before = buf.filled().len();
        let r = Pin::new(&mut self.inner).poll_read(cx, buf);
        if let Poll::Ready(Ok(())) = &r {
            let n = buf.filled().len().saturating_sub(before);
            if n > 0 {
                self.tap.rx(n);
            }
        }
        r
    }
}

impl<S: AsyncWrite + Unpin> AsyncWrite for Metered<S> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let r = Pin::new(&mut self.inner).poll_write(cx, buf);
        if let Poll::Ready(Ok(n)) = &r
            && *n > 0
        {
            self.tap.tx(*n);
        }
        r
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}
