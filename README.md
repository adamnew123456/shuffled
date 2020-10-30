# What is this?

**shuffled** is a multi-playlist manager intended to be used with ezstream and
Icecast. ezstream is only natively capable of using a single playlist as a
source, but it can use an external program to determine what audio files to
play next. By configuring ezstream to use shuffled you can switch between
multiple playlists on the fly and hot-reload playlists on demand. It can also
generate audio files internally to provide dynamic time checks and weather
forecasts.

shuffled comes in two parts:

- The daemon, which holds the state of the playlists and performs other tasks
  like generating utility streams.

- The client, which contacts the daemon via Unix domain socket and issues IPC
  requests to control the current playlist or request playlist reloads and
  shuffles.

# How do I build it?

Via cargo:

```
$ cargo build --release
```

# How do I use it?

First, you'll need to configure the shuffled daemon itself. The example
configuration in example.conf describes all the directives that shuffled
supports. 

Once you have a configuration that works you'll need to copy it to
/etc/shuffled.conf. Make sure that there is at least one m3u8 file in the
playlist directory before you start.

Then you can start the daemon. It's recommended that you setup a unit file for
long-term use but running from the console can be useful for testing purposes.
shuffled may output a few diagnostic messages on startup depending upon what
services you enabled in the configuration.

```
$ shuffled
```

Once the daemon has started you can interact with it using the included
shufflectl command. For example:

```
$ shufflectl /tmp/shuffled.sock list-playlists
a
b
c
$ shufflectl /tmp/shuffled.sock get-playlist
b
$ shufflectl /tmp/shuffled.sock next-track
/usr/share/music/b_1.mp3
$ shufflectl /tmp/shuffled.sock set-playlist a
$ shufflectl /tmp/shuffled.sock next-track
/usr/share/music/a_1.mp3
```

You'll want to use the next-track command as part the configuration for ezstream
or other mixer.

# Protocol

If you want to integrate with shuffled without having to through shufflectl
(for example, if you wanted to control it from a webpage or an IRC bot) you'll
need to use the shuffled control protocol. shuffled speaks this protocol over
a stream-based Unix domain socket and accepts one UTF-8 JSON command per line
(defined here as `\n`).

The protocol defines the following commands and generates the accompanying
responses:

- **Generic Responses** These can be returned for any command and indicate basic
  errors like malformed requests or requests that don't include enough
  information:

```
/* The command couldn't be parsed as UTF-8 JSON, was too long, or didn't have a "command" key */
{"status": "invalid-request"}

/* The value in the "command" key wasn't a recognized command */
{"status": "unknown-command"}

/* The request requires some other non-command key that wasn't provided */
{"status": "invalid-parameter"}
```

- **Getting the Next Track** The `next-track` command returns the next entry in
  the current playlist and advances the position in the current playlist.
  
```
/* Request */
{"command": "next-track"}

/* Response */
{"track": "<path to audio file>"}
```

- **List the Available Playlists** The `list-playlists` command returns a list
  of all playlists registered on the server.
  
```
/* Request */
{"command": "list-playlists"}

/* Response */
{"playlists": ["<playlist>", "<playlist>", ...]}
```

- **Get the Current Playlist** The `get-playlist` command returns the name of
  the active playlist. It'll be one of the ones returned by `list-playlists`.
  
```
/* Request */
{"command": "get-playlist"}

/* Response */
{"playlist": "<playlist>"}
```

- **Reload the Playlists from Disk** The `reload-playlists` command loads all
  the playlist files from the disk into shuffled. Internally this performs a merge
  so that, for any given playlist, removed songs are removed and new songs are shuffled
  and added onto the end of the playlist. Playlists which don't exist on disk are removed
  and playlists which are new are shuffled and added.
  
```
/* Request */
{"command": "reload-playlists"}

/* Response */
{"status": "ok"}

/* There weren't any playlist files on disk to load. */
{"status": "no-playlists-available"}
```

- **Shuffle the Playlists** The `shuffle-playlists` command reorders all the
  in-memory playlists and resets the current position within them.
  
```
/* Request */
{"command": "shuffle-playlists"}

/* Response */
{"status": "ok"}
```

- **Switch the Current Playlist** The `switch-playlist` command changes the
  current playlist. The position within the current playlist is preserved so
  that switching back to it later would return the same track as it would return
  now (if it weren't switched).
  
```
/* Request */
{"status": "switch-playlist"}

/* Response */
{"status": "ok"}

/* The named playlist doesn't exist. */
{"status": "no-such-playlist"}
```
