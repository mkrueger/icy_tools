//! Moebius-compatible document compression.
//!
//! Moebius transfers documents as JSON using `libtextmode.compress(doc)`.
//! That format contains `compressed_data` with three RLE streams: `code`, `fg`, `bg`.
//! Each stream is an array of `[value, repeat]` pairs, where `repeat` is the number
//! of *additional* repeats (so the run length is `repeat + 1`).
//!
//! This module implements compatible (de)compression helpers.

use super::Block;
use serde::{Deserialize, Serialize};

/// Moebius `compressed_data` payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoebiusCompressedData {
    pub code: Vec<[u32; 2]>,
    pub fg: Vec<[u32; 2]>,
    pub bg: Vec<[u32; 2]>,
}

/// Decompress a Moebius `compressed_data` payload into a row-major flat block array.
///
/// Row-major means index is `y * columns + x` (same as Moebius).
pub fn uncompress_moebius_data(
    columns: u32,
    rows: u32,
    compressed: &MoebiusCompressedData,
) -> Result<Vec<Block>, CompressionError> {
    let expected_len = (columns as usize)
        .checked_mul(rows as usize)
        .ok_or_else(|| CompressionError::InvalidData("columns*rows overflow".to_string()))?;

    let codes = expand_rle_stream(&compressed.code)?;
    let fgs = expand_rle_stream(&compressed.fg)?;
    let bgs = expand_rle_stream(&compressed.bg)?;

    if codes.len() != fgs.len() || codes.len() != bgs.len() {
        return Err(CompressionError::InvalidData(
            "compressed_data streams have different lengths".to_string(),
        ));
    }

    if codes.len() != expected_len {
        return Err(CompressionError::InvalidData(format!(
            "decompressed length mismatch: got {}, expected {}",
            codes.len(),
            expected_len
        )));
    }

    let mut blocks = Vec::with_capacity(expected_len);
    for i in 0..expected_len {
        blocks.push(Block {
            code: codes[i],
            fg: (fgs[i] & 0xFF) as u8,
            bg: (bgs[i] & 0xFF) as u8,
        });
    }

    Ok(blocks)
}

/// Compress a row-major flat block array into Moebius `compressed_data`.
pub fn compress_moebius_data(blocks: &[Block]) -> MoebiusCompressedData {
    let mut codes = Vec::with_capacity(blocks.len().saturating_div(2));
    let mut fgs = Vec::with_capacity(blocks.len().saturating_div(2));
    let mut bgs = Vec::with_capacity(blocks.len().saturating_div(2));

    compress_stream_u32(blocks.iter().map(|b| b.code), &mut codes);
    compress_stream_u32(blocks.iter().map(|b| b.fg as u32), &mut fgs);
    compress_stream_u32(blocks.iter().map(|b| b.bg as u32), &mut bgs);

    MoebiusCompressedData {
        code: codes,
        fg: fgs,
        bg: bgs,
    }
}

/// Convert row-major flat blocks into the column-major 2D layout used internally.
pub fn flat_to_columns(blocks: &[Block], columns: u32, rows: u32) -> Vec<Vec<Block>> {
    let mut result = Vec::with_capacity(columns as usize);
    for col in 0..columns as usize {
        let mut column = Vec::with_capacity(rows as usize);
        for row in 0..rows as usize {
            let idx = row * columns as usize + col;
            column.push(blocks.get(idx).cloned().unwrap_or_default());
        }
        result.push(column);
    }
    result
}

/// Error type for compression operations.
#[derive(Debug, Clone)]
pub enum CompressionError {
    /// Corrupt or inconsistent data
    InvalidData(String),
}

impl std::fmt::Display for CompressionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CompressionError::InvalidData(msg) => write!(f, "Invalid data: {}", msg),
        }
    }
}

impl std::error::Error for CompressionError {}

fn expand_rle_stream(pairs: &[[u32; 2]]) -> Result<Vec<u32>, CompressionError> {
    let mut out: Vec<u32> = Vec::new();
    for pair in pairs {
        let value = pair[0];
        let repeat = pair[1] as usize;
        let run_len = repeat
            .checked_add(1)
            .ok_or_else(|| CompressionError::InvalidData("repeat overflow".to_string()))?;

        out.reserve(run_len);
        for _ in 0..run_len {
            out.push(value);
        }
    }
    Ok(out)
}

fn compress_stream_u32<I: Iterator<Item = u32>>(mut iter: I, out: &mut Vec<[u32; 2]>) {
    let Some(mut current) = iter.next() else {
        return;
    };
    let mut repeat: u32 = 0;

    for value in iter {
        if value == current {
            repeat = repeat.saturating_add(1);
        } else {
            out.push([current, repeat]);
            current = value;
            repeat = 0;
        }
    }
    out.push([current, repeat]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_moebius_compress_uncompress_single() {
        let blocks = vec![Block { code: 65, fg: 7, bg: 0 }];
        let compressed = compress_moebius_data(&blocks);
        let decompressed = uncompress_moebius_data(1, 1, &compressed).unwrap();
        assert_eq!(blocks.len(), decompressed.len());
        assert_eq!(blocks[0].code, decompressed[0].code);
        assert_eq!(blocks[0].fg, decompressed[0].fg);
        assert_eq!(blocks[0].bg, decompressed[0].bg);
    }

    #[test]
    fn test_moebius_compress_uncompress_run() {
        let block = Block { code: 32, fg: 7, bg: 0 };
        let blocks: Vec<Block> = std::iter::repeat(block.clone()).take(100).collect();
        let compressed = compress_moebius_data(&blocks);
        let decompressed = uncompress_moebius_data(100, 1, &compressed).unwrap();
        assert_eq!(blocks.len(), decompressed.len());
        for (orig, dec) in blocks.iter().zip(decompressed.iter()) {
            assert_eq!(orig.code, dec.code);
            assert_eq!(orig.fg, dec.fg);
            assert_eq!(orig.bg, dec.bg);
        }
    }

    #[test]
    fn test_moebius_compress_uncompress_mixed() {
        let blocks = vec![
            Block { code: 65, fg: 1, bg: 0 },
            Block { code: 66, fg: 2, bg: 0 },
            Block { code: 67, fg: 3, bg: 0 },
            Block { code: 32, fg: 7, bg: 0 },
            Block { code: 32, fg: 7, bg: 0 },
            Block { code: 32, fg: 7, bg: 0 },
            Block { code: 32, fg: 7, bg: 0 },
            Block { code: 32, fg: 7, bg: 0 },
            Block { code: 68, fg: 4, bg: 1 },
        ];
        let compressed = compress_moebius_data(&blocks);
        let decompressed = uncompress_moebius_data(blocks.len() as u32, 1, &compressed).unwrap();
        assert_eq!(blocks.len(), decompressed.len());
        for (i, (orig, dec)) in blocks.iter().zip(decompressed.iter()).enumerate() {
            assert_eq!(orig.code, dec.code, "Mismatch at index {}", i);
            assert_eq!(orig.fg, dec.fg, "FG mismatch at index {}", i);
            assert_eq!(orig.bg, dec.bg, "BG mismatch at index {}", i);
        }
    }

    #[test]
    fn test_empty() {
        let blocks: Vec<Block> = vec![];
        let compressed = compress_moebius_data(&blocks);
        assert!(compressed.code.is_empty());
        assert!(compressed.fg.is_empty());
        assert!(compressed.bg.is_empty());
    }
}
