use crate::{
    assert_unchecked,
    mem::{
        Mask,
        layout::private::{AlignRepr, clamp, compare, get, max, min},
    },
};
use core::{
    alloc::{Layout, LayoutError},
    borrow::Borrow,
    cmp, convert,
    error::{self, Error as _},
    fmt, hash, mem,
    num::NonZero,
    ptr::NonNull,
};

/// An [`Align`] is a power-of-two [`usize`] that represents a possibly-valid
/// alignment for an allocation.
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct Align {
    /// # Safety
    ///
    /// An alignment is just a power-of-two [`usize`].
    repr: AlignRepr,
}

// NOTE: Sanity checks for ensuring the memory layout is as we expect.
//
//       Do not remove these.
const _: () = assert!(
    size_of::<Align>() == size_of::<usize>(),
    "`size_of::<Align>() != size_of::<usize>()`",
);
const _: () = assert!(
    align_of::<Align>() == align_of::<usize>(),
    "`align_of::<Align>() != align_of::<usize>()`",
);

// NOTE: Sanity checks for ensuring the associated constants in the documentation
//       are actually accurate.
//
//       Do not remove these.
const _: () = assert!(
    Align::MIN.repr as usize == get!(align.min),
    get!(align.min.assert),
);
const _: () = assert!(
    Align::MAX.repr as usize == get!(align.max),
    get!(align.max.assert),
);

// NOTE: Sanity checks for ensuring the associated constants have the values
//       we expect.
//
//       Do not remove these.
const _: () = assert!(
    Align::MIN.repr as usize == 1,
    "the minimum alignment is not one",
);
const _: () = assert!(
    Align::MAX.repr as usize == (isize::MAX as usize).strict_add(1),
    "the maximum alignment is not `isize::MAX + 1`",
);

impl Align {
    /// The smallest valid alignment that is supported (2<sup>0</sup> on all targets, or in other words, 1).
    ///
    /// # Examples
    ///
    /// ```
    /// # use gulf_core::mem::Align;
    /// #
    /// // What you'd expect.
    /// assert_eq!(Align::MIN.get(), 1);
    ///
    /// // Just demonstrating that `2^0` holds true here.
    /// assert_eq!(Align::MIN.get(), 2_usize.pow(0));
    ///
    /// // Bitwise version of `2^0`.
    /// assert_eq!(Align::MIN.get(), 1_usize << 0);
    /// ```
    pub const MIN: Align = Align {
        repr: AlignRepr::MIN,
    };

    /// The largest valid alignment that is supported
    #[doc = concat!(
        "(2<sup>",
        get!(bits - 1),
        "</sup> on ",
        get!(bits),
        "-bit platforms)."
    )]
    ///
    /// # Examples
    ///
    /// ```
    /// # use gulf_core::mem::Align;
    /// #
    #[doc = concat!(
        "// What you'd expect.\n",
        "assert_eq!(Align::MAX.get(), ", get!(align.max), ");\n"
    )]
    ///
    #[doc = concat!(
        "// Just demonstrating that `2^", get!(bits - 1), "` holds true here.\n",
        "assert_eq!(Align::MAX.get(), 2_usize.pow(", get!(bits - 1), "));\n",
    )]
    ///
    #[doc = concat!(
        "// Bitwise version of `2^", get!(bits - 1), "`.\n",
        "assert_eq!(Align::MAX.get(), 1_usize << ", get!(bits - 1), ");\n",
    )]
    /// ```
    pub const MAX: Align = Align {
        repr: AlignRepr::MAX,
    };

    /// Create an [`Align`] from a non-zero [`usize`].
    ///
    /// # Returns
    ///
    /// Returns [`None`] if `align` is not a power-of-two.
    #[inline(always)]
    #[track_caller]
    #[must_use]
    pub const fn from_nonzero(align: NonZero<usize>) -> Option<Align> {
        if align.is_power_of_two() {
            // SAFETY: We know `align` is a power-of-two.
            #[allow(clippy::missing_transmute_annotations)]
            Some(unsafe { mem::transmute(align) })
        } else {
            None
        }
    }

    /// Create an [`Align`] from a non-zero [`usize`] without any checks.
    ///
    /// # Safety
    ///
    /// The caller must ensure `align` is a power-of-two.
    #[inline(always)]
    #[must_use]
    #[track_caller]
    pub const unsafe fn from_nonzero_unchecked(align: NonZero<usize>) -> Align {
        // SAFETY: The caller ensures that `align` is a power-of-two.
        unsafe { assert_unchecked!(align.is_power_of_two(), "alignment is not a power-of-two") };

        // SAFETY: See above.
        unsafe { mem::transmute(align) }
    }

    /// Create an [`Align`] from a [`usize`].
    ///
    /// # Returns
    ///
    /// Returns [`None`] if `align` is not a power-of-two.
    #[inline(always)]
    #[track_caller]
    #[must_use]
    pub const fn new(align: usize) -> Option<Align> {
        if align.is_power_of_two() {
            // SAFETY: We know `align` is a power-of-two.
            #[allow(clippy::missing_transmute_annotations)]
            Some(unsafe { mem::transmute(align) })
        } else {
            None
        }
    }

    /// Create an [`Align`] from a [`usize`] without any checks.
    ///
    /// # Safety
    ///
    /// The caller must ensure `align` is a power-of-two.
    #[inline(always)]
    #[track_caller]
    #[must_use]
    pub const unsafe fn new_unchecked(align: usize) -> Align {
        // SAFETY: The caller ensures that `align` is a power-of-two.
        unsafe { assert_unchecked!(align.is_power_of_two(), "alignment is not a power-of-two") };

        // SAFETY: See above.
        unsafe { mem::transmute(align) }
    }

    /// Get the [`Align`] of some [`Layout`].
    #[inline(always)]
    #[track_caller]
    #[must_use]
    pub const fn from_layout(layout: Layout) -> Align {
        // SAFETY: `Layout::align` always yields a valid alignment.
        unsafe { Align::new_unchecked(layout.align()) }
    }

    /// Get the alignment of a non-zero memory address.
    #[inline(always)]
    #[track_caller]
    #[must_use]
    pub const fn from_addr_nonzero(addr: NonZero<usize>) -> Align {
        let align = 1_usize.strict_shl(addr.trailing_zeros());

        Align::new(align).unwrap()
    }

    /// Get the alignment of a memory address.
    ///
    /// # Returns
    ///
    /// Returns [`None`] if the address is null.
    #[inline(always)]
    #[track_caller]
    #[must_use]
    pub const fn from_addr(addr: usize) -> Option<Align> {
        match NonZero::new(addr) {
            Some(addr) => Some(Align::from_addr_nonzero(addr)),
            None => None,
        }
    }

    /// Get the alignment of the type `T`.
    #[inline(always)]
    #[track_caller]
    #[must_use]
    pub const fn of<T>() -> Align {
        const {
            // SAFETY: `align_of` always returns a valid alignment.
            unsafe { Align::new_unchecked(align_of::<T>()) }
        }
    }

    /// Get the alignment of `val`. This is *not* the alignment of the memory address for `val`.
    ///
    /// If you need to get the alignment of the address, use [`Align::from_addr`] or [`Align::from_addr_nonzero`].
    #[inline(always)]
    #[track_caller]
    #[must_use]
    pub const fn of_val<T>(val: &T) -> Align
    where
        T: ?Sized,
    {
        // SAFETY: `align_of_val` always returns a valid alignment.
        unsafe { Align::new_unchecked(align_of_val(val)) }
    }

    /// Get this alignment as a non-zero [`usize`].
    #[inline(always)]
    #[track_caller]
    #[must_use]
    pub const fn get_nonzero(self) -> NonZero<usize> {
        // SAFETY: We know that alignments are always nonzero.
        let align = unsafe { NonZero::new_unchecked(self.repr as usize) };

        // SAFETY: An alignment is always a power-of-two. This also ensures it is nonzero in debug builds.
        unsafe { assert_unchecked!(align.is_power_of_two(), "alignment is not a power-of-two") };

        align
    }

    /// Get this alignment as a [`usize`].
    #[inline(always)]
    #[track_caller]
    #[must_use]
    pub const fn get(self) -> usize {
        self.get_nonzero().get()
    }

    /// Calculate the [`Mask`] for this alignment.
    #[inline(always)]
    #[track_caller]
    #[must_use]
    pub const fn mask(self) -> Mask {
        Mask::from_align(self)
    }

    /// Calculate the base-two logarithm of this alignment.
    #[inline(always)]
    #[track_caller]
    #[must_use]
    pub const fn log2(self) -> u32 {
        self.get_nonzero().trailing_zeros()
    }

    //// Calculate the alignment for a [`Mask`].
    #[inline(always)]
    #[track_caller]
    #[must_use]
    pub const fn from_mask(mask: Mask) -> Align {
        mask.align()
    }

    /// Create a dangling pointer that is sufficiently aligned to
    /// this alignment.
    #[inline(always)]
    #[track_caller]
    #[must_use]
    pub const fn dangling(self) -> NonNull<u8> {
        NonNull::without_provenance(self.get_nonzero())
    }

    /// Create a [`Layout`] for this alignment and some size.
    #[inline(always)]
    #[track_caller]
    pub const fn layout_with_size(
        self,
        size: usize,
    ) -> Result<Layout, LayoutError> {
        Layout::from_size_align(size, self.get())
    }

    /// Round up some size to some multiple of this alignment.
    ///
    /// # Returns
    ///
    /// Returns [`None`] if rounding up the size to be some multiple of `self`
    /// overflows.
    #[inline(always)]
    #[track_caller]
    #[must_use]
    pub const fn round_up_size(
        self,
        size: usize,
    ) -> Option<NonZero<usize>> {
        // NOTE: We don't care about overflow with the summation, as
        //       we mask off the bits that'd be set upon overflow after.
        let new_size = size.wrapping_add(self.get().strict_sub(1)) & self.mask().get();

        // NOTE: Since the result on overflow is zero, this never panics on the overflow case.
        assert!(
            new_size.is_multiple_of(self.get()),
            "size is not a multiple of the alignment",
        );

        NonZero::new(new_size)
    }

    /// Calculate the maximum size for this alignment.
    #[inline(always)]
    #[track_caller]
    #[must_use]
    pub const fn max_size(self) -> usize {
        Align::MAX.get().strict_sub(self.get())
    }

    /// Perform [`Ord::cmp`] in a `const`-friendly manner.
    #[inline(always)]
    #[track_caller]
    #[must_use]
    pub const fn compare(
        self,
        rhs: Align,
    ) -> cmp::Ordering {
        compare(self.get(), rhs.get())
    }

    /// Perform [`Ord::min`] in a `const`-friendly manner.
    #[inline(always)]
    #[track_caller]
    #[must_use]
    pub const fn min(
        self,
        other: Align,
    ) -> Align {
        // SAFETY: We're simply getting the minimum of two alignments, thus the returned value is still an alignment.
        unsafe { Align::new_unchecked(min(self.get(), other.get())) }
    }

    /// Perform [`Ord::max`] in a `const`-friendly manner.
    #[inline(always)]
    #[track_caller]
    #[must_use]
    pub const fn max(
        self,
        other: Align,
    ) -> Align {
        // SAFETY: We're simple getting the maximum of two alignments, thus the returned value is still an alignment.
        unsafe { Align::new_unchecked(max(self.get(), other.get())) }
    }

    /// Perform [`Ord::clamp`] in a `const`-friendly manner.
    #[inline(always)]
    #[track_caller]
    #[must_use]
    pub const fn clamp(
        self,
        min: Align,
        max: Align,
    ) -> Option<Align> {
        match clamp(self.get(), min.get(), max.get()) {
            // SAFETY: The clamped value of an alignment against other alignments, is an alignment.
            Some(align) => Some(unsafe { Align::new_unchecked(align) }),
            None => None,
        }
    }
}

impl From<Align> for NonZero<usize> {
    #[inline(always)]
    fn from(align: Align) -> Self {
        align.get_nonzero()
    }
}

impl From<Align> for usize {
    #[inline(always)]
    fn from(align: Align) -> Self {
        align.get()
    }
}

impl<T> From<Align> for NonNull<T> {
    #[inline(always)]
    fn from(align: Align) -> Self {
        align.dangling().cast()
    }
}

impl<T> From<Align> for *const T {
    #[inline(always)]
    fn from(align: Align) -> Self {
        align.dangling().as_ptr().cast()
    }
}

impl<T> From<Align> for *mut T {
    #[inline(always)]
    fn from(align: Align) -> Self {
        align.dangling().as_ptr().cast()
    }
}

impl From<Mask> for Align {
    #[inline(always)]
    fn from(mask: Mask) -> Self {
        mask.align()
    }
}

impl TryFrom<NonZero<usize>> for Align {
    type Error = AlignError;

    #[inline(always)]
    fn try_from(align: NonZero<usize>) -> Result<Self, Self::Error> {
        Align::from_nonzero(align).ok_or(AlignError(()))
    }
}

impl TryFrom<usize> for Align {
    type Error = AlignError;

    #[inline(always)]
    fn try_from(align: usize) -> Result<Self, Self::Error> {
        Align::new(align).ok_or(AlignError(()))
    }
}

// NOTE: The ptr-to-align conversions get the alignment of the pointers!!!!!!

impl<T> From<NonNull<T>> for Align
where
    T: ?Sized,
{
    #[inline(always)]
    fn from(ptr: NonNull<T>) -> Self {
        Align::from_addr_nonzero(ptr.addr())
    }
}

impl<T> TryFrom<*const T> for Align
where
    T: ?Sized,
{
    type Error = AlignError;

    #[inline(always)]
    fn try_from(ptr: *const T) -> Result<Self, Self::Error> {
        Align::from_addr(ptr.addr()).ok_or(AlignError(()))
    }
}

impl<T> TryFrom<*mut T> for Align
where
    T: ?Sized,
{
    type Error = AlignError;

    #[inline(always)]
    fn try_from(ptr: *mut T) -> Result<Self, Self::Error> {
        Align::from_addr(ptr.addr()).ok_or(AlignError(()))
    }
}

impl From<Layout> for Align {
    #[inline(always)]
    fn from(layout: Layout) -> Self {
        Align::from_layout(layout)
    }
}

/// An error for when an [`Align`] could not be created.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AlignError(pub(crate) ());

impl From<convert::Infallible> for AlignError {
    #[inline(always)]
    fn from(value: convert::Infallible) -> Self {
        match value {}
    }
}

impl From<crate::num::NotPowerOfTwo> for AlignError {
    #[inline(always)]
    fn from(_: crate::num::NotPowerOfTwo) -> Self {
        AlignError(())
    }
}

impl fmt::Display for AlignError {
    #[inline]
    #[allow(deprecated)]
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        f.write_str(self.description())
    }
}

impl error::Error for AlignError {
    #[allow(deprecated)]
    fn description(&self) -> &str {
        "could not create an `Align`"
    }
}

impl Default for Align {
    #[inline(always)]
    fn default() -> Self {
        Align::MIN
    }
}

#[allow(clippy::non_canonical_partial_ord_impl)]
impl PartialOrd for Align {
    #[inline(always)]
    fn partial_cmp(
        &self,
        other: &Self,
    ) -> Option<cmp::Ordering> {
        Some(self.compare(*other))
    }
}

impl Ord for Align {
    #[inline(always)]
    fn cmp(
        &self,
        other: &Self,
    ) -> cmp::Ordering {
        self.compare(*other)
    }

    #[inline(always)]
    #[track_caller]
    fn clamp(
        self,
        min: Self,
        max: Self,
    ) -> Self {
        <Align>::clamp(self, min, max).expect("`max < min`")
    }

    #[inline(always)]
    fn max(
        self,
        other: Self,
    ) -> Self {
        <Align>::max(self, other)
    }

    #[inline(always)]
    fn min(
        self,
        other: Self,
    ) -> Self {
        <Align>::min(self, other)
    }
}

impl hash::Hash for Align {
    #[inline(always)]
    fn hash<H>(
        &self,
        state: &mut H,
    ) where
        H: hash::Hasher,
    {
        state.write_usize(self.get());
    }

    #[inline(always)]
    fn hash_slice<H>(
        data: &[Self],
        state: &mut H,
    ) where
        H: hash::Hasher,
    {
        // SAFETY: `Align`s are just `usize`s in disguise.
        let data = unsafe { (&raw const *data as *const [usize]).as_ref_unchecked() };

        usize::hash_slice(data, state);
    }
}

impl fmt::Debug for Align {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        write!(f, "{:?} (1 << {:?})", self.get(), self.log2())
    }
}

impl PartialEq<usize> for Align {
    #[inline(always)]
    fn eq(
        &self,
        other: &usize,
    ) -> bool {
        self.get() == *other
    }
}

impl PartialOrd<usize> for Align {
    #[inline(always)]
    fn partial_cmp(
        &self,
        other: &usize,
    ) -> Option<cmp::Ordering> {
        self.get().partial_cmp(other)
    }
}

impl PartialEq<NonZero<usize>> for Align {
    #[inline(always)]
    fn eq(
        &self,
        other: &NonZero<usize>,
    ) -> bool {
        self.get_nonzero() == *other
    }
}

impl PartialOrd<NonZero<usize>> for Align {
    #[inline(always)]
    fn partial_cmp(
        &self,
        other: &NonZero<usize>,
    ) -> Option<cmp::Ordering> {
        self.get_nonzero().partial_cmp(other)
    }
}

impl PartialEq<Align> for usize {
    #[inline(always)]
    fn eq(
        &self,
        other: &Align,
    ) -> bool {
        *self == other.get()
    }
}

impl PartialOrd<Align> for usize {
    #[inline(always)]
    fn partial_cmp(
        &self,
        other: &Align,
    ) -> Option<cmp::Ordering> {
        self.partial_cmp(&other.get())
    }
}

impl PartialEq<Align> for NonZero<usize> {
    #[inline(always)]
    fn eq(
        &self,
        other: &Align,
    ) -> bool {
        *self == other.get_nonzero()
    }
}

impl PartialOrd<Align> for NonZero<usize> {
    #[inline(always)]
    fn partial_cmp(
        &self,
        other: &Align,
    ) -> Option<cmp::Ordering> {
        self.partial_cmp(&other.get_nonzero())
    }
}

impl AsRef<usize> for Align {
    #[inline(always)]
    fn as_ref(&self) -> &usize {
        // SAFETY: An `Align` is just a `usize` in disguise.
        let this = unsafe { (&raw const *self).cast::<usize>().as_ref_unchecked() };

        // SAFETY: We're just providing hints to the compiler that `this` is a valid alignment.
        unsafe { _ = Align::new_unchecked(*this) };

        this
    }
}

impl AsRef<NonZero<usize>> for Align {
    #[inline(always)]
    fn as_ref(&self) -> &NonZero<usize> {
        // SAFETY: An `Align` is a power-of-two `usize` in disguise.
        let this = unsafe {
            (&raw const *self)
                .cast::<NonZero<usize>>()
                .as_ref_unchecked()
        };

        // SAFETY: We're just providing hints to the compiler that `this` is a valid alignment.
        unsafe { _ = Align::from_nonzero_unchecked(*this) };

        this
    }
}

impl Borrow<usize> for Align {
    #[inline(always)]
    fn borrow(&self) -> &usize {
        self.as_ref()
    }
}

impl Borrow<NonZero<usize>> for Align {
    #[inline(always)]
    fn borrow(&self) -> &NonZero<usize> {
        self.as_ref()
    }
}
