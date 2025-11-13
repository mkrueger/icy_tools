use icy_engine::{get_crc16, get_crc32};

#[test]
fn test_crc32() {
    let crc = get_crc32(&[4, 0, 0, 5, 3]);
    assert_eq!(0xD7DC_F422, crc);

    let mut data = Vec::new();
    for i in 0..1024 * 16 {
        data.push(i as u8);
    }
    let crc = get_crc32(&data);
    assert_eq!(0xE817_22F0, crc);
}

#[test]
fn test_crc16() {
    let crc = get_crc16(&[4, 0, 0, 5, 3]);
    assert_eq!(0x4690, crc);

    let mut data = Vec::new();
    for i in 0..1024 * 16 {
        data.push(i as u8);
    }
    let crc = get_crc16(&data);
    assert_eq!(0xF617, crc);
}
