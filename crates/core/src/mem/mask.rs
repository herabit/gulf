use crate::{
    assert_unchecked,
    mem::layout::{
        Align,
        private::{MaskRepr, clamp, compare, get, has_hole, max, min},
    },
    unreachable_unchecked,
};
use core::{alloc::Layout, cmp, mem, num::NonZero};

/// A [`usize`] bitmask that stores what bits are used for an address that is
/// aligned to some [`Align`]. All leading bits are one, and all trailing bits
/// are zero, with no holes in the middle.
///
/// This can be used to match a given alignment, being equivalent to `!(align - 1)`.
///
/// # Safety
///
/// While this type has the same memory layout as a [`usize`], it has stricter bit validity
/// requirements:
///
/// > A mask is a nonzero [`usize`] that has `n` leading ones, and `usize::BITS - n` trailing zeros,
/// > where `n > 0 && n <= usize::BITS` is always true.
/// >
/// > As such, a dumb check for confirming that a mask is valid would be to do
/// > `mask.leading_ones().strict_add(mask.trailing_zeros()) == usize::BITS`.
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct Mask {
    /// # Safety
    ///
    /// A mask can always be created from a valid alignment through `!(align.unchecked_sub(1))`,
    /// and a valid alignment can always be created from a valid mask through `(!mask).unchecked_add(1))`.
    repr: MaskRepr,
}

// NOTE: Sanity checks, do not remove.
const _: () = assert!(
    size_of::<Mask>() == size_of::<usize>(),
    "`size_of::<Mask>() != size_of::<usize>()`",
);
const _: () = assert!(
    align_of::<Mask>() == align_of::<usize>(),
    "`align_of::<Mask>() != align_of::<usize>()`",
);

// NOTE: Sanity checks, do not remove. We have the messages defined by the macro due to,
//       an issue when resolving the `get` macro. Such is life.
const _: () = assert!(
    !has_hole(get!(mask.docs.without_hole)),
    get!(mask.docs.without_hole.assert_message),
);
const _: () = assert!(
    has_hole(get!(mask.docs.with_hole)),
    get!(mask.docs.with_hole.assert_message),
);

impl Mask {
    pub const MIN: Mask = Mask {
        repr: MaskRepr::MIN,
    };

    pub const MAX: Mask = Mask {
        repr: MaskRepr::MAX,
    };

    #[inline(always)]
    #[track_caller]
    #[must_use]
    pub const fn from_nonzero(mask: NonZero<usize>) -> Option<Mask> {
        let leading_ones_mask = mask.get();
        let trailing_zeros_mask = 1_usize.strict_shl(mask.trailing_zeros()).strict_sub(1);

        // NOTE: If we get a mask of leading ones and trailing zeros, and compute the union
        //       of the two, if all bits are set, then we know it's valid.
        if (leading_ones_mask ^ trailing_zeros_mask) == usize::MAX {
            // SAFETY: We know that `mask` is a valid alignment mask.
            let _ = unsafe { Align::new_unchecked((!mask.get()).unchecked_add(1)) };

            // SAFETY: See above.
            Some(unsafe { mem::transmute(mask) })
        } else {
            None
        }
    }

    #[inline(always)]
    #[track_caller]
    #[must_use]
    pub const unsafe fn from_nonzero_unchecked(mask: NonZero<usize>) -> Mask {
        match Mask::from_nonzero(mask) {
            Some(mask) => mask,
            None => unsafe { unreachable_unchecked!("invalid alignment mask") },
        }
    }

    #[inline(always)]
    #[track_caller]
    #[must_use]
    pub const fn new(mask: usize) -> Option<Mask> {
        match NonZero::new(mask) {
            Some(mask) => Mask::from_nonzero(mask),
            None => None,
        }
    }

    #[inline(always)]
    #[track_caller]
    #[must_use]
    pub const unsafe fn new_unchecked(mask: usize) -> Mask {
        match Mask::new(mask) {
            Some(mask) => mask,
            None => unsafe { unreachable_unchecked!("invalid alignment mask") },
        }
    }

    #[inline(always)]
    #[track_caller]
    #[must_use]
    pub const fn from_align(align: Align) -> Mask {
        // SAFETY: It is always sound to create a `Mask` from calculating `!(align - 1)`.
        unsafe { mem::transmute(!align.get().strict_sub(1)) }
    }

    #[inline(always)]
    #[track_caller]
    #[must_use]
    pub const fn align(self) -> Align {
        // SAFETY: It is always sound to create a `Mask` from calculating `(!mask) + 1`.
        Align::new((!self.get()).strict_add(1)).unwrap()
    }

    #[inline(always)]
    #[track_caller]
    #[must_use]
    pub const fn get_nonzero(self) -> NonZero<usize> {
        // SAFETY: Alignment masks are always nonzero.
        let mask: NonZero<usize> = unsafe { NonZero::new_unchecked(self.repr as usize) };

        // SAFETY: Alignment masks can always be turned into alignments.
        unsafe {
            assert_unchecked!(
                (!mask.get()).unchecked_add(1).is_power_of_two(),
                "mask does not map to an alignment",
            )
        };

        mask
    }

    #[inline(always)]
    #[track_caller]
    #[must_use]
    pub const fn get(self) -> usize {
        self.get_nonzero().get()
    }
}
