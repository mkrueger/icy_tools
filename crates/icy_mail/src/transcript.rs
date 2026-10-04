use std::{io::Write, path::Path};

use crate::{drafts::atomic_write, qwk::QwkPackage, LANGUAGE_LOADER};
use i18n_embed_fl::fl;

pub fn save_transcript(package: &QwkPackage, indices: &[usize], source: &Path, destination: &Path) -> crate::Res<()> {
    if indices.is_empty() {
        return Err(fl!(LANGUAGE_LOADER, "batch-save-empty").into());
    }
    if destination.exists() && source.canonicalize()? == destination.canonicalize()? {
        return Err(fl!(LANGUAGE_LOADER, "batch-save-source-packet").into());
    }
    let mut transcript = String::new();
    for &index in indices {
        let info = package.infos.get(index).ok_or_else(|| fl!(LANGUAGE_LOADER, "app-message-unavailable"))?;
        let message = package.get_message(index)?;
        transcript.push_str(&format!(
            "From: {}\nTo: {}\nSubject: {}\nDate: {}\nConference: {}\nMessage: {}\n\n",
            info.from, info.to, info.subject, info.date_str, info.conference, info.number
        ));
        transcript.push_str(&crate::text::to_utf8(&message.text));
        if !transcript.ends_with('\n') {
            transcript.push('\n');
        }
        transcript.push_str("\n------------------------------------------------------------------------\n\n");
    }
    atomic_write(destination, |file| {
        file.write_all(transcript.as_bytes())?;
        Ok(())
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transcript_contains_ordered_headers_and_utf8_bodies() {
        let (dir, package) = crate::qwk::tests::load();
        let path = dir.path().join("messages.txt");
        save_transcript(&package, &[2, 0], &dir.path().join("TEST.QWK"), &path).unwrap();
        let transcript = std::fs::read_to_string(path).unwrap();
        let first = &package.infos[2];
        let second = &package.infos[0];
        assert!(transcript.starts_with(&format!("From: {}\nTo: {}\nSubject: {}\n", first.from, first.to, first.subject)));
        assert_eq!(transcript.matches("\nConference: ").count(), 2);
        assert!(transcript.find(&format!("Message: {}\n", first.number)).unwrap() < transcript.find(&format!("Message: {}\n", second.number)).unwrap());
        for index in [2, 0] {
            assert!(transcript.contains(&crate::text::to_utf8(&package.get_message(index).unwrap().text)));
        }
    }

    #[test]
    fn invalid_batch_preserves_destination_and_source() {
        let (dir, package) = crate::qwk::tests::load();
        let source = dir.path().join("TEST.QWK");
        let original = std::fs::read(&source).unwrap();
        let destination = dir.path().join("messages.txt");
        std::fs::write(&destination, "keep").unwrap();
        for indices in [vec![], vec![0, usize::MAX]] {
            assert!(save_transcript(&package, &indices, &source, &destination).is_err());
            assert_eq!(std::fs::read_to_string(&destination).unwrap(), "keep");
        }
        assert!(save_transcript(&package, &[0], &source, &source).is_err());
        assert_eq!(std::fs::read(source).unwrap(), original);
    }
}
