use crate::config::{ServiceConfig, SpecialBaseConfig};
use crate::utils;
use chrono::{Local, Timelike};
use json;
use random;
use std::borrow::Cow;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::io::prelude::*;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::str;
use std::time::{Duration, SystemTime};

/// The commands that can be received from RPC, in addition to some error cases
/// that can be reported when the commands are parsed
#[derive(Debug, PartialEq)]
enum RpcRequest {
    NextTrack,
    ListPlaylists,
    GetPlaylist,
    SwitchPlaylist(String),
    ReloadPlaylists,
    InvalidRequest,
    UnknownCommand,
    InvalidParameter,
}

/// The responses that can be sent back over RPC
#[derive(Debug, PartialEq)]
enum RpcResponse<'a> {
    Ok,
    Track(PathBuf),
    Playlists(Vec<&'a String>),
    Playlist(&'a str),
    NoSuchPlaylist,
    NoPlaylistsAvailable,
    InvalidRequest,
    UnknownCommand,
    InvalidParameter,
}

/// A single playlists and its current position
#[derive(Debug)]
struct Playlist {
    position: usize,
    songs: Vec<PathBuf>,
}

impl Playlist {
    /// Creates a new playlist that has its current position set to the first
    /// song
    fn new(songs: Vec<PathBuf>) -> Option<Playlist> {
        if songs.len() == 0 {
            None
        } else {
            Some(Playlist { position: 0, songs })
        }
    }

    /// Returns the playlist's current song
    fn current(&self) -> &PathBuf {
        &self.songs[self.position]
    }

    /// Advances the current song to the next song
    fn next(&mut self) {
        self.position = (self.position + 1) % self.songs.len();
    }

    /// Computes a delta between this playlist and another set of songs
    fn diff_playlist(&self, playlist: &Vec<PathBuf>) -> (Vec<PathBuf>, Vec<PathBuf>) {
        let mut to_add = Vec::new();
        let mut to_remove = Vec::new();

        for song in self.songs.iter() {
            if !playlist.contains(song) {
                to_remove.push(song.to_path_buf());
            }
        }

        for playlist_song in playlist.iter() {
            if !self.songs.contains(playlist_song) {
                to_add.push(playlist_song.to_path_buf());
            }
        }

        (to_add, to_remove)
    }

    /// Adds and removes songs from the given delta lists, putting all the songs
    /// in the add list at the end
    fn merge_songs(&mut self, to_add: &[PathBuf], to_remove: &[PathBuf]) {
        let mut to_remove_indices = Vec::new();
        for (idx, path) in self.songs.iter().enumerate() {
            if to_remove.contains(path) {
                to_remove_indices.push(idx);
            }
        }

        let mut offset = 0;
        for idx in to_remove_indices.iter() {
            self.songs.remove(idx + offset);

            // Try to keep the current song at the current position, so that we
            // don't miss playing any songs. We don't care about songs after the
            // current position but we do care about those before, since removing
            // those will shift the playlist back and cause repeats.
            //
            // A B C D E
            //     ^
            //
            // B C D E   A being removed should shift us back
            //   ^
            //
            // A B C D   E being remove doesn't matter
            //     ^
            //
            // A B D     Removing the current should do nothing, since we look at
            //     ^     current entry (now D, the old next) first and then advance it
            if *idx < self.position {
                self.position -= 1
            }

            offset -= 1;
        }

        self.songs.extend_from_slice(&to_add);

        if self.position > self.songs.len() {
            self.position = 0
        }
    }
}

/// A group of named playlists and their current positions
type Playlists = HashMap<String, Playlist>;

/// A group of named playlists without any position information
type SimplePlaylists = HashMap<String, Vec<PathBuf>>;

/// An entry in the special playlist, which either reports an existing file or
/// generates one
#[derive(Debug)]
enum SpecialQueueEntry {
    TimeGenerator,
    File(PathBuf),
}

/// The path of the clock MP3 file within the special working directory
const CLOCK_MP3_FILE: &str = "clock-stereo.mp3";

/// The playlist and timing for the special weather/time report queue
#[derive(Debug)]
struct SpecialQueue {
    entries: Vec<SpecialQueueEntry>,
    position: usize,
    working_dir: PathBuf,
    last_play_time: SystemTime,
    interval: Duration,
}

impl SpecialQueue {
    /// Checks whether enough time has elapsed since the previous play of a
    /// special entry item
    fn is_special_pending(&self) -> bool {
        if self.entries.len() == 0 {
            return false;
        }

        let since_last_time =
            if let Ok(delta) = SystemTime::now().duration_since(self.last_play_time) {
                delta
            } else {
                return false;
            };

        return since_last_time >= self.interval;
    }

    /// Updates the timer once a special item has been queued
    fn update_timer(&mut self) {
        self.last_play_time = SystemTime::now()
    }

    /// Returns the path to the current special entry
    fn current(&self) -> Option<PathBuf> {
        if self.entries.len() == 0 {
            return None;
        }

        match &self.entries[self.position] {
            SpecialQueueEntry::TimeGenerator => {
                let paths = utils::FileOutputs {
                    mono_wav: &self.working_dir.join("clock-mono.wav"),
                    stereo_wav: &self.working_dir.join("clock-stereo.wav"),
                    lame_mp3: &self.working_dir.join(CLOCK_MP3_FILE),
                };

                let current_time = Local::now();
                let announcement = format!(
                    "The current time is {:02} {:02} hours. Repeat, the current time is {:02} {:02} hours",
                    current_time.hour(),
                    current_time.minute(),
                    current_time.hour(),
                    current_time.minute()
                );

                if let Err(error) = utils::read_text_announcement(&announcement, &paths, "Clock") {
                    eprintln!("[service] {}", error);
                    None
                } else {
                    Some(paths.lame_mp3.to_path_buf())
                }
            }

            SpecialQueueEntry::File(path) => Some(path.clone()),
        }
    }

    fn next(&mut self) {
        self.position = (self.position + 1) % self.entries.len();
    }
}

/// The current playlist and song as well as all registered playlists
#[derive(Debug)]
struct PlaylistQueue {
    current_playlist: String,
    playlists: Playlists,
    directory: PathBuf,
}

impl PlaylistQueue {
    /// Combines a basic playlist with this one, making sure to preserve the
    /// order and position of the current playlist as much as possible
    fn merge_with(&mut self, playlists: &mut SimplePlaylists) {
        if playlists.len() == 0 {
            return;
        }

        let time = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .unwrap_or(12345);

        let mut rng = random::default().seed([time, 0]);

        for (disk_playlist, disk_songs) in playlists.iter_mut() {
            if disk_songs.len() == 0 {
                continue;
            }

            match self.playlists.get_mut(disk_playlist) {
                Some(our_playlist) => {
                    let (mut to_add, to_remove) = our_playlist.diff_playlist(disk_songs);
                    shuffle(&mut to_add, &mut rng);
                    our_playlist.merge_songs(&to_add, &to_remove);
                }

                None => {
                    let mut shuffled_songs = disk_songs.clone();
                    shuffle(&mut shuffled_songs, &mut rng);
                    self.playlists.insert(
                        disk_playlist.to_string(),
                        Playlist::new(shuffled_songs).unwrap(),
                    );
                }
            }
        }

        let to_remove_playlists = {
            self.playlists
                .keys()
                .filter(|playlist| {
                    !playlists.contains_key(*playlist) || self.playlists[*playlist].songs.len() == 0
                })
                .map(|playlist| playlist.to_string())
                .collect::<Vec<_>>()
        };

        for playlist in to_remove_playlists.iter() {
            self.playlists.remove(playlist);
        }

        if !self.playlists.contains_key(&self.current_playlist) {
            self.current_playlist = self.playlists.keys().next().unwrap().to_string();
        }
    }
}

/// Shuffles a vector using the given RNG source
fn shuffle<T>(vec: &mut Vec<T>, rng: &mut impl random::Source) {
    vec.sort_unstable_by_key(|_| rng.read_u64());
}

/// Reads an M3U8 file and returns a list of absolute paths to the audio files
/// listed within, or an error if the playlist or files are invalid
fn parse_m3u8_playlist(filename: &Path) -> Result<Vec<PathBuf>, String> {
    let buffer = fs::read(filename).or_else(|error| {
        Err(format!(
            "Could not read playlist {}: {}",
            filename.display(),
            error
        ))
    })?;

    let contents = String::from_utf8(buffer)
        .or_else(|error| Err(format!("Could not decode playlist: {}", error)))?;

    // m3u files that use relative paths are relative to the location of the file itself
    let playlist_relative = filename.parent().and_then(|dir| match dir.canonicalize() {
        Ok(parent) => Some(parent),
        Err(_) => None,
    });

    let mut playlist = Vec::new();
    for line in contents.split('\n') {
        let processed_line = line.trim();
        if processed_line == "" {
            continue;
        }

        let path = PathBuf::from(line.trim());
        let path = if !path.is_absolute() {
            match playlist_relative.as_ref() {
                Some(parent) => parent.to_path_buf().join(path),
                None => {
                    return Err(format!(
                        "Could not read playlist: failed to resolve relative entry {}",
                        path.display()
                    ))
                }
            }
        } else {
            path
        };

        if !path.is_file() {
            return Err(format!(
                "Could not read playlist: entry {} is not a file",
                path.display()
            ));
        }

        if playlist.contains(&path) {
            return Err(format!(
                "Could not read playlist: entry {} is a duplicate",
                path.display()
            ));
        }

        playlist.push(path);
    }

    if playlist.len() == 0 {
        return Err(format!(
            "Could not read playlist: no entries in {}",
            filename.display()
        ));
    }

    Ok(playlist)
}

/// Reads all the .m3u8 playlists available in the given directory
fn read_m3u8_files(directory: &Path) -> Result<SimplePlaylists, String> {
    let reader = directory
        .read_dir()
        .or_else(|error| Err(format!("Error reading playlist directory: {}", error)))?;

    let mut raw_playlists: HashMap<String, Vec<PathBuf>> = HashMap::new();
    for entry in reader {
        let dirent =
            entry.or_else(|error| Err(format!("Error reading playlist directory: {}", error)))?;

        let entry_path = dirent.path();
        if !entry_path.is_file() {
            continue;
        }

        let extension = entry_path.extension().map(OsStr::to_string_lossy);
        if extension != Some(Cow::Borrowed("m3u8")) {
            continue;
        }

        let name = entry_path
            .file_stem()
            .map(OsStr::to_string_lossy)
            .ok_or(format!(
                "Error reading playlist file {}: unreadable name",
                entry_path.display()
            ))?;

        let playlist = parse_m3u8_playlist(&entry_path).or_else(|error| Err(error.to_string()))?;

        raw_playlists.insert(name.to_string(), playlist);
    }

    if raw_playlists.len() == 0 {
        return Err("Error reading playlist directory: no playlists".to_string());
    }

    Ok(raw_playlists)
}

/// Attempts to parse a single command out of the buffer, either failing if the
/// buffer doesn't contain a complete command or succeeding and returning the
/// message and the next message's starting position
fn try_parse_request(buffer: &[u8]) -> Option<(RpcRequest, usize)> {
    let first_newline = buffer.iter().position(|byte| *byte == 10)?;
    let default_value = Some((RpcRequest::InvalidRequest, first_newline + 1));

    let first_line = if let Ok(line) = str::from_utf8(&buffer[..first_newline]) {
        line
    } else {
        return default_value;
    };

    let document = if let Ok(doc) = json::parse(first_line) {
        doc
    } else {
        return default_value;
    };

    if !document.is_object() || !document.has_key("command") {
        return default_value;
    }

    let command = if let Some(command) = document["command"].as_str() {
        command
    } else {
        return default_value;
    };

    match command {
        "next-track" => Some((RpcRequest::NextTrack, first_newline + 1)),
        "list-playlists" => Some((RpcRequest::ListPlaylists, first_newline + 1)),
        "get-playlist" => Some((RpcRequest::GetPlaylist, first_newline + 1)),
        "reload-playlists" => Some((RpcRequest::ReloadPlaylists, first_newline + 1)),
        "switch-playlist" => {
            if !document.has_key("playlist") {
                Some((RpcRequest::InvalidParameter, first_newline + 1))
            } else {
                match document["playlist"].as_str() {
                    Some(playlist) => Some((
                        RpcRequest::SwitchPlaylist(playlist.to_string()),
                        first_newline + 1,
                    )),
                    None => Some((RpcRequest::InvalidParameter, first_newline + 1)),
                }
            }
        }
        _ => Some((RpcRequest::UnknownCommand, first_newline + 1)),
    }
}

/// Serializes and sends a single RPC response
fn send_response(stream: &mut impl Write, response: RpcResponse) -> io::Result<()> {
    match response {
        RpcResponse::Ok => stream.write_all("{\"status\": \"ok\"}\n".as_bytes()),
        RpcResponse::Track(path) => {
            let path_raw = path.to_string_lossy().to_string();
            let encoded = json::stringify(json::JsonValue::String(path_raw));
            stream.write_all("{\"track\":".as_bytes())?;
            stream.write_all(encoded.as_bytes())?;
            stream.write_all("}\n".as_bytes())
        }
        RpcResponse::Playlists(mut playlists) => {
            let values = playlists
                .drain(..)
                .map(|playlist| json::JsonValue::String(playlist.to_string()))
                .collect::<Vec<_>>();

            let encoded = json::stringify(json::JsonValue::Array(values));
            stream.write_all("{\"playlists\":".as_bytes())?;
            stream.write_all(encoded.as_bytes())?;
            stream.write_all("}\n".as_bytes())
        }
        RpcResponse::Playlist(playlist) => {
            let encoded = json::stringify(json::JsonValue::String(playlist.to_string()));
            stream.write_all("{\"playlist\":".as_bytes())?;
            stream.write_all(encoded.as_bytes())?;
            stream.write_all("}\n".as_bytes())
        }
        RpcResponse::NoSuchPlaylist => {
            stream.write_all("{\"status\": \"no-such-playlist\"}\n".as_bytes())
        }
        RpcResponse::NoPlaylistsAvailable => {
            stream.write_all("{\"status\": \"no-playlists-available\"}\n".as_bytes())
        }
        RpcResponse::InvalidRequest => {
            stream.write_all("{\"status\": \"invalid-request\"}\n".as_bytes())
        }
        RpcResponse::UnknownCommand => {
            stream.write_all("{\"status\": \"unknown-command\"}\n".as_bytes())
        }
        RpcResponse::InvalidParameter => {
            stream.write_all("{\"status\": \"invalid-parameter\"}\n".as_bytes())
        }
    }
}

/// Checks that the paths used for the IPC and playlist options are actually valid
fn validate_configuration(service_config: &ServiceConfig) -> Result<(), String> {
    if !service_config.playlist_dir.is_absolute() {
        return Err("Playlist path must be absolute path".to_string());
    }

    if !service_config.playlist_dir.is_dir() {
        return Err("Playlist path is not a directory".to_string());
    }

    let ipc_socket = service_config.ipc_socket.as_path();
    if !ipc_socket.is_absolute() {
        return Err("IPC path must be absolute path".to_string());
    }

    if ipc_socket.is_dir() {
        return Err("IPC path cannot be a directory".to_string());
    }

    if ipc_socket.exists() {
        return Err("IPC path already exists, is this server already running?".to_string());
    }

    match ipc_socket.parent() {
        Some(parent) if !parent.is_dir() => return Err("IPC path does not exist".to_string()),

        // Should be caught by is_dir(), the only thing that would return None
        // parent is the root directory
        None => return Err("IPC path cannot be a directory".to_string()),
        _ => (),
    }

    Ok(())
}

/// Updates the state of the playlist queue according to the given request
fn process_request<'a>(
    rpc: RpcRequest,
    queue: &'a mut PlaylistQueue,
    special_queue: &mut SpecialQueue,
) -> RpcResponse<'a> {
    match rpc {
        RpcRequest::NextTrack => {
            if special_queue.is_special_pending() {
                if let Some(special) = special_queue.current() {
                    if special.is_file() {
                        special_queue.next();
                        special_queue.update_timer();
                        return RpcResponse::Track(special);
                    }
                }
            }

            let current_playlist = queue.playlists.get_mut(&queue.current_playlist).unwrap();
            let song = current_playlist.current().to_path_buf();
            current_playlist.next();
            RpcResponse::Track(song)
        }

        RpcRequest::ListPlaylists => {
            let playlists = queue.playlists.keys().collect::<Vec<_>>();
            RpcResponse::Playlists(playlists)
        }

        RpcRequest::GetPlaylist => RpcResponse::Playlist(&queue.current_playlist),

        RpcRequest::SwitchPlaylist(target) => {
            if queue.playlists.contains_key(&target) {
                queue.current_playlist = target;
                RpcResponse::Ok
            } else {
                RpcResponse::NoSuchPlaylist
            }
        }

        RpcRequest::ReloadPlaylists => {
            let mut raw_playlists = match read_m3u8_files(queue.directory.as_ref()) {
                Ok(playlists) => playlists,
                Err(error) => {
                    eprintln!("[server] {}", error);
                    return RpcResponse::NoPlaylistsAvailable;
                }
            };

            queue.merge_with(&mut raw_playlists);
            RpcResponse::Ok
        }

        RpcRequest::InvalidRequest => RpcResponse::InvalidRequest,
        RpcRequest::UnknownCommand => RpcResponse::UnknownCommand,
        RpcRequest::InvalidParameter => RpcResponse::InvalidParameter,
    }
}

/// Reads and executes commands, and sends responses, on a single connection
/// until that connection is terminated
fn process_connection(
    mut client: UnixStream,
    queue: &mut PlaylistQueue,
    special_queue: &mut SpecialQueue,
) {
    if let Err(error) = client.set_read_timeout(Some(Duration::from_secs(5))) {
        eprintln!("[server] Warning, could not set socket timeout: {}", error);
    };

    if let Err(error) = client.set_write_timeout(Some(Duration::from_secs(5))) {
        eprintln!("[server] Warning, could not set socket timeout: {}", error);
    };

    let mut command_buffer = Vec::new();
    let mut read_buffer = [0; 4096];

    loop {
        let size = match client.read(&mut read_buffer) {
            Ok(0) => break,
            Ok(size) => size,
            Err(error) => {
                eprintln!("[server] Lost connection to client: {}", error);
                break;
            }
        };

        command_buffer.extend_from_slice(&read_buffer[..size]);
        match try_parse_request(&command_buffer) {
            Some((rpc, offset)) => {
                command_buffer.drain(..offset);
                let response = process_request(rpc, queue, special_queue);
                match send_response(&mut client, response) {
                    Ok(()) => (),
                    Err(error) => {
                        eprintln!("[server] Could not reply to client: {}", error);
                        break;
                    }
                }
            }
            None => {
                if command_buffer.len() > 4096 {
                    eprintln!("[server] Client buffer too large, dropping");
                    break;
                }
            }
        }
    }
}

/// Processes incoming IPC requests and maintains the set of current playlists
pub fn server_worker(service_config: ServiceConfig, special_config: SpecialBaseConfig) {
    if let Err(message) = validate_configuration(&service_config) {
        eprintln!("[server] {}", message);
        return;
    }

    let server = match UnixListener::bind(service_config.ipc_socket) {
        Ok(server) => server,
        Err(error) => {
            eprintln!("[server] Could not bind IPC socket: {}", error);
            eprintln!("[server] Terminating");
            return;
        }
    };

    let init_playlists = match read_m3u8_files(&service_config.playlist_dir) {
        Ok(mut playlists) => playlists
            .drain()
            .map(|(playlist, paths)| (playlist, Playlist::new(paths).unwrap()))
            .collect::<HashMap<String, Playlist>>(),
        Err(error) => {
            eprintln!("[server] {}", error);
            eprintln!("[server] Terminating");
            return;
        }
    };

    let mut queue = PlaylistQueue {
        current_playlist: init_playlists.keys().next().unwrap().to_string(),
        playlists: init_playlists,
        directory: service_config.playlist_dir,
    };

    let mut special_entries = Vec::new();
    if service_config.clock_enabled {
        special_entries.push(SpecialQueueEntry::TimeGenerator);
    }

    if service_config.weather_enabled {
        special_entries.push(SpecialQueueEntry::File(
            special_config
                .working_dir
                .join(crate::weather::WEATHER_MP3_FILE)
                .to_path_buf(),
        ));
    }

    let mut special_queue = SpecialQueue {
        entries: special_entries,
        position: 0,
        working_dir: special_config.working_dir,
        last_play_time: SystemTime::now(),
        interval: Duration::from_secs(special_config.interval as u64 * 60),
    };

    for stream in server.incoming() {
        match stream {
            Ok(client) => process_connection(client, &mut queue, &mut special_queue),
            Err(error) => eprintln!("[server] Lost client: {}", error),
        }
    }
}
