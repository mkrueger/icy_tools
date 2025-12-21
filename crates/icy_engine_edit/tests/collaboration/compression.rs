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
