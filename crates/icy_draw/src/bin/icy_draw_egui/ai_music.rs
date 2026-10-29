//! Tools for the ANSI music editor. They edit a draft of the music string; the user listens to
//! it, reviews the diff and applies it.

use icy_draw::ansi_music::{self, Articulation, Event, Introducer, HIGHEST_PITCH};
use serde_json::{json, Value};

const MAX_TEXT_BYTES: usize = 256 * 1024;
const MAX_EVENTS_PER_READ: usize = 2000;

const API_DOC: &str = "\
ANSI music (CTerm/SyncTERM/BananaCom), one music string per line of the editor text.
Commands (case-insensitive, spaces are ignored):
  A-G      note in the current octave; + or # sharp, - flat; a number after it is its length, . dots it (1.5x)
  N<n>     note number 0-71 (O0 C = 0); above 71 it is a rest
  P<n>     rest (pause), same length/dot rules as notes
  O<n>     octave 0-6;  > one up, < one down
  L<n>     default length 1-64 (1 whole, 2 half, 4 quarter, 8 eighth, 16, 32, 64)
  T<n>     tempo 32-255 quarter notes per minute
  MN ML MS articulation normal (7/8 sounding), legato (full), staccato (3/4)
  MF MB    foreground (terminal waits) / background playback
Defaults at the start: T120 O4 L4 MN MF. Settings carry over between strings.
Pitches here are 0 (O0 C) to 83 (O6 B); O4 C is 48 and sounds at 1046.5 Hz, so O2/O3 are the usual melody range.
Characters other than the commands make terminals discard the whole string.
A whole note lasts 240000 / tempo ms. Keep strings short; lines over 1022 characters are split on save.
Write music as the user asks: a melody, rhythm, bass line on one voice (ANSI music is monophonic).";

#[derive(Clone)]
pub struct MusicDraft {
    /// The music string when drawing started; the diff and stale checks compare against it.
    pub original: String,
    pub text: String,
    introducer: Introducer,
    file_name: Option<String>,
    selected: Option<usize>,
}

impl MusicDraft {
    pub fn new(text: &str, introducer: Introducer, file_name: Option<String>, selected: Option<usize>) -> Self {
        Self {
            original: text.to_owned(),
            text: text.to_owned(),
            introducer,
            file_name,
            selected,
        }
    }

    pub fn changed(&self) -> bool {
        self.text != self.original
    }

    pub fn diff(&self) -> super::animation_tools::Diff {
        super::animation_tools::text_diff(&self.original, &self.text)
    }

    /// Notes and length of the draft, for the proposal.
    pub fn summary(&self) -> (usize, f64) {
        let events = ansi_music::parse(&self.text).events;
        let notes = events.iter().filter(|event| matches!(event, Event::Note { pitch: Some(_), .. })).count();
        (notes, total_millis(&events) / 1000.0)
    }

    pub fn music(&self) -> icy_parser_core::AnsiMusic {
        ansi_music::to_ansi_music(&ansi_music::parse(&self.text).events, 0)
    }

    pub fn call(&mut self, tool: &str, arguments: &Value) -> Result<String, String> {
        match tool {
            "icy_music_info" => Ok(self.info().to_string()),
            "icy_music_api" => Ok(API_DOC.to_owned()),
            "icy_read_music" => self.read(arguments),
            "icy_write_music" => {
                let text = arguments.get("text").and_then(Value::as_str).ok_or("text is required")?;
                self.set_text(text.to_owned())
            }
            "icy_replace_music_events" => self.replace_events(arguments),
            _ => Err(format!("Unknown tool {tool}")),
        }
    }

    fn info(&self) -> Value {
        let parsed = ansi_music::parse(&self.text);
        let pitches: Vec<u8> = parsed
            .events
            .iter()
            .filter_map(|event| match event {
                Event::Note { pitch, .. } => *pitch,
                _ => None,
            })
            .collect();
        json!({
            "file": self.file_name,
            "introducer": self.introducer.label(),
            "lines": self.text.split('\n').count(),
            "characters": self.text.len(),
            "events": parsed.events.len(),
            "notes": pitches.len(),
            "rests": parsed.events.iter().filter(|event| matches!(event, Event::Note { pitch: None, .. })).count(),
            "duration_seconds": total_millis(&parsed.events) / 1000.0,
            "lowest_note": pitches.iter().min().map(|pitch| ansi_music::note_name(*pitch)),
            "highest_note": pitches.iter().max().map(|pitch| ansi_music::note_name(*pitch)),
            "selected_event": self.selected,
            "problems": problems(&self.text),
        })
    }

    fn read(&self, arguments: &Value) -> Result<String, String> {
        let parsed = ansi_music::parse(&self.text);
        let start = arguments.get("start").and_then(Value::as_u64).unwrap_or(0) as usize;
        let count = arguments
            .get("count")
            .and_then(Value::as_u64)
            .map_or(MAX_EVENTS_PER_READ, |count| count as usize)
            .min(MAX_EVENTS_PER_READ);
        let timing = ansi_music::timing(&parsed.events);
        let events: Vec<Value> = parsed
            .events
            .iter()
            .enumerate()
            .skip(start)
            .take(count)
            .map(|(index, event)| {
                let mut value = event_json(event);
                value["index"] = json!(index);
                value["text"] = json!(&self.text[parsed.spans[index].clone()]);
                value["start_ms"] = json!(timing[index].0);
                value["duration_ms"] = json!(timing[index].1);
                value
            })
            .collect();
        Ok(json!({
            "text": self.text,
            "events": events,
            "total_events": parsed.events.len(),
            "problems": problems(&self.text),
        })
        .to_string())
    }

    fn set_text(&mut self, text: String) -> Result<String, String> {
        if text.len() > MAX_TEXT_BYTES {
            return Err(format!("The music is limited to {MAX_TEXT_BYTES} bytes"));
        }
        let problems = problems(&text);
        if !problems.is_empty() {
            return Err(format!(
                "Rejected: terminals discard strings with these characters: {}. Use only the music commands of icy_music_api.",
                Value::from(problems)
            ));
        }
        self.text = text;
        Ok(self.info().to_string())
    }

    fn replace_events(&mut self, arguments: &Value) -> Result<String, String> {
        let mut events = ansi_music::parse(&self.text).events;
        let start = arguments.get("start").and_then(Value::as_u64).ok_or("start is required")? as usize;
        let end = arguments.get("end").and_then(Value::as_u64).map_or(start, |end| end as usize);
        if start > end || end > events.len() {
            return Err(format!("start..end must be within 0..={} with start <= end", events.len()));
        }
        let new: Vec<Event> = arguments
            .get("events")
            .and_then(Value::as_array)
            .ok_or("events is required")?
            .iter()
            .enumerate()
            .map(|(index, value)| parse_event(value).map_err(|error| format!("events[{index}]: {error}")))
            .collect::<Result<_, _>>()?;
        events.splice(start..end, new);
        // The editor text is rewritten from the events, as the piano roll does.
        self.set_text(ansi_music::generate(&events))
    }
}

fn total_millis(events: &[Event]) -> f64 {
    ansi_music::timing(events).last().map_or(0.0, |(start, duration)| start + duration)
}

fn problems(text: &str) -> Vec<Value> {
    ansi_music::parse(text)
        .problems
        .iter()
        .map(|problem| json!({ "character": problem.character.to_string(), "position": problem.position }))
        .collect()
}

fn articulation_name(articulation: Articulation) -> &'static str {
    match articulation {
        Articulation::Normal => "normal",
        Articulation::Legato => "legato",
        Articulation::Staccato => "staccato",
    }
}

fn event_json(event: &Event) -> Value {
    match *event {
        Event::Note {
            pitch: Some(pitch),
            length,
            dotted,
        } => json!({ "type": "note", "pitch": pitch, "note": ansi_music::note_name(pitch), "length": length, "dotted": dotted }),
        Event::Note { pitch: None, length, dotted } => json!({ "type": "rest", "length": length, "dotted": dotted }),
        Event::Tempo(tempo) => json!({ "type": "tempo", "value": tempo }),
        Event::Articulation(articulation) => json!({ "type": "articulation", "value": articulation_name(articulation) }),
        Event::Background(background) => json!({ "type": "background", "value": background }),
    }
}

/// `48`, or a name like `O4 C#`, `O3Eb` or `C#4`.
fn parse_pitch(value: &Value) -> Result<u8, String> {
    if let Some(pitch) = value.as_u64() {
        return u8::try_from(pitch)
            .ok()
            .filter(|pitch| *pitch <= HIGHEST_PITCH)
            .ok_or_else(|| format!("pitch must be 0..={HIGHEST_PITCH}"));
    }
    let name: String = value
        .as_str()
        .ok_or("pitch must be a number or a note name")?
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    let name = name.to_ascii_uppercase();
    let (octave, rest) = match name.strip_prefix('O') {
        Some(rest) => {
            let digits = rest.chars().take_while(char::is_ascii_digit).count();
            (rest[..digits].parse::<i32>().ok(), &rest[digits..])
        }
        None => {
            let digits = name.chars().rev().take_while(char::is_ascii_digit).count();
            (name[name.len() - digits..].parse::<i32>().ok(), &name[..name.len() - digits])
        }
    };
    let octave = octave.ok_or_else(|| format!("{name}: the octave is missing, e.g. O4 C#"))?;
    let mut chars = rest.chars();
    let semitone = match chars.next() {
        Some('C') => 0,
        Some('D') => 2,
        Some('E') => 4,
        Some('F') => 5,
        Some('G') => 7,
        Some('A') => 9,
        Some('B') => 11,
        _ => return Err(format!("{name}: expected a note letter A-G")),
    };
    let accidental: i32 = chars
        .map(|c| match c {
            '#' | '+' => 1,
            '-' | 'B' => -1,
            _ => 0,
        })
        .sum();
    u8::try_from(octave * 12 + semitone + accidental)
        .ok()
        .filter(|pitch| *pitch <= HIGHEST_PITCH)
        .ok_or_else(|| format!("{name} is outside O0 C..O6 B"))
}

fn parse_event(value: &Value) -> Result<Event, String> {
    let kind = value.get("type").and_then(Value::as_str).ok_or("type is required")?;
    let length = || -> Result<u8, String> {
        let length = value.get("length").and_then(Value::as_u64).unwrap_or(4);
        u8::try_from(length)
            .ok()
            .filter(|length| (1..=64).contains(length))
            .ok_or_else(|| "length must be 1..=64".to_owned())
    };
    let dotted = value.get("dotted").and_then(Value::as_bool).unwrap_or(false);
    Ok(match kind {
        "note" => Event::note(
            parse_pitch(value.get("pitch").or_else(|| value.get("note")).ok_or("pitch is required")?)?,
            length()?,
            dotted,
        ),
        "rest" => Event::rest(length()?, dotted),
        "tempo" => {
            let tempo = value.get("value").and_then(Value::as_u64).ok_or("value is required")?;
            Event::Tempo(u8::try_from(tempo).ok().filter(|tempo| *tempo >= 32).ok_or("tempo must be 32..=255")?)
        }
        "articulation" => Event::Articulation(match value.get("value").and_then(Value::as_str) {
            Some("normal") => Articulation::Normal,
            Some("legato") => Articulation::Legato,
            Some("staccato") => Articulation::Staccato,
            _ => return Err("value must be normal, legato or staccato".into()),
        }),
        "background" => Event::Background(value.get("value").and_then(Value::as_bool).ok_or("value must be true or false")?),
        _ => return Err(format!("unknown type {kind}; use note, rest, tempo, articulation or background")),
    })
}

pub fn tool_specs() -> Vec<(&'static str, &'static str, Value)> {
    let event = json!({
        "type": "object",
        "properties": {
            "type": { "type": "string", "enum": ["note", "rest", "tempo", "articulation", "background"] },
            "pitch": { "description": "Notes: 0 (O0 C) to 83 (O6 B), or a name like \"O3 C#\"" },
            "length": { "type": "integer", "minimum": 1, "maximum": 64, "description": "Notes and rests: 1 whole, 2 half, 4 quarter, 8 eighth..." },
            "dotted": { "type": "boolean" },
            "value": { "description": "tempo: 32-255; articulation: normal, legato or staccato; background: true or false" },
        },
        "required": ["type"],
    });
    vec![
        (
            "icy_music_info",
            "[ANSI music editor] Describe the open music: introducer, events, notes, duration, range and characters terminals reject.",
            json!({ "type": "object", "properties": {} }),
        ),
        (
            "icy_music_api",
            "[ANSI music editor] Return the ANSI music command reference.",
            json!({ "type": "object", "properties": {} }),
        ),
        (
            "icy_read_music",
            "[ANSI music editor] Read the music string and its events with index, pitch, length and timing.",
            json!({
                "type": "object",
                "properties": {
                    "start": { "type": "integer", "minimum": 0, "description": "First event, default 0" },
                    "count": { "type": "integer", "minimum": 1, "description": "Number of events, at most 2000" },
                },
            }),
        ),
        (
            "icy_write_music",
            "[ANSI music editor] Replace the whole music text. Each line is one music string; only music commands are allowed.",
            json!({
                "type": "object",
                "properties": { "text": { "type": "string", "description": "Music commands, e.g. T140 O3 L8 CDEFG4" } },
                "required": ["text"],
            }),
        ),
        (
            "icy_replace_music_events",
            "[ANSI music editor] Replace events start..end (end exclusive, default start: insert) with new events. \
             The text is rewritten from the events.",
            json!({
                "type": "object",
                "properties": {
                    "start": { "type": "integer", "minimum": 0 },
                    "end": { "type": "integer", "minimum": 0 },
                    "events": { "type": "array", "items": event },
                },
                "required": ["start", "events"],
            }),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tools_read_and_edit_the_music() {
        let mut draft = MusicDraft::new("T120 L8 CDE", Introducer::SyncTerm, Some("tune.ams".into()), Some(2));
        let info: Value = serde_json::from_str(&draft.call("icy_music_info", &json!({})).unwrap()).unwrap();
        assert_eq!((info["notes"].as_u64(), info["lowest_note"].as_str()), (Some(3), Some("O4 C")));
        let read: Value = serde_json::from_str(&draft.call("icy_read_music", &json!({ "start": 1, "count": 2 })).unwrap()).unwrap();
        assert_eq!(read["events"][0]["text"], "C");
        assert_eq!(read["events"][1]["start_ms"], 250.0);
        assert!(draft.call("icy_music_api", &json!({})).unwrap().contains("MN ML MS"));

        draft
            .call(
                "icy_replace_music_events",
                &json!({ "start": 2, "end": 3, "events": [
                    { "type": "note", "pitch": "O3 F#", "length": 4, "dotted": true },
                    { "type": "articulation", "value": "staccato" },
                    { "type": "note", "pitch": 50, "length": 8 },
                    { "type": "rest", "length": 8 },
                ]}),
            )
            .unwrap();
        assert_eq!(draft.text, "L8T120C<F#4.MS>DPE");
        assert!(draft.changed());
        assert_eq!(draft.summary().0, 4);

        let error = draft.call("icy_write_music", &json!({ "text": "CDE X" })).unwrap_err();
        assert!(error.contains("\"X\""), "{error}");
        assert_eq!(draft.text, "L8T120C<F#4.MS>DPE", "rejected writes leave the draft alone");
        draft.call("icy_write_music", &json!({ "text": "T90 O3 C D E\nMS G" })).unwrap();
        assert_eq!(draft.diff().added, 2);
        assert!(draft
            .call(
                "icy_replace_music_events",
                &json!({ "start": 0, "events": [{ "type": "note", "pitch": "H4" }] })
            )
            .unwrap_err()
            .contains("events[0]"));
    }

    #[test]
    fn pitches_can_be_names_or_numbers() {
        for (name, pitch) in [("O4 C", 48), ("o3eb", 39), ("C#4", 49), ("O6 B", 83), ("O0 C", 0)] {
            assert_eq!(parse_pitch(&json!(name)), Ok(pitch), "{name}");
        }
        assert_eq!(parse_pitch(&json!(12)), Ok(12));
        assert!(parse_pitch(&json!("O7 C")).is_err());
        assert!(parse_pitch(&json!(84)).is_err());
        assert!(parse_pitch(&json!("C")).is_err());
    }
}
