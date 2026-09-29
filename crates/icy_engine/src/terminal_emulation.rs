//! `TerminalEmulation`: which parser a screen is created with.
//!
//! With the `net` feature (on by default) this is icy_net's type, as before,
//! so it keeps working with icy_net's telnet code. Without it, icy_engine
//! defines the same enum itself, so the engine builds without icy_net's
//! networking stack (tokio, rustls, russh), for example for
//! `wasm32-unknown-unknown`. Keep the variants in step with icy_net's.

#[cfg(feature = "net")]
pub use icy_net::telnet::TerminalEmulation;

#[cfg(not(feature = "net"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum TerminalEmulation {
    #[default]
    Ansi,
    Utf8Ansi,
    Avatar,
    Ascii,
    PETscii,
    ATAscii,
    ViewData,
    Mode7,
    Rip,
    Skypix,
    AtariST,
}
