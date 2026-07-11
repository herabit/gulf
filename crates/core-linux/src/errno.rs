//! Utilities for handling Linux errors.

use core::{
    borrow::{Borrow, BorrowMut},
    cmp::Ordering,
    ffi::{CStr, c_int},
    fmt::{self, Alignment},
    hash,
    iter::FusedIterator,
    mem::{self, MaybeUninit},
    ops::ControlFlow,
    str::Utf8Chunks,
};

/// An enum describing a Linux error number.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
#[repr(transparent)]
pub struct Errno(pub c_int);

impl Errno {
    /// Get the last OS error that has occurred.
    #[inline(always)]
    #[must_use]
    pub fn last_error() -> Errno {
        // SAFETY: It is always safe to read from `__errno_location` as it always
        //         points to the errno for the current thread.
        Errno(unsafe { libc::__errno_location().read() })
    }

    /// Set the last OS error to this error.
    #[inline(always)]
    #[must_use]
    pub fn set_last_error(self) {
        // SAFETY: It is always safe to write to `__errno_location` as it always
        //         points to the errno for the current thread.
        unsafe { libc::__errno_location().write(self.0) }
    }

    /// Attempt to encode this [`Errno`] as a value that could be used in
    /// a syscall.
    #[inline(always)]
    #[must_use]
    pub const fn to_syscall(self) -> Result<(), isize> {
        match self.0 {
            error @ 1..4096 => Err(-(error as isize)),
            _value @ ..1 | _value @ 4096.. => Ok(()),
        }
    }

    /// Create an [`Error`] from the returned value from a syscall.
    #[inline(always)]
    #[must_use]
    pub const fn from_syscall(return_value: isize) -> Result<isize, Errno> {
        match return_value {
            // NOTE: Apparently syscalls will never return a non-error value within
            //       the range of `(-4095..0)` (So `-1` down to and including `-4095`).
            error @ -4095..0 => Err(Errno(-(error as i32))),
            value @ ..-4095 | value @ 0.. => Ok(value),
        }
    }

    /// Get a [`std::io::ErrorKind`] for this error.
    #[inline(always)]
    #[must_use]
    #[cfg(feature = "std")]
    pub fn error_kind(self) -> std::io::ErrorKind {
        // FIXME: See the TryFrom impl.
        let error = mem::ManuallyDrop::new(std::io::Error::from(self));

        error.kind()
    }

    /// Attempt to get the string for this error.
    ///
    /// # Buffer contents.
    ///
    /// We fill the buffer with zeros prior to using it.
    pub fn write_details<'s>(
        self,
        buffer: &'s mut [MaybeUninit<u8>],
    ) -> Result<&'s CStr, Errno> {
        let buffer = {
            buffer.fill(MaybeUninit::new(0x00));

            // SAFETY: It's a byte buffer and we just zeroed it out.
            unsafe { buffer.assume_init_mut() }
        };

        // SAFETY: libc promises to not be naughty.
        let result = unsafe {
            libc::strerror_r(
                self.0,
                (&raw mut *buffer).cast(),
                (&raw const *buffer).len(),
            )
        };
        match result {
            0 => cfg_select! {
                any(debug_assertions, miri) => match CStr::from_bytes_until_nul(buffer) {
                    Ok(string) => Ok(string),
                    Err(..) => panic!("undefined behavior: `strerror_r` gave us a string with no NUL terminator"),
                },
                // SAFETY: We're assuming `strerror_r` works properly in release builds.
                _ => Ok(unsafe { CStr::from_ptr(buffer.as_ptr().cast()) }),
            },
            error @ 1.. => Err(Errno(error)),
            ..0 => Err(Errno::last_error()),
        }
    }

    #[inline(always)]
    fn details<'s>(
        self,
        buffer: &'s mut [MaybeUninit<u8>],
    ) -> Result<Details<'s>, Errno> {
        self.write_details(buffer)
            .map(|details| Details::NextChunk {
                iter: details.to_bytes().utf8_chunks(),
            })
    }
}

/// How we do the [`fmt::Debug`] and [`fmt::Display`] implementations without any allocations.
///
/// Weeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee.
#[derive(Clone, Default)]
enum Details<'a> {
    NextChunk {
        iter: Utf8Chunks<'a>,
    },
    InvalidChunk {
        iter: Utf8Chunks<'a>,
    },
    #[default]
    Done,
}

impl<'a> Details<'a> {
    #[inline(always)]
    fn process(&mut self) -> ControlFlow<Option<&'a str>> {
        match mem::take(self) {
            Details::NextChunk { mut iter } => match iter.next() {
                Some(chunk) => {
                    *self = match chunk.invalid().len() {
                        1.. => Details::InvalidChunk { iter },
                        0 => Details::NextChunk { iter },
                    };

                    match chunk.valid().len() {
                        1.. => ControlFlow::Break(Some(chunk.valid())),
                        0 => ControlFlow::Continue(()),
                    }
                }
                None => ControlFlow::Break(None),
            },
            Details::InvalidChunk { iter } => {
                *self = Details::NextChunk { iter };

                ControlFlow::Break(Some("\u{FFFD}"))
            }
            Details::Done => ControlFlow::Break(None),
        }
    }
}

impl<'a> Iterator for Details<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        // NOTE: There is a better way to do this, I just wanted a state machine.
        loop {
            match self.process() {
                ControlFlow::Continue(()) => continue,
                ControlFlow::Break(next) => break next,
            }
        }
    }
}

impl<'a> FusedIterator for Details<'a> {}

/// We get the first `max_char_count` characters from our iterator of valid UTF-8 strings.
///
/// The algorithm *may* be improved, but we specifically avoid allocations with this.
///
/// Would it be better to allocate a temporary buffer? Probably. But I, in my hatred of all things
/// holy, would rather this crate be `no_std` and `alloc`-free!
///
/// Why? Zero reasonable reason. Doctors are still studing me to this day.
fn with_max<'s>(
    details: Details<'s>,
    max_char_count: usize,
) -> (usize, impl Iterator<Item = &'s str>) {
    let result = details
        .clone()
        .enumerate()
        .try_fold(0_usize, |mut total_count, (index, s)| {
            let current_count = s.chars().count();
            total_count = total_count + current_count;

            if total_count < max_char_count {
                return ControlFlow::Continue(total_count);
            }

            let mut chars = s.chars();
            let char_index = total_count - max_char_count;
            let char = chars.nth(char_index).expect("FUCK");

            total_count = (total_count - current_count) + char_index;

            let end_addr = chars.as_str().as_ptr().addr() - char.len_utf8();
            let start_addr = s.as_ptr().addr();

            let byte_index = end_addr - start_addr;

            ControlFlow::Break((total_count, index, &s[..byte_index]))
        });

    match result {
        ControlFlow::Continue(char_count) => (char_count, details.take(usize::MAX).chain(None)),
        ControlFlow::Break((char_count, take_count, final_string)) => (
            char_count,
            details.take(take_count).chain(Some(final_string)),
        ),
    }
}

// See the str Display impl
impl<'a> fmt::Display for Details<'a> {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        if f.width().is_none() && f.precision().is_none() {
            return self.clone().try_for_each(|s| f.write_str(s));
        }

        let (char_count, strings) = match f.precision() {
            Some(max_char_count) => {
                let (char_count, iter) = with_max(self.clone(), max_char_count);

                (char_count, Ok(iter))
            }
            None => (
                self.clone().map(|s| s.chars().count()).sum(),
                Err(self.clone()),
            ),
        };

        let write_strings = move |f: &mut fmt::Formatter<'_>| match strings {
            Ok(mut iter) => iter.try_for_each(|s| f.write_str(s)),
            Err(mut iter) => iter.try_for_each(|s| f.write_str(s)),
        };

        let width = f.width().unwrap_or(0);

        if char_count < width {
            let padding = char_count - width;
            let padding_left = match f.align().unwrap_or(Alignment::Left) {
                Alignment::Left => 0,
                Alignment::Right => padding,
                Alignment::Center => padding / 2,
            };
            let padding_right = padding - padding_left;

            // `write_char` for most implementations does this repeatedly,
            // yet most writers prefer strings. Caching the value is just,
            // better. *glares at the stdlib*
            let mut fill = [0_u8; 4];
            let fill = f.fill().encode_utf8(&mut fill);

            (0..padding_left)
                .into_iter()
                .try_for_each(|_| f.write_str(fill))?;

            write_strings(f)?;

            (0..padding_right)
                .into_iter()
                .try_for_each(|_| f.write_str(fill))?;

            Ok(())
        } else {
            write_strings(f)
        }
    }
}

// See the str Debug impl.
fn debug_str(
    s: &str,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    let mut printable_range = 0..0;

    #[inline(always)]
    fn needs_escape(&b: &u8) -> bool {
        b > 0x7E || b < 0x20 || b == b'\\' || b == b'"'
    }

    let mut rest = s;

    while rest.len() > 0 {
        let Some(non_printable_start) = rest.as_bytes().iter().position(needs_escape) else {
            printable_range.end += rest.len();
            break;
        };

        printable_range.end += non_printable_start;

        // SAFETY: We know this to always be in bounds.
        rest = unsafe { rest.get_unchecked(non_printable_start..) };

        let mut chars = rest.chars();

        if let Some(char) = chars.next() {
            let esc = char.escape_debug();

            if esc.size_hint().1.unwrap() != 1 {
                f.write_str(&s[printable_range.clone()])?;
                fmt::Display::fmt(&esc, f)?;
                printable_range.start = printable_range.end + char.len_utf8();
            }

            printable_range.end += char.len_utf8();
        }

        rest = chars.as_str();
    }

    f.write_str(&s[printable_range])?;

    Ok(())
}

impl<'a> fmt::Debug for Details<'a> {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        f.write_str("\"")?;

        self.clone().try_for_each(|s| debug_str(s, f))?;

        f.write_str("\"")?;

        Ok(())
    }
}

/// WEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE
const TEMP_BUF: [MaybeUninit<u8>; 1024] = [MaybeUninit::uninit(); _];

impl fmt::Display for Errno {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        match self.details(&mut { TEMP_BUF }) {
            Ok(details) => core::write!(f, "{details} (os error {code})", code = self.0),
            Err(Errno(error)) => core::write!(
                f,
                "strerror_r failed with {error} (os error {code})",
                code = self.0
            ),
        }
    }
}

impl fmt::Debug for Errno {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        f.debug_struct("Errno")
            .field("code", &self.0)
            .field("details", &self.details(&mut { TEMP_BUF }))
            .finish_non_exhaustive()
    }
}

impl hash::Hash for Errno {
    #[inline(always)]
    fn hash<H: hash::Hasher>(
        &self,
        state: &mut H,
    ) {
        self.0.hash(state);
    }

    #[inline(always)]
    fn hash_slice<H: hash::Hasher>(
        data: &[Self],
        state: &mut H,
    ) {
        // SAFETY: `Errno`s are just fancy `c_int`s.
        let int_slice =
            unsafe { (&raw const *data as *const [Errno] as *const [c_int]).as_ref_unchecked() };

        c_int::hash_slice(int_slice, state);
    }
}

impl From<c_int> for Errno {
    #[inline(always)]
    fn from(error: c_int) -> Self {
        Errno(error)
    }
}

impl From<Errno> for c_int {
    #[inline(always)]
    fn from(Errno(errno): Errno) -> Self {
        errno
    }
}

impl PartialEq<c_int> for Errno {
    #[inline(always)]
    fn eq(
        &self,
        other: &c_int,
    ) -> bool {
        self.0 == *other
    }
}

impl PartialEq<Errno> for c_int {
    #[inline(always)]
    fn eq(
        &self,
        other: &Errno,
    ) -> bool {
        *self == other.0
    }
}

impl PartialOrd<c_int> for Errno {
    #[inline(always)]
    fn partial_cmp(
        &self,
        other: &c_int,
    ) -> Option<Ordering> {
        Some(self.0.cmp(other))
    }
}

impl PartialOrd<Errno> for c_int {
    #[inline(always)]
    fn partial_cmp(
        &self,
        other: &Errno,
    ) -> Option<Ordering> {
        Some(self.cmp(&other.0))
    }
}

impl Borrow<c_int> for Errno {
    #[inline(always)]
    fn borrow(&self) -> &c_int {
        &self.0
    }
}
impl Borrow<Errno> for c_int {
    #[inline(always)]
    fn borrow(&self) -> &Errno {
        // SAFETY: We know `Errno` to be transparent over `c_int`.
        unsafe { (&raw const *self).cast::<Errno>().as_ref_unchecked() }
    }
}

impl BorrowMut<c_int> for Errno {
    #[inline(always)]
    fn borrow_mut(&mut self) -> &mut c_int {
        &mut self.0
    }
}

impl BorrowMut<Errno> for c_int {
    #[inline(always)]
    fn borrow_mut(&mut self) -> &mut Errno {
        // SAFETY: We know `Errno` to be transparent over `c_int`.
        unsafe { (&raw mut *self).cast::<Errno>().as_mut_unchecked() }
    }
}

impl AsRef<c_int> for Errno {
    #[inline(always)]
    fn as_ref(&self) -> &c_int {
        self.borrow()
    }
}

impl AsRef<Errno> for c_int {
    #[inline(always)]
    fn as_ref(&self) -> &Errno {
        self.borrow()
    }
}

impl AsMut<c_int> for Errno {
    #[inline(always)]
    fn as_mut(&mut self) -> &mut c_int {
        self.borrow_mut()
    }
}

impl AsMut<Errno> for c_int {
    #[inline(always)]
    fn as_mut(&mut self) -> &mut Errno {
        self.borrow_mut()
    }
}

impl core::error::Error for Errno {}

#[cfg(feature = "std")]
impl From<Errno> for std::io::Error {
    #[inline(always)]
    fn from(Errno(errno): Errno) -> Self {
        std::io::Error::from_raw_os_error(errno)
    }
}

#[cfg(feature = "std")]
impl TryFrom<std::io::Error> for Errno {
    type Error = std::io::Error;

    #[inline(always)]
    fn try_from(error: std::io::Error) -> Result<Self, Self::Error> {
        match error.raw_os_error() {
            Some(raw_error) => {
                // FIXME: Maybe omit this? For errors that are raw OS errors, this shouldn't
                //        require any destructor, but for some reason the drop impl is not inlined here,
                //        so, yeah... Should be fine.
                core::mem::forget(error);
                Ok(Errno(raw_error))
            }
            None => Err(error),
        }
    }
}
