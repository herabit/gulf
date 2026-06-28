use core::{alloc::Layout, cmp::Ordering, mem, num::NonZero};

use crate::assert_unchecked;

/// A structure representing a valid alignment in memory.
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct Align {
    /// # Safety
    ///
    /// An alignment is just a power-of-two [`usize`].
    repr: AlignRepr,
}

impl Align {
    #[doc = concat!(
        "The minimum valid alignment (`",
        self::value!(align.min),
        "`) on this platform.",
    )]
    pub const MIN: Align = Align {
        repr: AlignRepr::MIN,
    };

    #[doc = concat!(
        "The maximum valid alignment (`",
        self::value!(align.max),
        "`) on this platform.",
    )]
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
    pub const fn of_addr_nonzero(addr: NonZero<usize>) -> Align {
        // NOTE: We're using safe stuff just in case I mess up the logic.
        Align::new(1_usize.strict_shl(addr.trailing_zeros()))
            .expect("somehow got an invalid alignment")
    }

    #[inline(always)]
    #[track_caller]
    #[must_use]
    pub const fn of_addr(addr: usize) -> Option<Align> {
        match NonZero::new(addr) {
            Some(addr) => Some(Align::of_addr_nonzero(addr)),
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

/// A structure representing a valid mask for some allocation in memory.
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct Mask {
    /// # Safety
    ///
    /// A mask can always be created from a valid alignment through `!(align.unchecked_sub(1))`,
    /// and a valid alignment can always be created from a valid mask through `(!mask).unchecked_add(1))`.
    repr: MaskRepr,
}

impl Mask {
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
        Align::new(usize::strict_add(!self.get(), 1)).unwrap()
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

#[inline(always)]
#[must_use]
const fn compare(
    lhs: NonZero<usize>,
    rhs: NonZero<usize>,
) -> Ordering {
    match (lhs.get() > rhs.get(), lhs.get() < rhs.get()) {
        (true, false) => Ordering::Greater,
        (false, false) => Ordering::Equal,
        (false, true) => Ordering::Less,
        (true, true) => unreachable!(),
    }
}

macro_rules! repr {
    {
        "16" => [$($_16:ident),+ $(,)?],
        "32" => [$($_32:ident),+ $(,)?],
        "64" => [$($_64:ident),+ $(,)?]

        $(,)?
    } => {
        cfg_select! {
            target_pointer_width = "16" => {
                repr!(@generate {
                    variants: [$($_16,)+],
                    align_range: 0x0001..=0x8000,
                    mask_range:  0x8000..=0xFFFF,
                });
            }
            target_pointer_width = "32" => {
                repr!(@generate {
                    variants: [$($_16,)+ $($_32,)+],
                    align_range: 0x0000_0001..=0x8000_0000,
                    mask_range:  0x8000_0000..=0xFFFF_FFFF,
                });
            }
            target_pointer_width = "64" => {
                repr!(@generate {
                    variants: [$($_16,)+ $($_32,)+ $($_64,)+],
                    align_range: 0x0000_0000_0000_0001..=0x8000_0000_0000_0000,
                    mask_range:  0x8000_0000_0000_0000..=0xFFFF_FFFF_FFFF_FFFF,
                });
            }
            // _ => {}
        }
    };
    (@generate {
        variants: [$($variant:ident),+ $(,)?],
        align_range: $min_align:tt..=$max_align:tt,
        mask_range: $min_mask:tt..=$max_mask:tt
        $(,)?
    }) => {
        macro_rules! value {
            (align.min) => { $min_align };
            (align.max) => { $max_align };
            (mask.min) => { $min_mask };
            (mask.max) => { $max_mask };
        }

        pub(self) use value;

        /// This associates a variant with how many zeros are before the bit representing
        /// alignments and masks.
        #[repr(u32)]
        enum AlignShift {
            $($variant),+
        }

        /// This is the actual representation alignments.
        ///
        /// The variant names have no say in the actual value.
        #[derive(Clone, Copy, PartialEq, Eq)]
        #[repr(usize)]
        enum AlignRepr {
            $($variant = 1_usize.strict_shl(AlignShift::$variant as u32),)+
        }

        impl AlignRepr {
            const MIN: AlignRepr = $crate::macros::first![$(AlignRepr::$variant),+];
            const MAX: AlignRepr = $crate::macros::last![$(AlignRepr::$variant),+];
        }

        /// This is the actual representation of masks.
        ///
        /// The variant names have no say in the actual value.
        #[derive(Clone, Copy, PartialEq, Eq)]
        #[repr(usize)]
        enum MaskRepr {
            $($variant = !(AlignRepr::$variant as usize).strict_sub(1),)+
        }
    };
}

// //
// #[unsafe(no_mangle)]
// pub unsafe fn mask(alignment: NonZero<usize>) -> NonZero<usize> {
//     unsafe { core::hint::assert_unchecked(alignment.is_power_of_two()) };

//     let mask = NonZero::new(!(alignment.get() - 1)).unwrap();

//     assert!(mask >= alignment);

//     mask
// }

// NOTE: This is how we generate what goes into the macro invocation below.
//
//       This shouldn't really ever need to be touched.
//
//       If you ant to run this, execute `cargo test -p gulf-core --features=__generate __generate_align -- --nocapture`.
#[cfg(all(test, feature = "__generate"))]
#[test]
fn __generate_align() {
    use std::{cell::Cell, io::Write as _, iter, range::Range};

    let mut stdout = std::io::stdout().lock();

    let ranges = iter::from_fn({
        let mut state = Cell::<Range<u32>>::new((0_u32..16).into());

        move || {
            Some(state.get().end)
                .and_then(|start| start.checked_mul(2).map(|end| (start, end)))
                .and_then(|(start, end)| u8::try_from(end).ok().map(|end| (start as u8, end)))
                .map(|(start, end)| state.replace((start as u32..end as u32).into()))
                .map(|Range { start, end }| (start as u8, end as u8))
        }
    });

    for (start, end) in ranges {
        if start != 0 {
            write!(&mut stdout, "\n\n").expect("failed to write to stdout");
        }

        write!(
            &mut stdout,
            concat!(
                "    // For {end}-bit platforms and above.\n",
                "    \"{end}\" => [\n",
            ),
            end = end,
        )
        .expect("failed to write to stdout");

        assert!(
            end.strict_sub(start) % 16 == 0,
            "ranges of values are expected to be multiples of 16"
        );

        for x in start..end {
            let _ = u8::from(x);

            let (spacing, delim) = match (x - start) % 16 {
                0 => ("        ", ", "),
                1..15 => ("", ", "),
                15 => ("", ",\n"),
                _ => unreachable!(),
            };

            write!(&mut stdout, "{spacing}_{x:02X}{delim}").expect("failed to write to stdout");
        }

        write!(&mut stdout, "    ],\n").expect("failed to write to stdout");
    }

    stdout.flush().expect("failed to flush stdout");
    // let mut ranges =
}

repr! {
    // For 16-bit platforms and above.
    "16" => [
        _00, _01, _02, _03, _04, _05, _06, _07, _08, _09, _0A, _0B, _0C, _0D, _0E, _0F,
    ],


    // For 32-bit platforms and above.
    "32" => [
        _10, _11, _12, _13, _14, _15, _16, _17, _18, _19, _1A, _1B, _1C, _1D, _1E, _1F,
    ],


    // For 64-bit platforms and above.
    "64" => [
        _20, _21, _22, _23, _24, _25, _26, _27, _28, _29, _2A, _2B, _2C, _2D, _2E, _2F,
        _30, _31, _32, _33, _34, _35, _36, _37, _38, _39, _3A, _3B, _3C, _3D, _3E, _3F,
    ],
}
