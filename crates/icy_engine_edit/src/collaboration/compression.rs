//! Moebius-compatible RLE compression for document transfer.
//!
//! The compression format encodes character blocks (code, fg, bg) using
//! run-length encoding to reduce the size of document transfers.
//!
//! # Format
//!
//! Each block is encoded as 3 bytes: [code, fg, bg]
//! Runs of identical blocks are encoded with a count prefix.
//!
//! The compressed data is then base64 encoded for JSON transport.

use super::Block;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use std::io::Read;

/// Marker byte indicating a run-length encoded sequence.
const RLE_MARKER: u8 = 0xFF;

/// Maximum run length that can be encoded.
const MAX_RUN_LENGTH: usize = 255;

/// Encode a sequence of blocks using RLE compression.
///
/// Returns base64-encoded compressed data.
pub fn compress_blocks(blocks: &[Block]) -> String {
    let compressed = rle_encode(blocks);
    base64_encode(&compressed)
}

/// Decode RLE-compressed blocks from base64-encoded data.
///
/// Returns the decompressed blocks.
pub fn decompress_blocks(data: &str) -> Result<Vec<Block>, CompressionError> {
    let bytes = base64_decode(data)?;
    rle_decode(&bytes)
}

/// Compress a 2D document (column-major order as used by Moebius).
pub fn compress_document(blocks: &[Vec<Block>], columns: u32, rows: u32) -> String {
    // Flatten to column-major order (Moebius format)
    let mut flat = Vec::with_capacity((columns * rows) as usize);
    for row in 0..rows as usize {
        for col in 0..columns as usize {
            if col < blocks.len() && row < blocks[col].len() {
                flat.push(blocks[col][row].clone());
            } else {
                flat.push(Block::default());
            }
        }
    }
    compress_blocks(&flat)
}

/// Decompress a document into 2D structure.
pub fn decompress_document(data: &str, columns: u32, rows: u32) -> Result<Vec<Vec<Block>>, CompressionError> {
    let flat = decompress_blocks(data)?;

    // Convert from row-major flat to column-major 2D
    let mut result = Vec::with_capacity(columns as usize);
    for col in 0..columns as usize {
        let mut column = Vec::with_capacity(rows as usize);
        for row in 0..rows as usize {
            let idx = row * columns as usize + col;
            if idx < flat.len() {
                column.push(flat[idx].clone());
            } else {
                column.push(Block::default());
            }
        }
        result.push(column);
    }
    Ok(result)
}

/// Error type for compression operations.
#[derive(Debug, Clone)]
pub enum CompressionError {
    /// Invalid base64 encoding
    Base64Error(String),
    /// Corrupted RLE data
    InvalidRleData(String),
    /// Unexpected end of data
    UnexpectedEof,
}

impl std::fmt::Display for CompressionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CompressionError::Base64Error(msg) => write!(f, "Base64 error: {}", msg),
            CompressionError::InvalidRleData(msg) => write!(f, "Invalid RLE data: {}", msg),
            CompressionError::UnexpectedEof => write!(f, "Unexpected end of compressed data"),
        }
    }
}

impl std::error::Error for CompressionError {}

/// RLE encode blocks to bytes.
fn rle_encode(blocks: &[Block]) -> Vec<u8> {
    let mut result = Vec::new();
    let mut i = 0;

    while i < blocks.len() {
        let current = &blocks[i];
        let mut run_length = 1;

        // Count consecutive identical blocks
        while i + run_length < blocks.len()
            && run_length < MAX_RUN_LENGTH
            && blocks[i + run_length].code == current.code
            && blocks[i + run_length].fg == current.fg
            && blocks[i + run_length].bg == current.bg
        {
            run_length += 1;
        }

        // Encode the block
        let code_byte = (current.code & 0xFF) as u8;

        if run_length > 3 || code_byte == RLE_MARKER {
            // Use RLE encoding: [MARKER, count, code, fg, bg]
            result.push(RLE_MARKER);
            result.push(run_length as u8);
            result.push(code_byte);
            result.push(current.fg);
            result.push(current.bg);
        } else {
            // Write blocks individually: [code, fg, bg] for each
            for _ in 0..run_length {
                result.push(code_byte);
                result.push(current.fg);
                result.push(current.bg);
            }
        }

        i += run_length;
    }

    result
}

/// RLE decode bytes to blocks.
fn rle_decode(data: &[u8]) -> Result<Vec<Block>, CompressionError> {
    let mut result = Vec::new();
    let mut reader = std::io::Cursor::new(data);

    loop {
        let mut byte = [0u8; 1];
        match reader.read_exact(&mut byte) {
            Ok(()) => {}
            Err(ref e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(e) => return Err(CompressionError::InvalidRleData(e.to_string())),
        }

        if byte[0] == RLE_MARKER {
            // RLE encoded sequence
            let mut header = [0u8; 4];
            reader.read_exact(&mut header).map_err(|_| CompressionError::UnexpectedEof)?;

            let count = header[0] as usize;
            let block = Block {
                code: header[1] as u32,
                fg: header[2],
                bg: header[3],
            };

            for _ in 0..count {
                result.push(block.clone());
            }
        } else {
            // Single block
            let mut rest = [0u8; 2];
            reader.read_exact(&mut rest).map_err(|_| CompressionError::UnexpectedEof)?;

            result.push(Block {
                code: byte[0] as u32,
                fg: rest[0],
                bg: rest[1],
            });
        }
    }

    Ok(result)
}

/// Base64 encode bytes using the standard alphabet.
fn base64_encode(data: &[u8]) -> String {
    BASE64.encode(data)
}

/// Base64 decode string to bytes.
fn base64_decode(data: &str) -> Result<Vec<u8>, CompressionError> {
    BASE64.decode(data).map_err(|e| CompressionError::Base64Error(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compress_decompress_single() {
        let blocks = vec![Block { code: 65, fg: 7, bg: 0 }];
        let compressed = compress_blocks(&blocks);
        let decompressed = decompress_blocks(&compressed).unwrap();
        assert_eq!(blocks.len(), decompressed.len());
        assert_eq!(blocks[0].code, decompressed[0].code);
        assert_eq!(blocks[0].fg, decompressed[0].fg);
        assert_eq!(blocks[0].bg, decompressed[0].bg);
    }

    #[test]
    fn test_compress_decompress_run() {
        let block = Block { code: 32, fg: 7, bg: 0 };
        let blocks: Vec<Block> = std::iter::repeat(block.clone()).take(100).collect();
        let compressed = compress_blocks(&blocks);
        let decompressed = decompress_blocks(&compressed).unwrap();
        assert_eq!(blocks.len(), decompressed.len());
        for (orig, dec) in blocks.iter().zip(decompressed.iter()) {
            assert_eq!(orig.code, dec.code);
            assert_eq!(orig.fg, dec.fg);
            assert_eq!(orig.bg, dec.bg);
        }
    }

    #[test]
    fn test_compress_decompress_mixed() {
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
        let compressed = compress_blocks(&blocks);
        let decompressed = decompress_blocks(&compressed).unwrap();
        assert_eq!(blocks.len(), decompressed.len());
        for (i, (orig, dec)) in blocks.iter().zip(decompressed.iter()).enumerate() {
            assert_eq!(orig.code, dec.code, "Mismatch at index {}", i);
            assert_eq!(orig.fg, dec.fg, "FG mismatch at index {}", i);
            assert_eq!(orig.bg, dec.bg, "BG mismatch at index {}", i);
        }
    }

    #[test]
    fn test_base64_roundtrip() {
        let data = b"Hello, World! This is a test.";
        let encoded = base64_encode(data);
        let decoded = base64_decode(&encoded).unwrap();
        assert_eq!(data.as_slice(), decoded.as_slice());
    }

    #[test]
    fn test_empty() {
        let blocks: Vec<Block> = vec![];
        let compressed = compress_blocks(&blocks);
        let decompressed = decompress_blocks(&compressed).unwrap();
        assert!(decompressed.is_empty());
    }
}
