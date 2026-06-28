#![doc = crate::_docs::crate_level!()]
#![cfg_attr(not(test), no_std)]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "std")]
extern crate std;

// Video utilities.
pub mod video;

// Audio utilities.
pub mod audio;

// Compiler hints.
pub mod hint;

// Numerical utilities.
pub mod num;

// Pointer related utilities.
pub mod ptr;

// Memory related utilities.
pub mod mem;

// `Arc` stuff.
#[cfg(all(feature = "alloc", target_has_atomic = "ptr"))]
pub mod arc;

// `Rc` stuff.
#[cfg(feature = "alloc")]
pub mod rc;

// Async stuff.
#[cfg(all(feature = "tokio", feature = "futures"))]
pub mod task;

// Macros.
pub mod macros;

mod _docs {
    cfg_select! {
        feature = "alloc" => {
            macro_rules! _alloc {
                () => { "[`alloc`](::alloc)" };
            }
        }
        _ => {
            macro_rules! _alloc {
                () => { "[`alloc`](https://doc.rust-lang.org/alloc/)" };
            }
        }
    }

    pub(crate) use _alloc;

    cfg_select! {
        feature = "std" => {
            macro_rules! _std {
                () => { "[`std`](::std)" };
            }
        }

        _ => {
            macro_rules! _std {
                () => { "[`std`](https://doc.rust-lang.org/std/)" };
            }
        }
    }

    pub(crate) use _std;

    macro_rules! crate_level {
        () => {
            ::core::concat!(
                "A crate for utilities that *should be within* [`core`](::core), ",
                crate::_docs::_alloc!(),
                ", ",
                crate::_docs::_std!(),
                " or just other general things that should be fairly universal for this ecosystem of crates.",
            )
        };
    }

    pub(crate) use crate_level;
}

/// Abort the program.
#[inline(always)]
#[track_caller]
pub fn abort() -> ! {
    cfg_select! {
        feature = "std" => { ::std::process::abort() },
        _ => {
            struct Abort;

            impl Drop for Abort {
                #[inline(always)]
                fn drop(&mut self) {
                    panic!()
                }
            }

            let _abort = Abort;

            panic!()
        },
    }
}
