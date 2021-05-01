use std::{io, pin::Pin, task::Poll};

use futures::AsyncRead;
use pin_project::pin_project;

pub trait ObserveRead: AsyncRead + Sized {
    fn observe<F: FnMut(&[u8])>(self, f: F) -> ObservedReader<Self, F>;
}

impl<R: AsyncRead + Sized> ObserveRead for R {
    fn observe<F: FnMut(&[u8])>(self, f: F) -> ObservedReader<Self, F> {
        ObservedReader::new(self, f)
    }
}

#[pin_project]
pub struct ObservedReader<R, F> {
    #[pin]
    inner: R,
    f: F,
}

impl<R, F> ObservedReader<R, F>
where
    R: AsyncRead,
    F: FnMut(&[u8]),
{
    pub fn new(inner: R, f: F) -> Self {
        Self { inner, f }
    }
}

impl<R, F> AsyncRead for ObservedReader<R, F>
where
    R: AsyncRead,
    F: FnMut(&[u8]),
{
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        let this = self.as_mut().project();
        let num_read = futures::ready!(this.inner.poll_read(cx, buf))?;
        (this.f)(&buf[0..num_read]);
        Poll::Ready(Ok(num_read))
    }
}
