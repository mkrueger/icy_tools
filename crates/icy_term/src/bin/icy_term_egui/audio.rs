use icy_engine_gui::music::music::SoundThread;
use icy_term::{Options, TerminalEvent};

pub fn dispatch(sound: &mut SoundThread, event: &TerminalEvent, options: &Options) -> Result<(), String> {
    let result = match event {
        TerminalEvent::PlayMusic(music) => sound.play_music(music.clone()),
        TerminalEvent::PlayGist(data) => sound.play_gist(data.clone()),
        TerminalEvent::PlayChipMusic {
            sound_data,
            voice,
            volume,
            pitch,
        } => sound.play_chip_music(sound_data.clone(), *voice, *volume, *pitch),
        TerminalEvent::AudioApc(command, directory) => sound.audio_apc(command.clone(), directory.clone()),
        TerminalEvent::SndOff(voice) => sound.snd_off(*voice),
        TerminalEvent::StopSnd(voice) => sound.stop_snd(*voice),
        TerminalEvent::SndOffAll => sound.snd_off_all(),
        TerminalEvent::StopSndAll => sound.stop_snd_all(),
        TerminalEvent::Beep if options.console_beep => sound.beep(),
        TerminalEvent::OpenLineSound => sound.start_line_sound(options.dial_tone),
        TerminalEvent::OpenDialSound(tone, number) => sound.start_dial_sound(*tone, options.dial_tone, number),
        TerminalEvent::StopSound => sound.stop_line_sound(),
        TerminalEvent::Disconnected(_) => {
            sound.clear();
            return Ok(());
        }
        _ => return Ok(()),
    };
    result.map_err(|error| error.to_string())
}
