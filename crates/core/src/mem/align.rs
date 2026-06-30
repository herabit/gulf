use crate::{
    assert_unchecked,
    mem::layout::{
        Mask,
        private::{AlignRepr, clamp, compare, get, max, min},
    },
};
use core::{alloc::Layout, cmp, mem, num::NonZero};

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

// NOTE: Sanity checks, do not remove.
const _: () = assert!(
    size_of::<Align>() == size_of::<usize>(),
    "`size_of::<Align>() != size_of::<usize>()`",
);
const _: () = assert!(
    align_of::<Align>() == align_of::<usize>(),
    "`align_of::<Align>() != align_of::<usize>()`",
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

    #[inline(always)]
    #[track_caller]
    #[must_use]
    pub const fn from_nonzero(align: NonZero<usize>) -> Option<Align> {
        if align.is_power_of_two() {
            // SAFETY: We know `align` is a power-of-two.
            Some(unsafe { mem::transmute(align) })
        } else {
            None
        }
    }

    #[inline(always)]
    #[must_use]
    #[track_caller]
    pub const unsafe fn from_nonzero_unchecked(align: NonZero<usize>) -> Align {
        // SAFETY: The caller ensures that `align` is a power-of-two.
        unsafe { assert_unchecked!(align.is_power_of_two(), "alignment is not a power-of-two") };

        // SAFETY: See above.
        unsafe { mem::transmute(align) }
    }

    #[inline(always)]
    #[track_caller]
    #[must_use]
    pub const fn new(align: usize) -> Option<Align> {
        if align.is_power_of_two() {
            // SAFETY: We know `align` is a power-of-two.
            Some(unsafe { mem::transmute(align) })
        } else {
            None
        }
    }

    #[inline(always)]
    #[track_caller]
    #[must_use]
    pub const unsafe fn new_unchecked(align: usize) -> Align {
        // SAFETY: The caller ensures that `align` is a power-of-two.
        unsafe { assert_unchecked!(align.is_power_of_two(), "alignment is not a power-of-two") };

        // SAFETY: See above.
        unsafe { mem::transmute(align) }
    }

    #[inline(always)]
    #[track_caller]
    #[must_use]
    pub const fn of<T>() -> Align {
        const {
            // SAFETY: `align_of` always returns a valid alignment.
            unsafe { Align::new_unchecked(align_of::<T>()) }
        }
    }

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

    #[inline(always)]
    #[track_caller]
    #[must_use]
    pub const fn of_layout(layout: &Layout) -> Align {
        // SAFETY: `Layout::align` always yields a valid alignment.
        unsafe { Align::new_unchecked(layout.align()) }
    }

    #[inline(always)]
    #[track_caller]
    #[must_use]
    pub const fn of_addr(addr: usize) -> Option<Align> {
        match NonZero::new(addr) {
            Some(addr) => Some({
                let align = 1_usize.strict_shl(addr.trailing_zeros());
                Align::new(align).expect("undefined behavior: `1_usize.strict_shl(addr.trailing_zeros())` is not a valid alignment")
            }),
            None => None,
        }
    }

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

    #[inline(always)]
    #[track_caller]
    #[must_use]
    pub const fn get(self) -> usize {
        self.get_nonzero().get()
    }

    #[inline(always)]
    #[track_caller]
    #[must_use]
    pub const fn compare(
        self,
        rhs: Align,
    ) -> cmp::Ordering {
        compare(self.get(), rhs.get())
    }

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

    #[inline(always)]
    #[track_caller]
    #[must_use]
    pub const fn mask(self) -> Mask {
        Mask::from_align(self)
    }

    #[inline(always)]
    #[track_caller]
    #[must_use]
    pub const fn from_mask(mask: Mask) -> Align {
        mask.align()
    }
}
