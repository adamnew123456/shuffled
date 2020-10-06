use std::path::Path;
use std::process::Command;

/// Describes where the separates stages of the reader process should write
/// their data to
#[derive(Debug, PartialEq)]
pub struct FileOutputs<'a> {
    pub mono_wav: &'a Path,
    pub stereo_wav: &'a Path,
    pub lame_mp3: &'a Path,
}

/// Reads a text announcement and outputs an ID3-tagged MP3 file
pub fn read_text_announcement(
    announcement: &str,
    outputs: &FileOutputs,
    title: &str,
) -> Result<(), String> {
    Command::new("/usr/bin/espeak")
        .arg("-g")
        .arg("15")
        .arg("-w")
        .arg(outputs.mono_wav)
        .arg(announcement)
        .output()
        .or_else(|err| Err(format!("Could not invoke espeak: {}", err)))?;

    Command::new("/usr/bin/sox")
        .arg(outputs.mono_wav)
        .arg("-r")
        .arg("44.1k")
        .arg("-c")
        .arg("2")
        .arg(outputs.stereo_wav)
        .output()
        .or_else(|err| Err(format!("Could not invoke sox: {}", err)))?;

    Command::new("/usr/bin/lame")
        .arg(outputs.stereo_wav)
        .arg(outputs.lame_mp3)
        .output()
        .or_else(|err| Err(format!("Could not invoke lame: {}", err)))?;

    Command::new("/usr/bin/eyeD3")
        .arg("-1")
        .arg("-a")
        .arg("shuffled")
        .arg("-t")
        .arg(title)
        .arg(outputs.lame_mp3)
        .output()
        .or_else(|err| Err(format!("Could not invoke eyeD3: {}", err)))?;

    Ok(())
}
