//! I/O implementations for various file formats.
//!
//! This module contains the actual loading and saving logic for all supported file formats.
//! The implementations are accessed through `FileFormat::from_bytes()` and `FileFormat::to_bytes()`.

mod ansi;
mod artworx;
mod ascii;
mod atascii;
mod avatar;
mod bin;
mod ctrla;
mod ice_draw;
mod icy_draw;
mod pcboard;
mod renegade;
pub(crate) mod seq;
mod tundra;
mod xbinary;

// Re-export load/save functions for use by FileFormat
pub(crate) use ansi::{load_ansi, save_ansi};
pub(crate) use artworx::{load_artworx, save_artworx};
pub(crate) use ascii::{load_ascii, save_ascii};
pub(crate) use atascii::{load_atascii, save_atascii};
pub(crate) use avatar::{load_avatar, save_avatar};
pub(crate) use bin::{load_bin, save_bin};
pub(crate) use ctrla::{load_ctrla, save_ctrla};
pub(crate) use ice_draw::{load_ice_draw, save_ice_draw};
pub(crate) use icy_draw::{load_icy_draw, save_icy_draw};
pub(crate) use pcboard::{load_pcboard, save_pcboard};
pub(crate) use renegade::{load_renegade, save_renegade};
pub(crate) use seq::{load_seq, save_seq};
pub(crate) use tundra::{load_tundra, save_tundra};
pub(crate) use xbinary::{load_xbin, save_xbin};
