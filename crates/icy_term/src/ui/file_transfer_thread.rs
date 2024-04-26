/*
use icy_net::protocol::TransferProtocolType;

use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use super::connect::Connection;
use super::dialogs;
pub struct FileTransferThread {
    pub current_transfer: Arc<Mutex<TransferState>>,
    pub file_transfer_dialog: dialogs::up_download_dialog::FileTransferDialog,

    pub join_handle: Option<JoinHandle<Box<Connection>>>,
}

impl FileTransferThread {
    pub fn new(mut connection: Box<Connection>, protocol_type: TransferProtocolType, download: bool, files_opt: Option<Vec<FileDescriptor>>) -> Self {
        let current_transfer = Arc::new(Mutex::new(TransferState::default()));

        let current_transfer2 = current_transfer.clone();
        let join_handle = std::thread::Builder::new().name("file_transfer".to_string()).spawn(move || {
            let mut protocol = protocol_type.create();
            if protocol.use_raw_transfer() {
                if let Err(err) = connection.set_raw_mode(true) {
                    log::error!("Error setting raw mode on file transfer thread: {err}");
                    return connection;
                }
            }
            if let Err(err) = if download {
                protocol.initiate_recv(&mut *connection, &mut current_transfer2.lock().unwrap())
            } else {
                protocol.initiate_send(&mut *connection, files_opt.unwrap(), &mut current_transfer2.lock().unwrap())
            } {
                log::error!("{err}");
                return connection;
            }
/*
            if let Ok(mut storage_handler) = crate::protocol::DiskStorageHandler::new() {
                loop {
                    if let Err(err) = connection.update_state() {
                        log::error!("Error updating state on file transfer thread: {err}");
                        break;
                    }
                    match protocol.update(&mut *connection, &current_transfer2, &mut storage_handler) {
                        Ok(b) => {
                            if !b {
                                break;
                            }
                        }
                        Err(err) => {
                            log::error!("Error updating protocol on file transfer thread: {err}");
                            break;
                        }
                    }
                    match current_transfer2.lock() {
                        Ok(ct) => {
                            if ct.request_cancel {
                                if let Err(err) = protocol.cancel(&mut *connection) {
                                    log::error!("Error sending cancel request on file transfer thread: {err}");
                                }
                                break;
                            }
                        }
                        Err(err) => {
                            log::error!("Error locking current_transfer on file transfer thread: {err}");
                            break;
                        }
                    }
                }
            }*/
            if protocol.use_raw_transfer() {
                if let Err(err) = connection.set_raw_mode(false) {
                    log::error!("Error setting raw mode on file transfer thread: {err}");
                }
            }
            current_transfer2.lock().unwrap().is_finished = true;

            connection
        });
        if let Err(err) = &join_handle {
            log::error!("Error creating file transfer thread: {err}");
        }

        Self {
            current_transfer,
            file_transfer_dialog: dialogs::up_download_dialog::FileTransferDialog::new(),
            join_handle: Some(join_handle.unwrap()),
        }
    }
}
*/
