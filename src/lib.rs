use std::{
    alloc::{alloc, Layout},
    io,
    pin::Pin,
    ptr,
    ptr::slice_from_raw_parts_mut,
    task::Poll,
};

use futures::AsyncRead;
use pin_project::pin_project;

/// Convenience trait to apply the utility functions to types implementing
/// [`futures::AsyncRead`].
pub trait AsyncReadUtil: AsyncRead + Sized {
    /// Observe the bytes being read from `self` using the provided closure.
    /// Refer to [`crate::ObservedReader`] for more info.
    fn observe<F: FnMut(&[u8])>(self, f: F) -> ObservedReader<Self, F>;
    /// Map the bytes being read from `self` into a new buffer using the
    /// provided closure. Refer to [`crate::MappedReader`] for more
    /// info.
    fn map_read<F>(self, f: F) -> MappedReader<Self, F>
    where
        F: FnMut(&[u8], &mut [u8]) -> (usize, usize);
}

impl<R: AsyncRead + Sized> AsyncReadUtil for R {
    fn observe<F: FnMut(&[u8])>(self, f: F) -> ObservedReader<Self, F> {
        ObservedReader::new(self, f)
    }

    fn map_read<F>(self, f: F) -> MappedReader<Self, F>
    where
        F: FnMut(&[u8], &mut [u8]) -> (usize, usize),
    {
        MappedReader::new(self, f)
    }
}

/// An async reader which allows a closure to observe the bytes being read as
/// they are ready. This has use cases such as hashing the output of a reader
/// without interfering with the actual content.
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

/// An async reader which allows a closure to map the output of the inner async
/// reader into a new buffer. This allows things like compression/encryption to
/// be layered on top of a normal reader.
///
/// NOTE: The closure must consume at least 1 byte for the reader to continue.
///
/// SAFETY: This currently creates the equivalent of `Box<[MaybeUninit<u8>]>`,
/// but does so through use of accessing the allocator directly. Once new_uninit
/// is available on stable, `Box::new_uninit_slice` will be used. This will
/// still utilize unsafe. A uninitialized buffer is acceptable because it's
/// contents are only ever written to before reading only the written section.
///
/// ref: https://github.com/rust-lang/rust/issues/63291
#[pin_project]
pub struct MappedReader<R, F> {
    #[pin]
    inner: R,
    f: F,
    buf: Box<[u8]>,
    pos: usize,
    cap: usize,
    done: bool,
}

impl<R, F> MappedReader<R, F>
where
    R: AsyncRead,
    F: FnMut(&[u8], &mut [u8]) -> (usize, usize),
{
    pub fn new(inner: R, f: F) -> Self {
        Self::with_capacity(8096, inner, f)
    }

    pub fn with_capacity(capacity: usize, inner: R, f: F) -> Self {
        let buf = unsafe { uninit_buf(capacity) };
        Self {
            inner,
            f,
            buf,
            pos: 0,
            cap: 0,
            done: false,
        }
    }
}

impl<R, F> AsyncRead for MappedReader<R, F>
where
    R: AsyncRead,
    F: FnMut(&[u8], &mut [u8]) -> (usize, usize),
{
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut [u8],
    ) -> Poll<io::Result<usize>> {
        let this = self.as_mut().project();
        if *this.pos == *this.cap {
            *this.pos = 0;
            *this.cap = 0;
        }
        if !*this.done && *this.cap < this.buf.len() {
            let nread = futures::ready!(this.inner.poll_read(cx, &mut this.buf[*this.cap..]))?;
            *this.cap += nread;
            if nread == 0 {
                *this.done = true;
            }
        }
        let unprocessed = &this.buf[*this.pos..*this.cap];
        let (nsrc, ndst) = (this.f)(&this.buf[*this.pos..*this.cap], buf);
        assert!(
            ndst <= buf.len(),
            "mapped reader is reportedly reading more than the destination buffer's capacity"
        );
        // Nothing has been consumed and there are unprocessed bytes.
        if nsrc == 0 && !unprocessed.is_empty() {
            assert!(unprocessed.len() < this.buf.len());
            let count = unprocessed.len();
            // SAFETY: This utilizes `ptr::copy` which per the documentation is
            // safe to use for overlapping areas. The only invariants we have to
            // keep track of are:
            // - `src` is valid data
            // - `dst` is owned and capable of containing `count` bytes
            // `src` points to be beginning of the `unprocessed` data and is valid.
            // `dst` is merely a `*mut` to start of `self.buf`, so it is owned.
            // `count` must be less than `self.buf.len()` because `unprocessed`
            // is fully contained within `self.buf`.
            unsafe {
                ptr::copy(unprocessed.as_ptr(), this.buf.as_mut().as_mut_ptr(), count);
            }
        }
        *this.pos += nsrc;
        Poll::Ready(Ok(ndst))
    }
}

unsafe fn uninit_buf(size: usize) -> Box<[u8]> {
    let layout = Layout::array::<u8>(size).unwrap();
    let ptr = slice_from_raw_parts_mut(alloc(layout), size);
    Box::from_raw(ptr)
}
