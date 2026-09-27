pub mod com_thread;
pub mod connect;
pub mod emulated_modem;
pub mod terminal_thread;

pub const BAUD_RATES: [u32; 11] = [300, 1200, 2400, 4800, 9600, 14400, 19200, 28800, 38400, 57600, 115_200];

pub use terminal_thread::{ConnectionConfig, TerminalCommand, TerminalEvent, TerminalThread};
