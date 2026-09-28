//! Compressed and PCM audio files (MP3, Ogg Vorbis, FLAC, WAV, AAC/M4A) for the music
//! player: tags and length for the info sheet, and a rodio decoder for playback.

use std::{io::Cursor, path::Path, sync::Arc, time::Duration};

use symphonia::core::{
    formats::FormatOptions,
    io::MediaSourceStream,
    meta::{MetadataOptions, MetadataRevision, StandardTagKey},
    probe::Hint,
};

pub const EXTENSIONS: &[&str] = &["mp3", "ogg", "oga", "flac", "wav", "m4a", "aac"];

fn extension(path: &Path) -> Option<String> {
    path.extension().and_then(|ext| ext.to_str()).map(|ext| ext.to_ascii_lowercase())
}

pub fn is_audio_file(path: &Path) -> bool {
    extension(path).is_some_and(|ext| EXTENSIONS.contains(&ext.as_str()))
}

#[derive(Clone, Debug)]
pub struct AudioFile {
    data: Arc<[u8]>,
    hint: Option<String>,
    pub format: &'static str,
    /// The title tag, empty without one.
    pub title: String,
    /// Artist, album and the other tags worth showing, as (label, value).
    pub tags: Vec<(&'static str, String)>,
    pub sample_rate: u32,
    pub channels: usize,
    pub duration: f64,
}

fn format_name(extension: Option<&str>) -> &'static str {
    match extension {
        Some("mp3") => "MP3 audio",
        Some("ogg" | "oga") => "Ogg Vorbis",
        Some("flac") => "FLAC audio",
        Some("wav") => "WAV audio",
        Some("m4a" | "aac") => "AAC audio",
        _ => "Audio",
    }
}

const SHOWN_TAGS: [(StandardTagKey, &str); 6] = [
    (StandardTagKey::Artist, "Artist"),
    (StandardTagKey::Album, "Album"),
    (StandardTagKey::Date, "Date"),
    (StandardTagKey::Genre, "Genre"),
    (StandardTagKey::Comment, "Comment"),
    (StandardTagKey::Copyright, "Copyright"),
];

/// iTunes stores loudness and gapless data as comments of hexadecimal words.
fn is_encoder_data(value: &str) -> bool {
    let mut words = value.split_whitespace().peekable();
    words.peek().is_some() && words.all(|word| word.len() == 8 && word.chars().all(|ch| ch.is_ascii_hexdigit()))
}

impl AudioFile {
    /// Reads the stream parameters and tags; fails for files no decoder supports.
    pub fn load(path: &Path, data: &[u8]) -> anyhow::Result<Self> {
        let hint = extension(path);
        let data: Arc<[u8]> = Arc::from(data);
        let source = MediaSourceStream::new(Box::new(Cursor::new(data.clone())), Default::default());
        let mut probe_hint = Hint::new();
        if let Some(hint) = &hint {
            probe_hint.with_extension(hint);
        }
        let mut probed = symphonia::default::get_probe()
            .format(&probe_hint, source, &FormatOptions::default(), &MetadataOptions::default())
            .map_err(|error| anyhow::anyhow!("not a playable audio file: {error}"))?;
        let track = probed
            .format
            .default_track()
            .ok_or_else(|| anyhow::anyhow!("the audio file has no audio track"))?;
        let params = track.codec_params.clone();
        let track_id = track.id;
        let sample_rate = params.sample_rate.unwrap_or(0);
        let channels = params.channels.map_or(0, |channels| channels.count());

        let mut revisions: Vec<MetadataRevision> = Vec::new();
        if let Some(metadata) = probed.metadata.get() {
            revisions.extend(metadata.current().cloned());
        }
        revisions.extend(probed.format.metadata().current().cloned());
        let tag = |key: StandardTagKey| {
            revisions
                .iter()
                .flat_map(|revision| revision.tags())
                .find(|tag| tag.std_key == Some(key))
                .map(|tag| tag.value.to_string().trim().to_owned())
                .filter(|value| !value.is_empty())
        };
        let title = tag(StandardTagKey::TrackTitle).unwrap_or_default();
        let tags = SHOWN_TAGS
            .iter()
            .filter_map(|(key, label)| tag(*key).map(|value| (*label, value)))
            .filter(|(_, value)| !is_encoder_data(value))
            .collect();

        let frames = match params.n_frames {
            Some(frames) => frames,
            // Without a frame count (e.g. MP3 without a Xing header) add up the packet lengths;
            // demuxing is cheap compared to decoding.
            None => {
                let mut frames = 0;
                while let Ok(packet) = probed.format.next_packet() {
                    if packet.track_id() == track_id {
                        frames += packet.dur;
                    }
                }
                frames
            }
        };
        let duration = match (params.time_base, sample_rate) {
            (Some(time_base), _) => {
                let time = time_base.calc_time(frames);
                time.seconds as f64 + time.frac
            }
            (None, rate) if rate > 0 => frames as f64 / rate as f64,
            _ => 0.0,
        };
        let mut file = Self {
            data,
            format: format_name(hint.as_deref()),
            hint,
            title,
            tags,
            sample_rate,
            channels,
            duration,
        };
        // Some containers (e.g. MP4) only know the layout once decoding starts.
        let decoder = file.decoder()?;
        if file.channels == 0 {
            use rodio::Source;
            file.channels = decoder.channels().get() as usize;
            file.sample_rate = decoder.sample_rate().get();
        }
        Ok(file)
    }

    fn decoder(&self) -> anyhow::Result<FileDecoder> {
        let mut builder = rodio::Decoder::builder()
            .with_data(Cursor::new(self.data.clone()))
            .with_byte_len(self.data.len() as u64)
            .with_seekable(true);
        if let Some(hint) = &self.hint {
            builder = builder.with_hint(hint);
        }
        builder.build().map_err(|error| anyhow::anyhow!("cannot decode the audio file: {error}"))
    }
}

type FileDecoder = rodio::Decoder<Cursor<Arc<[u8]>>>;

/// The decoder without span lengths. rodio's converter restarts at every span end and
/// treats an empty first span (Ogg Vorbis reports one) as the end of the stream. Audio files
/// keep their rate and layout, so the conversion never needs to restart.
struct Unspanned(FileDecoder);

impl Iterator for Unspanned {
    type Item = rodio::Sample;

    fn next(&mut self) -> Option<rodio::Sample> {
        self.0.next()
    }
}

impl rodio::Source for Unspanned {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> rodio::ChannelCount {
        self.0.channels()
    }

    fn sample_rate(&self) -> rodio::SampleRate {
        self.0.sample_rate()
    }

    fn total_duration(&self) -> Option<Duration> {
        self.0.total_duration()
    }

    fn try_seek(&mut self, position: Duration) -> Result<(), rodio::source::SeekError> {
        self.0.try_seek(position)
    }
}

/// Stereo samples at the output rate, converted from the file's rate and channel layout.
pub struct AudioStream {
    samples: rodio::source::UniformSourceIterator<Unspanned>,
}

impl AudioStream {
    pub fn new(file: &AudioFile, rate: u32) -> anyhow::Result<Self> {
        let rate = std::num::NonZero::new(rate).ok_or_else(|| anyhow::anyhow!("invalid sample rate"))?;
        let stereo = std::num::NonZero::new(2).unwrap();
        Ok(Self {
            samples: rodio::source::UniformSourceIterator::new(Unspanned(file.decoder()?), stereo, rate),
        })
    }

    pub fn next_f32(&mut self) -> Option<f32> {
        self.samples.next().map(|sample| sample as f32)
    }

    /// Seeks and returns the new position; decoders that cannot seek restart instead.
    pub fn seek_seconds(&mut self, file: &AudioFile, rate: u32, seconds: f64) -> f64 {
        use rodio::Source;
        if self.samples.try_seek(Duration::from_secs_f64(seconds.max(0.0))).is_ok() {
            return seconds;
        }
        if let Ok(stream) = Self::new(file, rate) {
            *self = stream;
        }
        0.0
    }
}

#[cfg(test)]
pub(crate) fn test_wav(seconds: f64, rate: u32) -> Vec<u8> {
    let frames = (seconds * rate as f64) as u32;
    let data_len = frames * 4;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data_len).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes()); // PCM
    bytes.extend_from_slice(&2u16.to_le_bytes()); // stereo
    bytes.extend_from_slice(&rate.to_le_bytes());
    bytes.extend_from_slice(&(rate * 4).to_le_bytes());
    bytes.extend_from_slice(&4u16.to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_len.to_le_bytes());
    for frame in 0..frames {
        let sample = if (frame / 50) % 2 == 0 { 8000i16 } else { -8000 };
        bytes.extend_from_slice(&sample.to_le_bytes());
        bytes.extend_from_slice(&sample.to_le_bytes());
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audio_files_are_recognized_by_extension() {
        for name in ["SONG.MP3", "a.ogg", "b.flac", "c.wav", "d.m4a"] {
            assert!(is_audio_file(Path::new(name)), "{name}");
        }
        assert!(!is_audio_file(Path::new("art.ans")));
        assert!(!is_audio_file(Path::new("mp3")));
    }

    #[test]
    fn wav_files_report_length_and_decode_at_the_output_rate() {
        let file = AudioFile::load(Path::new("tone.WAV"), &test_wav(1.5, 22_050)).unwrap();
        assert_eq!(file.format, "WAV audio");
        assert_eq!((file.sample_rate, file.channels), (22_050, 2));
        assert!((file.duration - 1.5).abs() < 0.01, "{}", file.duration);
        let mut stream = AudioStream::new(&file, 44_100).unwrap();
        let mut count = 0usize;
        while stream.next_f32().is_some() {
            count += 1;
        }
        let expected = (1.5 * 44_100.0 * 2.0) as usize;
        assert!(count.abs_diff(expected) < 400, "{count} samples, expected about {expected}");
        let position = stream.seek_seconds(&file, 44_100, 1.0);
        assert!((position - 1.0).abs() < 0.01);
        assert!(stream.next_f32().is_some(), "playback continues after seeking");
    }

    #[test]
    fn ogg_vorbis_plays_despite_its_empty_first_span() {
        let file = AudioFile::load(Path::new("sine.OGG"), include_bytes!("test_data/sine.ogg")).unwrap();
        assert_eq!(file.format, "Ogg Vorbis");
        assert_eq!(file.title, "Fixture");
        assert_eq!(file.tags, vec![("Artist", "Icy View".to_owned())]);
        assert!((file.duration - 0.5).abs() < 0.05, "{}", file.duration);
        let mut stream = AudioStream::new(&file, 48_000).unwrap();
        let samples: Vec<f32> = std::iter::from_fn(|| stream.next_f32()).collect();
        assert!(samples.len().abs_diff(48_000) < 2_000, "{} samples of 0.5 s stereo", samples.len());
        assert!(samples.iter().any(|sample| sample.abs() > 0.05), "the sine wave is audible");
    }

    #[test]
    fn itunes_encoder_comments_are_hidden() {
        assert!(is_encoder_data(" 00003F2F 00003CD3 0001897A 000198F1"));
        assert!(!is_encoder_data("Made with love"));
        assert!(!is_encoder_data(""));
    }

    #[test]
    fn broken_files_are_rejected() {
        assert!(AudioFile::load(Path::new("song.mp3"), b"ID3\x04\x00 not really").is_err());
    }
}
