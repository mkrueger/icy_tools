#[cfg(test)]
mod zmodem_test {
    use std::sync::{Arc, Mutex};

    use crate::{
        protocol::{
            str_from_null_terminated_utf8_unchecked, zmodem::rz::read_subpacket, FileDescriptor, Header, HeaderType, Protocol, TestStorageHandler,
            TransferState, ZFrameType, Zmodem,
        },
        ui::connect::{DataConnection, TestConnection},
    };

    #[test]
    fn test_encode_subpckg_crc32() {
        let pck = Zmodem::encode_subpacket_crc32(crate::protocol::ZCRCE, b"a\n", false);
        assert_eq!(vec![0x61, 0x0a, 0x18, 0x68, 0xe5, 0x79, 0xd2, 0x0f], pck);
    }

    #[test]
    fn test_zmodem_simple_send() {
        let data = vec![1u8, 2, 5, 10];

        let mut test_connection = TestConnection::new(true);
        let transfer_state = Arc::new(Mutex::new(TransferState::default()));

        let mut send = Zmodem::new(512);

        send.initiate_send(
            &mut test_connection,
            vec![FileDescriptor::create_test("foo.bar".to_string(), data.clone())],
            &mut transfer_state.lock().unwrap(),
        )
        .expect("error.");
        let mut handler = TestStorageHandler::new();

        let mut can_count = 0;
        // sender          receiver
        // ZRQINIT(0)
        //                 ZRINIT
        // ZFILE
        //                 ZRPOS
        // ZDATA data…
        // ZEOF
        //                 ZRINIT
        // ZFIN
        //                 ZFIN
        // OO
        send.update(&mut test_connection, &transfer_state, &mut handler).expect("error.");
        test_connection.is_sender = false;
        let header: Header = Header::read(&mut test_connection, &mut can_count).unwrap().unwrap();
        assert_eq!(ZFrameType::RQInit, header.frame_type);
        Header::from_flags(ZFrameType::RIinit, 0, 0, 0, 0x23)
            .write(&mut test_connection, HeaderType::Hex, false)
            .unwrap();
        Header::from_number(ZFrameType::RPos, 0)
            .write(&mut test_connection, HeaderType::Hex, false)
            .unwrap();

        test_connection.is_sender = true;
        send.update(&mut test_connection, &transfer_state, &mut handler).expect("error.");
        test_connection.is_sender = false;
        let header = Header::read(&mut test_connection, &mut can_count).unwrap().unwrap();
        assert_eq!(ZFrameType::File, header.frame_type);
        let (block, _, _) = read_subpacket(&mut test_connection, 1024, true, false).unwrap();
        let file_name = str_from_null_terminated_utf8_unchecked(&block).to_string();
        assert_eq!("foo.bar", file_name);
        test_connection.is_sender = true;
        send.update(&mut test_connection, &transfer_state, &mut handler).expect("error.");
        send.update(&mut test_connection, &transfer_state, &mut handler).expect("error.");
        test_connection.is_sender = false;
        let header = Header::read(&mut test_connection, &mut can_count).unwrap().unwrap();
        assert_eq!(ZFrameType::Data, header.frame_type);

        match read_subpacket(&mut test_connection, 1024, true, false) {
            Ok((block_data, last, _)) => {
                assert!(last);
                assert_eq!(data, block_data);
            }
            Err(err) => {
                panic!("error reading subpacket: {err:?}");
            }
        }

        let header = Header::read(&mut test_connection, &mut can_count).unwrap().unwrap();
        assert_eq!(ZFrameType::Eof, header.frame_type);

        Header::from_flags(ZFrameType::RIinit, 0, 0, 0, 0x23)
            .write(&mut test_connection, HeaderType::Hex, false)
            .unwrap();
        test_connection.is_sender = true;
        send.update(&mut test_connection, &transfer_state, &mut handler).expect("error.");
        test_connection.is_sender = false;
        let header = Header::read(&mut test_connection, &mut can_count).unwrap().unwrap();
        assert_eq!(ZFrameType::Fin, header.frame_type);
    }

    #[test]
    fn test_encode_char_table() {
        let mut test_connection = TestConnection::new(true);

        for i in 0..255 {
            let data = vec![i as u8];
            let encoded = Zmodem::encode_subpacket_crc32(0x6B, &data, true);

            test_connection.is_sender = true;
            test_connection.send(encoded).unwrap();

            test_connection.is_sender = false;
            let (decoded, _, _) = crate::protocol::zmodem::rz::read_subpacket(&mut test_connection, 1024, true, false).unwrap();
            compare_data_packages(&data, &decoded);
        }
    }

    #[test]
    fn subpacket_bug() {
        let data = include_bytes!("sub_package_test1.dat").to_vec();
        let mut test_connection = TestConnection::new(true);
        test_connection.send(data).unwrap();
        test_connection.is_sender = false;
        crate::protocol::zmodem::rz::read_subpacket(&mut test_connection, 1024, true, false).unwrap();
    }

    fn compare_data_packages(orig: &[u8], encoded: &[u8]) {
        let upper: usize = orig.len().min(encoded.len());
        for i in 0..upper {
            if orig[i] != encoded[i] {
                println!("      org:    enc:");
                for j in i.saturating_sub(5)..(i + 5).min(upper) {
                    println!(
                        "{:-4}: 0x{:02X} {} 0x{:02X}",
                        j,
                        orig[j],
                        if orig[j] == encoded[j] { "==" } else { "!=" },
                        encoded[j]
                    );
                }
                break;
            }
        }
        assert_eq!(orig.len(), encoded.len());
    }
}
