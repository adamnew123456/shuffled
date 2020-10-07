use random;
use std::convert::TryInto;
use std::fs;
use std::io::prelude::*;
use std::path::Path;
use std::process::Command;
use std::time::SystemTime;

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

/// Creates a new RNG seeded either from /dev/urandom or the system time
pub fn seeded_random() -> random::Default {
    let (upper_seed, lower_seed) = fs::File::open("/dev/urandom")
        .map(|mut urandom| {
            let mut buffer = [0; 16];
            if let Ok(16) = urandom.read(&mut buffer) {
                let upper = u64::from_le_bytes(buffer[..8].try_into().unwrap());
                let lower = u64::from_le_bytes(buffer[8..].try_into().unwrap());
                (upper, lower)
            } else if let Ok(duration) = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) {
                let upper = (duration.as_nanos() >> 64) as u64;
                let lower = duration.as_nanos() as u64;
                (upper, lower)
            } else {
                (12345, 67890)
            }
        }).unwrap_or((12345, 67890));

    random::default().seed([upper_seed, lower_seed])
}
