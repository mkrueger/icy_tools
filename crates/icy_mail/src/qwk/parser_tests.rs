use super::tests::{control_dat, header};
use super::{packet_files, read_control_dat, PacketFileKind, QwkPackage};

#[test]
fn control_conference_counts_preserve_the_last_area_and_screens() {
    for count in [1, 2] {
        let data = String::from_utf8(control_dat())
            .unwrap()
            .replacen("4\r\n2\r\n", &format!("4\r\n{count}\r\n"), 1);
        let control = read_control_dat(data.as_bytes(), &[]).unwrap();
        assert_eq!(control.conferences.len(), 2);
        assert_eq!(control.conferences[1].number, 2);
        assert_eq!(control.conferences[1].name, "Retro");
        assert_eq!(control.welcome_screen, "HELLO");
        assert_eq!(control.news_screen, "NEWS");
        assert_eq!(control.logoff_screen, "GOODBYE");
    }

    let data = b"TEST\r\n\r\n\r\nSysop\r\n0,TEST\r\n01-01-2600:00\r\nUSER\r\n\r\n0\r\n0\r\n0\r\n0\r\nMain\r\nHELLO\r\nNEWS\r\nGOODBYE\r\n";
    let control = read_control_dat(data, &[]).unwrap();
    assert_eq!(control.conferences.len(), 1);
    assert_eq!(control.conferences[0].number, 0);
    assert_eq!(control.welcome_screen, "HELLO");
}

#[test]
fn numeric_welcome_screens_preserve_actual_count_conferences() {
    for name in ["123", "123.ans"] {
        for optional in ["", "0\r\n2\r\nUSER\r\nFirst\r\n"] {
            let data = format!("{}{optional}", String::from_utf8(control_dat()).unwrap().replace("HELLO\r\n", "123\r\n"));
            let files = vec![
                (name.to_owned(), b"Welcome".to_vec()),
                ("NEWS".to_owned(), b"News".to_vec()),
                ("GOODBYE".to_owned(), b"Goodbye".to_vec()),
            ];
            let control = read_control_dat(data.as_bytes(), &files).unwrap();
            assert_eq!(control.conferences.len(), 2);
            assert_eq!(control.conferences[1].name, "Retro");
            assert_eq!(control.welcome_screen, "123");
            assert_eq!(control.news_screen, "NEWS");
            assert_eq!(control.logoff_screen, "GOODBYE");
            let screens = packet_files(&control, files);
            assert_eq!(screens.len(), 3);
            assert_eq!(screens[0].name, name);
            assert_eq!(screens[0].kind, PacketFileKind::Welcome);
            assert_eq!(screens[1].name, "NEWS");
            assert_eq!(screens[2].name, "GOODBYE");
        }
    }
    let data = String::from_utf8(control_dat()).unwrap().replace("HELLO\r\n", "123\r\n");
    let control = read_control_dat(data.as_bytes(), &[]).unwrap();
    assert_eq!(control.conferences.len(), 2, "a missing numeric screen is not an extra conference");
    assert_eq!(control.welcome_screen, "123");
    assert_eq!(control.news_screen, "NEWS");
    assert_eq!(control.logoff_screen, "GOODBYE");
}

#[test]
fn invalid_qwk_block_counts_report_errors() {
    for count in [b"0     ", b"      ", b"broken", b"999999", b"2     "] {
        let mut messages = vec![b' '; 128];
        let mut invalid = header(b' ', 1, "10-12-2618:14", "ALL", "AUTHOR", "Invalid", 0, 1, 1);
        invalid[116..122].copy_from_slice(count);
        messages.extend(invalid);
        let error = QwkPackage::parse_headers(&messages).unwrap_err().to_string();
        assert!(error.contains("byte 128"), "{error}");
    }
}
