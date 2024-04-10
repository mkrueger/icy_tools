#[cfg(test)]
use crate::protocol::Protocol;

#[cfg(test)]
pub fn test_sender(test_connection: &mut crate::ui::connect::TestConnection, files: Vec<crate::protocol::FileDescriptor>, send: &mut dyn Protocol) {
    use crate::protocol::{TestStorageHandler, TransferState};
    use std::sync::{Arc, Mutex};
    test_connection.is_sender = true;
    let send_transfer_state = Arc::new(Mutex::new(TransferState::default()));
    send.initiate_send(test_connection, files, &mut send_transfer_state.lock().unwrap())
        .expect("error.");

    let mut storage_handler = TestStorageHandler::new();
    while !send_transfer_state.lock().unwrap().is_finished {
        send.update(test_connection, &send_transfer_state, &mut storage_handler).expect("error.");
    }
}

#[cfg(test)]
pub fn test_receiver(test_connection: &mut crate::ui::connect::TestConnection, receiver: &mut dyn Protocol) -> crate::protocol::TestStorageHandler {
    use crate::protocol::{TestStorageHandler, TransferState};
    use std::sync::{Arc, Mutex};

    test_connection.is_sender = false;
    let send_transfer_state = Arc::new(Mutex::new(TransferState::default()));
    receiver
        .initiate_recv(test_connection, &mut send_transfer_state.lock().unwrap())
        .expect("error.");

    let mut storage_handler = TestStorageHandler::new();
    while !send_transfer_state.lock().unwrap().is_finished {
        receiver.update(test_connection, &send_transfer_state, &mut storage_handler).expect("error.");
    }
    storage_handler
}

#[cfg(test)]
mod xy_modem_tests {
    use crate::{
        com::TestChannel,
        protocol::{
            tests::{test_receiver, test_sender},
            xymodem::constants::{ACK, EOT, NAK, SOH, STX},
            FileDescriptor, XYModemVariant,
        },
        ui::connect::{DataConnection, TestConnection},
    };

    fn create_channel() -> TestChannel {
        let mut cmd_table = std::collections::HashMap::new();
        cmd_table.insert(b'C', "C".to_string());
        cmd_table.insert(b'G', "G".to_string());
        cmd_table.insert(0x04, "EOT".to_string());
        cmd_table.insert(0x06, "ACK".to_string());
        cmd_table.insert(0x15, "NAK".to_string());
        cmd_table.insert(0x18, "CAN".to_string());
        TestChannel::from_cmd_table(cmd_table, false)
    }

    #[test]
    fn test_xmodem_128block_sender() {
        let mut test_connection = TestConnection::new(false);

        let mut send = crate::protocol::XYmodem::new(XYModemVariant::XModem);

        let mut data = vec![1u8, 2, 5, 10];
        let files = vec![FileDescriptor::create_test("foo.bar".to_string(), data.clone())];

        test_connection
            .send(vec![
                NAK, ACK, ACK, // ACK EOT
            ])
            .unwrap();

        test_sender(&mut test_connection, files, &mut send);

        // construct result
        let mut result = vec![SOH, 0x01, 0xFE];
        data.resize(128, 0x1A);
        result.extend_from_slice(&data);
        result.push(0xAA); // CHECKSUM
        result.push(EOT);

        assert_eq!(result, test_connection.read_receive_buffer());
    }

    #[test]
    fn test_xmodem_128block_case2_sender() {
        let mut test_connection = TestConnection::new(false);
        let mut send = crate::protocol::XYmodem::new(XYModemVariant::XModem);
        let mut data = vec![1u8, 2, 5, 10];
        let files = vec![FileDescriptor::create_test("foo.bar".to_string(), data.clone())];

        test_connection.send(vec![b'C', ACK, ACK]).unwrap();

        test_sender(&mut test_connection, files, &mut send);

        // construct result
        let mut result = vec![SOH, 0x01, 0xFE];
        data.resize(128, 0x1A);
        result.extend_from_slice(&data);
        result.push(150); // CHECKSUM
        result.push(207); // CHECKSUM
        result.push(EOT);

        assert_eq!(result, test_connection.read_receive_buffer());
    }

    #[test]
    fn test_xmodem_1kblock_sender() {
        let mut test_connection = TestConnection::new(false);
        let mut send = crate::protocol::XYmodem::new(XYModemVariant::XModem1k);
        let mut data = vec![5; 900];
        let files = vec![FileDescriptor::create_test("foo.bar".to_string(), data.clone())];

        test_connection
            .send(vec![
                b'C', ACK, ACK, // ACK EOT
            ])
            .unwrap();

        test_sender(&mut test_connection, files, &mut send);

        // construct result
        let mut result = vec![STX, 0x01, 0xFE];
        data.resize(1024, 0x1A);
        result.extend_from_slice(&data);
        result.push(184); // CHECKSUM
        result.push(85); // CHECKSUM
        result.push(EOT);

        assert_eq!(result, test_connection.read_receive_buffer());
    }

    #[test]
    fn test_xmodem_128block_receiver() {
        let mut test_connection = TestConnection::new(true);
        let mut recv = crate::protocol::XYmodem::new(XYModemVariant::XModem);
        let data = vec![1u8, 2, 5, 10];

        let mut result = vec![SOH, 0x01, 0xFE];
        let mut cloned_data = data.clone();
        cloned_data.resize(128, 0x1A);
        result.extend_from_slice(&cloned_data);
        result.push(0xAA); // CHECKSUM
        result.push(EOT);
        test_connection.send(result).unwrap();

        let storage_handler = test_receiver(&mut test_connection, &mut recv);
        assert_eq!(1, storage_handler.file.len());
        for (_, file_data) in storage_handler.file {
            assert_eq!(data, file_data);
        }
    }

    #[test]
    fn test_xmodem_1kblock_receiver() {
        let mut test_connection = TestConnection::new(true);
        let mut recv = crate::protocol::XYmodem::new(XYModemVariant::XModem1k);
        let data = vec![5; 900];

        let mut result = vec![STX, 0x01, 0xFE];
        let mut cloned_data = data.clone();
        cloned_data.resize(1024, 0x1A);
        result.extend_from_slice(&cloned_data);
        result.push(184); // CHECKSUM
        result.push(85); // CHECKSUM
        result.push(EOT);
        test_connection.send(result).unwrap();

        let storage_handler = test_receiver(&mut test_connection, &mut recv);
        assert_eq!(1, storage_handler.file.len());
        for (_, file_data) in storage_handler.file {
            assert_eq!(data, file_data);
        }
    }

    #[test]
    fn test_ymodem_sender() {
        let mut test_connection = TestConnection::new(false);

        let mut send = crate::protocol::XYmodem::new(XYModemVariant::YModem);

        let mut data = vec![1u8, 2, 5, 10];
        let files = vec![FileDescriptor::create_test("foo.bar".to_string(), data.clone())];

        test_connection
            .send(vec![
                b'C', ACK, b'C', ACK, ACK, ACK, // ACK EOT
            ])
            .unwrap();

        test_sender(&mut test_connection, files, &mut send);

        // construct result
        let mut result = vec![SOH, 0x00, 0xFF];
        result.extend_from_slice(b"foo.bar");
        result.extend_from_slice(&[0, b'4']); // length

        result.extend_from_slice(vec![0; 128 - "foo.bar".len() - 2].as_slice());
        result.extend_from_slice(&[108, 107]); // CHECKSUM

        result.extend_from_slice(&[SOH, 0x01, 0xFE]);
        data.resize(128, 0x1A);
        result.extend_from_slice(&data);
        result.extend_from_slice(&[150, 207]); // CHECKSUM
        result.push(EOT);

        assert_eq!(result, test_connection.read_receive_buffer());
    }

    #[test]
    fn test_ymodem_receiver() {
        let mut test_connection = TestConnection::new(true);
        let mut recv = crate::protocol::XYmodem::new(XYModemVariant::YModem);
        let data = vec![1u8, 2, 5, 10];
        let mut result = vec![SOH, 0x00, 0xFF];

        result.extend_from_slice(b"foo.bar");
        result.extend_from_slice(&[0, b'4']); // length
        result.extend_from_slice(vec![0; 128 - "foo.bar".len() - 2].as_slice());
        result.extend_from_slice(&[108, 107]); // CHECKSUM
        result.extend_from_slice(&[SOH, 0x01, 0xFE]);

        let mut cloned_data = data.clone();
        cloned_data.resize(128, 0x1A);
        result.extend_from_slice(&cloned_data);
        result.extend_from_slice(&[150, 207]); // CHECKSUM
        result.push(EOT); // -> NACK
        result.push(EOT); // -> ACK

        // No next file:
        result.extend_from_slice(&[SOH, 0x00, 0xFF]);
        result.extend_from_slice(vec![0; 128].as_slice());
        result.extend_from_slice(&[0, 0]);

        test_connection.send(result).unwrap();

        let storage_handler = test_receiver(&mut test_connection, &mut recv);
        assert_eq!(1, storage_handler.file.len());
        assert_eq!(data, storage_handler.file["foo.bar"]);
    }
}
