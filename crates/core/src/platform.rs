//! Platform specific functionality.

cfg_select! {
    target_os = "linux" => {
        pub mod linux;
    }
    _ => {},
}
