use std::{
    fs::{File, OpenOptions},
    path::Path,
    sync::Mutex,
};

use tracing_subscriber::{filter::LevelFilter, layer::SubscriberExt, util::SubscriberInitExt, Layer};

fn open_log(path: &Path) -> std::io::Result<File> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let truncate = match std::fs::metadata(path) {
        Ok(metadata) => metadata.len() > 256 * 1024,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
        Err(error) => return Err(error),
    };
    OpenOptions::new().create(true).write(true).append(!truncate).truncate(truncate).open(path)
}

fn subscriber(file: File, debug: bool) -> impl SubscriberInitExt {
    let level = if debug { LevelFilter::DEBUG } else { LevelFilter::WARN };
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .with_ansi(false)
                .with_writer(Mutex::new(file))
                .with_filter(level),
        )
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr).with_filter(level))
}

/// Log both tracing events and log records to stderr and the application's logfile.
pub fn init(debug: bool) -> anyhow::Result<()> {
    let directories = directories::ProjectDirs::from("com", "GitHub", "icy_mail").ok_or_else(|| anyhow::anyhow!("Error getting log directory"))?;
    let path = directories.config_dir().join("icy_mail.log");
    subscriber(open_log(&path)?, debug).try_init()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn warnings_reach_logfile_and_large_logs_are_rotated() {
        let directory = crate::qwk::tests::TempDir::new();
        let path = directory.path().join("logs/icy_mail.log");
        open_log(&path).unwrap().write_all(b"previous run\n").unwrap();
        {
            let _guard = subscriber(open_log(&path).unwrap(), false).set_default();
            log::warn!("Blue Wave logfile regression warning");
            let mut files = crate::blue_wave::fixture_files(3, true);
            files[0].1[76..119].fill(0);
            files[0].1[1236..1257].fill(0);
            files[1].1[8..10].copy_from_slice(&2u16.to_le_bytes());
            files[1].1[24..28].copy_from_slice(&u32::MAX.to_le_bytes());
            files[2].1[144..164].fill(b'?');
            files[3].1[0] = b'X';
            let packet = directory.path().join("warnings.zip");
            crate::blue_wave_tests::packet(&packet, &files);
            let package = crate::qwk::QwkPackage::load_from_file(&packet).unwrap();
            assert_eq!(package.read_message(0).unwrap().text.as_slice(), b"XHello\n\x82!");
            assert!(crate::qwk::QwkPackage::load_from_file(directory.path().join("missing.zip")).is_err());
        }
        let contents = std::fs::read_to_string(&path).unwrap();
        assert!(contents.starts_with("previous run\n"));
        assert!(contents.contains("Blue Wave logfile regression warning"));
        assert!(contents.contains("no login identity"));
        assert!(contents.contains("Invalid Blue Wave echotag"));
        assert!(contents.contains("personal count exceeds message count"));
        assert!(contents.contains("invalid unused FTI pointer"));
        assert!(contents.contains("unrecognized date"));
        assert!(contents.contains("missing leading marker"));
        assert!(contents.contains("unable to load mail packet"));
        assert!(contents.contains("missing.zip"));
        assert!(!contents.contains("\x1b["));
        std::fs::write(&path, vec![b'x'; 256 * 1024 + 1]).unwrap();
        open_log(&path).unwrap().write_all(b"new run\n").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"new run\n");
    }
}
