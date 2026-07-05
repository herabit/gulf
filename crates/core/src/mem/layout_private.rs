///! Private implementation details for the layout types.
use core::cmp::Ordering;

/// Returns whether or not the given value has any holes (discontinuous sequences of bits).
#[inline(always)]
#[must_use]
pub(crate) const fn has_hole(value: usize) -> bool {
    value.leading_ones().strict_add(value.trailing_zeros()) != usize::BITS
}

/// Perform a three-way-comparison on two [`usize`]s.
#[inline(always)]
#[must_use]
pub(crate) const fn compare(
    lhs: usize,
    rhs: usize,
) -> Ordering {
    match (lhs > rhs, lhs < rhs) {
        (true, false) => Ordering::Greater,
        (false, false) => Ordering::Equal,
        (false, true) => Ordering::Less,
        (true, true) => unreachable!(),
    }
}

/// Get the minimum of two [`usize`]s.
#[inline(always)]
#[must_use]
pub(crate) const fn min(
    a: usize,
    b: usize,
) -> usize {
    match compare(b, a) {
        Ordering::Less => b,
        Ordering::Greater | Ordering::Equal => a,
    }
}

/// Get the maximum of two [`usize`]s.
#[inline(always)]
#[must_use]
pub(crate) const fn max(
    a: usize,
    b: usize,
) -> usize {
    match compare(b, a) {
        Ordering::Less => a,
        Ordering::Greater | Ordering::Equal => b,
    }
}

/// Clamp a [`usize`] with a `min` and `max` value.
#[inline(always)]
#[must_use]
pub(crate) const fn clamp(
    value: usize,
    min: usize,
    max: usize,
) -> Option<usize> {
    if min <= max {
        if value < min {
            Some(min)
        } else if value > max {
            Some(max)
        } else {
            Some(value)
        }
    } else {
        None
    }
}

/// This is the actual meat of how we generate things automagically.
macro_rules! generate {
    {
        $D:tt;

        bits: $bits:tt,
        bits_minus_one: $bits_minus_one:tt,

        variants: enum {
            $($variant:ident),+
            $(,)?
        },

        align: {
            bounds: $align_min:tt..=$align_max:tt
            $(,)?
        },

        mask: {
            bounds: $mask_min:tt..=$mask_max:tt,
            without_hole: $without_hole:tt,
            with_hole: $with_hole:tt
            $(,)?
        }

        $(,)?
    } => {
        /// This is an "index" into the possible alignments and masks.
        #[repr(u32)]
        pub(crate) enum Index {
            $($variant),+
        }

        /// This is the actual representation of alignments.
        ///
        /// The variant name does not imply the actual value
        /// of the variant, merely the index.
        #[derive(
            ::core::clone::Clone,
            ::core::marker::Copy,
            ::core::cmp::PartialEq,
            ::core::cmp::Eq,
        )]
        #[repr(usize)]
        pub(crate) enum AlignRepr {
            $(
                $variant = 1_usize.strict_shl(Index::$variant as u32),
            )+
        }

        impl AlignRepr {
            pub(crate) const MIN: AlignRepr = $crate::macros::first![
                $( AlignRepr::$variant ),+
            ];
            pub(crate) const MAX: AlignRepr = $crate::macros::last![
                $( AlignRepr::$variant ),+
            ];
        }

        /// This is the actual representation of address masks.
        ///
        /// The variant name does not imply the actual value
        /// of the variant, merely the index.
        #[derive(
            ::core::clone::Clone,
            ::core::marker::Copy,
            ::core::cmp::PartialEq,
            ::core::cmp::Eq,
        )]
        #[repr(usize)]
        pub(crate) enum MaskRepr {
            $(
                $variant = !(AlignRepr::$variant as usize).strict_sub(1),
            )+
        }

        impl MaskRepr {
            pub(crate) const MIN: MaskRepr = $crate::macros::last![
                $( MaskRepr::$variant ),+
            ];
            pub(crate) const MAX: MaskRepr = $crate::macros::first![
                $( MaskRepr::$variant ),+
            ];
        }

        /// Macro for getting values and constants relating to the layout types.
        macro_rules! get {
            (bits) => { $bits };
            (@bits) => { ::core::stringify!($bits) };

            (bits - 1) => { $bits_minus_one };
            (@bits - 1) => { ::core::stringify!($bits_minus_one) };

            (align.min) => { $align_min };
            (@align.min) => { ::core::stringify!($align_min) };
            (align.min.assert) => {
                ::core::concat!(
                    "`Align`'s documented minimum value, `",
                    ::core::stringify!($align_min),
                    "`, is wrong",
                )
            };

            (align.max) => { $align_max };
            (@align.max) => { ::core::stringify!($align_max) };
            (align.max.assert) => {
                ::core::concat!(
                    "`Align`'s documented maximum value, `",
                    ::core::stringify!($align_max),
                    "`, is wrong",
                )
            };

            (mask.min) => { $mask_min };
            (@mask.min) => { ::core::stringify!($mask_min) };
            (mask.min.assert) => {
                ::core::concat!(
                    "`Mask`'s documented minimum value, `",
                    ::core::stringify!($mask_min),
                    "`, is wrong",
                )
            };

            (mask.max) => { $mask_max };
            (@mask.max) => { ::core::stringify!($mask_max) };
            (mask.max.assert) => {
                ::core::concat!(
                    "`Mask`'s documented maximum value, `",
                    ::core::stringify!($mask_max),
                    "`, is wrong",
                )
            };

            (mask.with_hole) => { $with_hole };
            (@mask.with_hole) => { ::core::stringify!($with_hole) };
            (mask.with_hole.assert) => {
                ::core::concat!(
                    "`Mask`'s documented example of a mask with a hole, `",
                    ::core::stringify!($with_hole),
                    "`, has no hole",
                )
            };

            (mask.without_hole) => { $without_hole };
            (@mask.without_hole) => { ::core::stringify!($without_hole) };
            (mask.without_hole.assert) => {
                ::core::concat!(
                    "`Mask`'s documented example of a mask without a hole, `",
                    ::core::stringify!($without_hole),
                    "`, has a hole",
                )
            };

            // This just provides an alternate syntax for getting bounds.
            ($D this:ident . bounds . $D bound:ident) => {
                $D crate::mem::layout::private::get!(
                    $D this . $D bound
                )
            };
            // This just provides an alternate syntax for getting bound strings.
            (@ $D this:ident . bounds . $D bound:ident) => {
                $D crate::mem::layout::private::get!(
                    @ $D this . $D bound
                )
            };
        }

        pub(crate) use get;
    };
}

pub(crate) use generate;

macro_rules! define {
    {
        $D:tt;

        "16" => {
            variants: enum {
                $($_16:ident),+
                $(,)?
            },

            $($_16_rest:tt)*
        } $(,)?

        "32" => {
            variants: enum {
                $($_32:ident),+
                $(,)?
            },

            $($_32_rest:tt)*
        } $(,)?

        "64" => {
            variants: enum {
                $($_64:ident),+
                $(,)?
            },

            $($_64_rest:tt)*
        } $(,)?
    } => {
        ::core::cfg_select! {
            target_pointer_width = "16" => {
                $crate::mem::layout::private::generate! {
                    $D;

                    bits: 16,
                    bits_minus_one: 15,

                    variants: enum {
                        $( $_16, )+
                    },

                    $($_16_rest)*
                }
            }
            target_pointer_width = "32" => {
                $crate::mem::layout::private::generate! {
                    $D;

                    bits: 32,
                    bits_minus_one: 31,

                    variants: enum {
                        $( $_16, )+
                        $( $_32, )+
                    },

                    $($_32_rest)*
                }
            }
            target_pointer_width = "64" => {
                $crate::mem::layout::private::generate! {
                    $D;

                    bits: 64,
                    bits_minus_one: 63,

                    variants: enum {
                        $( $_16, )+
                        $( $_32, )+
                        $( $_64, )+
                    },

                    $($_64_rest)*
                }
            }
            _ => {
                ::core::compiler_error!("unsupported target");
            }
        }
    };
}

define! {
    $;

    "16" => {
        // For 16-bit platforms and above.
        variants: enum {
            _00, _01, _02, _03, _04, _05, _06, _07, _08, _09, _0A, _0B, _0C, _0D, _0E, _0F,
        },

        align: {
            bounds: 0x0001..=0x8000,
        },

        mask: {
            bounds: 0x8000..=0xFFFF,

            without_hole: 0xFFF0,
            with_hole:    0xF0F0,
        },
    },

    "32" => {
        // For 32-bit platforms and above.
        variants: enum {
            _10, _11, _12, _13, _14, _15, _16, _17, _18, _19, _1A, _1B, _1C, _1D, _1E, _1F,
        },

        align: {
            bounds: 0x0000_0001..=0x8000_0000,
        },

        mask: {
            bounds: 0x8000_0000..=0xFFFF_FFFF,

            without_hole: 0xFFFF_0000,
            with_hole:    0xFF0F_0000,
        },
    },

    "64" => {
        // For 64-bit platforms and above.
        variants: enum {
            _20, _21, _22, _23, _24, _25, _26, _27, _28, _29, _2A, _2B, _2C, _2D, _2E, _2F,
            _30, _31, _32, _33, _34, _35, _36, _37, _38, _39, _3A, _3B, _3C, _3D, _3E, _3F,
        },

        align: {
            bounds: 0x0000_0000_0000_0001..=0x8000_0000_0000_0000,
        },

        mask: {
            bounds: 0x8000_0000_0000_0000..=0xFFFF_FFFF_FFFF_FFFF,

            without_hole: 0xFFFF_FFFF_0000_0000,
            with_hole:    0xFFFF_FF0F_0000_0000,
        }
    },
}
