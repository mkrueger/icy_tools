//! Standalone synthetic Blue Wave fixture: no codec/crate dependencies.
//! Literal offsets follow the published November 1995 packet structures,
//! independently checked against ENiGMA's producer.
//!
//! TEST: Alice/Ally; areas 12 LOCAL and 24 SECOND; one private Bob -> Alice
//! message 45 replying to 42, subject Hey, CP437 body `Hello\r\n\x82!`.

pub fn fixture_files(level: u8, uses_upl: bool) -> Vec<(String, Vec<u8>)> {
    let (mut inf, mix, fti, dat) = incoming(level, false);
    inf[984] = u8::from(uses_upl);
    vec![
        ("TEST.INF".into(), inf),
        ("TEST.MIX".into(), mix),
        ("TEST.FTI".into(), fti),
        ("TEST.DAT".into(), dat),
    ]
}

pub(crate) fn incoming(level: u8, extended: bool) -> (Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>) {
    let (head, area, mix_stride, fti_stride) = if extended { (1240, 88, 18, 194) } else { (1230, 80, 14, 186) };
    let mut inf = vec![0; head + area * 2];
    inf[0] = level;
    inf[76..81].copy_from_slice(b"Alice");
    inf[119..123].copy_from_slice(b"Ally");
    inf[235..242].copy_from_slice(b"My BBS!");
    inf[987..991].copy_from_slice(b"TEST");
    inf[984] = 1;
    if extended {
        inf[976..978].copy_from_slice(&(head as u16).to_le_bytes());
        inf[978..980].copy_from_slice(&(area as u16).to_le_bytes());
        inf[980..982].copy_from_slice(&(mix_stride as u16).to_le_bytes());
        inf[982..984].copy_from_slice(&(fti_stride as u16).to_le_bytes());
    }
    for (i, (number, tag)) in [(b"12".as_slice(), b"LOCAL".as_slice()), (b"24", b"SECOND")].iter().enumerate() {
        let p = head + area * i;
        inf[p..p + number.len()].copy_from_slice(number);
        inf[p + 6..p + 6 + tag.len()].copy_from_slice(tag);
        inf[p + 27..p + 32].copy_from_slice(b"Title");
        inf[p + 77..p + 79].copy_from_slice(&0x21u16.to_le_bytes());
    }
    let mut mix = vec![0; mix_stride * 2];
    mix[..2].copy_from_slice(b"12");
    mix[6..8].copy_from_slice(&1u16.to_le_bytes());
    mix[8..10].copy_from_slice(&1u16.to_le_bytes());
    mix[mix_stride..mix_stride + 2].copy_from_slice(b"24");
    mix[mix_stride + 10..mix_stride + 14].copy_from_slice(&(fti_stride as u32).to_le_bytes());
    let mut fti = vec![0; fti_stride];
    fti[..3].copy_from_slice(b"Bob");
    fti[36..41].copy_from_slice(b"Alice");
    fti[72..75].copy_from_slice(b"Hey");
    fti[144..163].copy_from_slice(b"01 Jan 24  12:34:56");
    fti[164..166].copy_from_slice(&45u16.to_le_bytes());
    fti[166..168].copy_from_slice(&42u16.to_le_bytes());
    let dat = b" Hello\r\n\x82!".to_vec();
    fti[174..178].copy_from_slice(&(dat.len() as u32).to_le_bytes());
    fti[178..180].copy_from_slice(&1u16.to_le_bytes());
    (inf, mix, fti, dat)
}
