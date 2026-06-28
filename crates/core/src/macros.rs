/// A macro that takes the first element from a list.
#[macro_export]
macro_rules! first {
    [$first:expr $(, $rest:expr)*  $(,)?] => {
        $first
    };
}

#[doc(inline)]
pub use crate::first;

/// A macro that takes the final element from a list.
#[macro_export]
macro_rules! last {
    [
        $a:expr, $b:expr, $c:expr, $d:expr,
        $e:expr, $f:expr, $g:expr, $h:expr,
        $i:expr, $j:expr, $k:expr, $l:expr,
        $m:expr, $n:expr, $o:expr, $p:expr
        $(, $rest:expr)* $(,)?
    ] => {
        $crate::macros::last![
            $p $(, $rest)*,
        ]
    };

    [
        $a:expr, $b:expr, $c:expr, $d:expr,
        $e:expr, $f:expr, $g:expr, $h:expr
        $(, $rest:expr)* $(,)?
    ] => {
        $crate::macros::last![
            $h $(, $rest)*,
        ]
    };

    [
        $a:expr, $b:expr, $c:expr, $d:expr
        $(, $rest:expr)* $(,)?
    ] => {
        $crate::macros::last![
            $d $(, $rest)*,
        ]
    };

    [
        $a:expr, $b:expr
        $(, $rest:expr)* $(,)?
    ] => {
        $crate::macros::last![
            $b $(, $rest)*,
        ]
    };

    [$final:expr $(,)?] => {
        $final
    };
}

#[doc(inline)]
pub use crate::last;
