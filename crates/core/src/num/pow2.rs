use core::{cmp, convert, error, fmt, hash, mem};

/// A struct representing a power-of-two integer.
///
/// All powers-of-two can be represented within a [`prim@u128`].
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct PowerOfTwo {
    /// The internal representation. This is a [`prim@u8`] enum in the
    /// range `0x00..=0x7F` (or `0x00..0x80` for my half-open lovers).
    ///
    /// This representation is ***guaranteed***.
    exponent: Exponent,
}

impl PowerOfTwo {
    /// The smallest power-of-two supported (`2^0`, or `1`).
    pub const MIN: PowerOfTwo = PowerOfTwo {
        exponent: Exponent::MIN,
    };
    /// The largest power-of-two supported (`2^127`, or `170,141,183,460,469,231,731,687,303,715,884,105,728`).
    pub const MAX: PowerOfTwo = PowerOfTwo {
        exponent: Exponent::MAX,
    };

    /// Create a [`PowerOfTwo`] given some exponent.
    ///
    /// # Returns
    ///
    /// Returns [`None`] if `exponent` is not a valid power-of-two that can be used within Rust.
    #[inline(always)]
    #[must_use]
    pub const fn from_exponent(exponent: u8) -> Option<PowerOfTwo> {
        const MIN: u8 = PowerOfTwo::MIN.exponent as u8;
        const MAX: u8 = PowerOfTwo::MAX.exponent as u8;

        match exponent {
            // SAFETY: `PowerOfTwo` is guaranteed to be represented as a `u8` in the range `MIN..=MAX`.
            exponent @ MIN..=MAX => Some(unsafe { mem::transmute(exponent) }),
            _ => None,
        }
    }

    /// Create a [`PowerOfTwo`] given some exponent without any checks.
    ///
    /// # Safety
    ///
    /// The caller must ensure that `exponent` is a valid Rust power-of-two exponent (`exponent <= PowerOfTwo::MAX.to_exponent()`).
    #[inline(always)]
    #[must_use]
    #[track_caller]
    pub const unsafe fn from_exponent_unchecked(exponent: u8) -> PowerOfTwo {
        match PowerOfTwo::from_exponent(exponent) {
            Some(power_of_two) => power_of_two,
            None => unsafe { crate::unreachable_unchecked!("provided `exponent` is too large") },
        }
    }

    /// Get the exponent for this power of two.
    #[inline(always)]
    #[must_use]
    pub const fn to_exponent(self) -> u8 {
        // SAFETY: `PowerOfTwo` is guaranteed to be represented as a `u8` in the range `MIN..=MAX`.
        unsafe {
            crate::assert_unchecked!(
                PowerOfTwo::from_exponent(self.exponent as u8).is_some(),
                "exponent is outside its valid range"
            )
        };

        self.exponent as u8
    }

    /// Attempts to multiply two [`PowerOfTwo`] values.
    ///
    /// # Returns
    ///
    /// Returns [`None`] if the multiplication fails.
    #[inline(always)]
    #[must_use]
    pub const fn checked_mul(
        self,
        rhs: PowerOfTwo,
    ) -> Option<PowerOfTwo> {
        let exponent = self.to_exponent().strict_add(rhs.to_exponent());

        PowerOfTwo::from_exponent(exponent)
    }

    /// Attempts to divide this [`PowerOfTwo`] value by another.
    ///
    /// # Returns
    ///
    /// Returns [`None`] if the division fails.
    #[inline(always)]
    #[must_use]
    pub const fn checked_div(
        self,
        rhs: PowerOfTwo,
    ) -> Option<PowerOfTwo> {
        let Some(exponent) = self.to_exponent().checked_sub(rhs.to_exponent()) else {
            return None;
        };

        PowerOfTwo::from_exponent(exponent)
    }
}

impl Default for PowerOfTwo {
    #[inline(always)]
    fn default() -> Self {
        Self::MIN
    }
}

impl PartialOrd for PowerOfTwo {
    #[inline(always)]
    fn partial_cmp(
        &self,
        other: &Self,
    ) -> Option<cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PowerOfTwo {
    #[inline(always)]
    fn cmp(
        &self,
        other: &Self,
    ) -> cmp::Ordering {
        self.to_exponent().cmp(&other.to_exponent())
    }
}

impl hash::Hash for PowerOfTwo {
    #[inline(always)]
    fn hash<H: hash::Hasher>(
        &self,
        state: &mut H,
    ) {
        self.to_exponent().hash(state);
    }

    #[inline(always)]
    fn hash_slice<H: hash::Hasher>(
        data: &[Self],
        state: &mut H,
    ) where
        Self: Sized,
    {
        // SAFETY: `PowerOfTwo` instances are all just `u8`s, so reinterpreting them as such is safe.
        let data = unsafe { (&raw const *data as *const [u8]).as_ref_unchecked() };

        u8::hash_slice(data, state)
    }
}

/// Error type indicating that something was not a power-of-two.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct NotPowerOfTwo;

impl fmt::Display for NotPowerOfTwo {
    #[inline]
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        f.write_str("provided value was not a power of two")
    }
}

impl error::Error for NotPowerOfTwo {
    #[allow(deprecated)]
    #[inline]
    fn description(&self) -> &str {
        "provided value was not a power of two"
    }
}

impl From<convert::Infallible> for NotPowerOfTwo {
    #[inline(always)]
    fn from(value: convert::Infallible) -> Self {
        match value {}
    }
}

macro_rules! repr {
    (
        $($variant:ident),+ $(,)?
    ) => {
        /// The internal representation of a [`PowerOfTwo`].
        #[derive(Clone, Copy, PartialEq, Eq)]
        #[repr(u8)]
        enum Exponent {
            $($variant,)+
        }

        impl Exponent {
            const MIN: Exponent = $crate::macros::first![$(Exponent::$variant),+];
            const MAX: Exponent = $crate::macros::last![$(Exponent::$variant),+];
        }

        // NOTE: We add this to ensure that the make sure that the `MIN` value is what we expect.
        const _: () = assert!((Exponent::MIN as u32) == 0, "unexpected power-of-two minimum value");
        // NOTE: We add this to ensure that we have all possible variants.
        const _: () = assert!((Exponent::MAX as u32).strict_add(1) == u128::BITS, "unexpected power-of-two maximum value");
    };
}

// NOTE: This is how we generate what goes into the macro invocation below.
//
//       This shouldn't really ever need to be touched.
//
//       If you wanna run this, execute `cargo test -p gulf-core --features=__generate __generate_pow2 -- --nocapture`.
#[cfg(all(test, feature = "__generate"))]
#[test]
fn __generate_pow2() {
    use std::io::Write as _;
    let mut stdout = std::io::stdout().lock();

    for x in 0x00..0x80 {
        assert!(u8::try_from(x).is_ok());

        let delim = match x % 16 {
            0..15 => ", ",
            15 => ",\n",
            _ => unreachable!(),
        };

        write!(&mut stdout, "_{x:02X}{delim}").expect("failed to write to stdout");
    }

    stdout.flush().expect("failed to flush stdout");
}

// NOTICE: DO NOT CHANGE THIS.
repr! {
    _00, _01, _02, _03, _04, _05, _06, _07, _08, _09, _0A, _0B, _0C, _0D, _0E, _0F,
    _10, _11, _12, _13, _14, _15, _16, _17, _18, _19, _1A, _1B, _1C, _1D, _1E, _1F,
    _20, _21, _22, _23, _24, _25, _26, _27, _28, _29, _2A, _2B, _2C, _2D, _2E, _2F,
    _30, _31, _32, _33, _34, _35, _36, _37, _38, _39, _3A, _3B, _3C, _3D, _3E, _3F,
    _40, _41, _42, _43, _44, _45, _46, _47, _48, _49, _4A, _4B, _4C, _4D, _4E, _4F,
    _50, _51, _52, _53, _54, _55, _56, _57, _58, _59, _5A, _5B, _5C, _5D, _5E, _5F,
    _60, _61, _62, _63, _64, _65, _66, _67, _68, _69, _6A, _6B, _6C, _6D, _6E, _6F,
    _70, _71, _72, _73, _74, _75, _76, _77, _78, _79, _7A, _7B, _7C, _7D, _7E, _7F,
}
