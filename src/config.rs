use std::io::prelude::*;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use toml::Value;
use url::Url;

/// The basic configuration used by the server, regardless of what modules are running
#[derive(Debug, PartialEq)]
pub struct ServiceConfig {
    /// The directory where the playlist files are stored according to the wildcard *.m3u
    pub playlist_dir: PathBuf,

    /// The location of the Unix IPC socket
    pub ipc_socket: PathBuf,

    /// Whether the watchdog module is currently enabled
    pub watchdog_enabled: bool,

    /// Whether the weather module is currently enabled
    pub weather_enabled: bool,

    /// Whether the clock module is currently enabled
    pub clock_enabled: bool,
}

/// The configuration options available for all of the "special" music entries,
/// currently weather and music
#[derive(Debug, PartialEq)]
pub struct SpecialBaseConfig {
    /// The directory used by the special processes for storing output
    pub working_dir: PathBuf,

    /// How often (in minutes) play through the special entries. Note that this
    /// only gives the time between *this* special entry and the *next* special
    /// entry. The whole list can be cycled through after several multiples of
    /// this interval (depending upon how many special entries are enabled)
    pub interval: u32,
}

/// The configuration options available for the watchdog service
#[derive(Debug, PartialEq)]
pub struct WatchdogConfig {
    /// How often to probe the server to see if the stream is active
    pub interval: u32,

    /// The host and port of the Icecast stream we're monitoring
    pub addr: SocketAddr,

    /// The URL path of the Icecast stream we're monitoring
    pub path: String,

    /// The name of the systemd service which runs the ezstream instance
    /// that we are servicing
    pub service: String,
}

/// The configuration options available for the weather special service
#[derive(Debug, PartialEq)]
pub struct SpecialWeatherConfig {
    /// What region to report the weather on
    pub region: String,

    /// How many hours worth of forecasts to combine into a single report
    pub duration: u32,

    /// How often to check with the weather API, in hours. Note that this
    /// is just a cooldown for cases where the API calls are successful;
    /// when they aren't, we poll once every hour until we get a response
    pub interval: u32,
}

/// The combined server settings stored in the configuration file
#[derive(Debug, PartialEq)]
pub struct Config {
    pub service: ServiceConfig,
    pub special_base: SpecialBaseConfig,
    pub special_weather: SpecialWeatherConfig,
    pub watchdog: WatchdogConfig,
}

/// Utility functions for working with dot-separated paths and type corecions
/// with toml::Value
trait ConfigUtils {
    /// Like get_at_path, but returns an Err if the result does not exist
    fn require_at_path(&self, path: &str) -> Result<&Value, String>;

    /// Fetches a Value through nested tables using a dot-separted path
    fn get_at_path(&self, path: &str) -> Option<&Value>;

    /// Requires that the current Value is an array, reporting an Err with the
    /// given path if not
    fn require_array(&self, path: &str) -> Result<&Vec<Value>, String>;

    /// Requires that the current Value is a table, reporting an Err with the
    /// given path if not
    fn require_table(&self, path: &str) -> Result<&toml::map::Map<String, Value>, String>;

    /// Requires that the current Value is a string, reporting an Err with the
    /// given path if not
    fn require_str(&self, path: &str) -> Result<&str, String>;

    /// Requires that the current Value is an integer, reporting an Err with the
    /// given path if not
    fn require_int(&self, path: &str) -> Result<i64, String>;

    /// Like as_pathbuf, but reports an Err with the given path if the value is
    /// not a string
    fn require_pathbuf(&self, path: &str) -> Result<PathBuf, String>;

    /// Converts the current Value to a PathBuf, if it is a string
    fn as_pathbuf(&self) -> Option<PathBuf>;
}

impl ConfigUtils for Value {
    fn require_at_path(&self, path: &str) -> Result<&Value, String> {
        self.get_at_path(path)
            .ok_or(format!("Could not parse config: '{}' is required", path))
    }

    fn get_at_path(&self, path: &str) -> Option<&Value> {
        let mut element = self;
        for node in path.split('.') {
            element = element.as_table().and_then(|table| table.get(node))?;
        }

        Some(element)
    }

    fn require_array(&self, path: &str) -> Result<&Vec<Value>, String> {
        self.as_array().ok_or(format!(
            "Could not parse config: '{}' must be an array",
            path
        ))
    }

    fn require_table(&self, path: &str) -> Result<&toml::map::Map<String, Value>, String> {
        self.as_table().ok_or(format!(
            "Could not parse config: '{}' must be a table",
            path
        ))
    }

    fn require_str(&self, path: &str) -> Result<&str, String> {
        self.as_str().ok_or(format!(
            "Could not parse config: '{}' must be a string",
            path
        ))
    }

    fn require_int(&self, path: &str) -> Result<i64, String> {
        self.as_integer().ok_or(format!(
            "Could not parse config: '{}' must be an integer",
            path
        ))
    }

    fn require_pathbuf(&self, path: &str) -> Result<PathBuf, String> {
        self.as_pathbuf().ok_or(format!(
            "Could not parse config: '{}' must be a file path",
            path
        ))
    }

    fn as_pathbuf(&self) -> Option<PathBuf> {
        self.as_str().map(PathBuf::from)
    }
}

/// Builds the service section of the configuration, which contains the
/// following options:
///
/// - playlist_dir, which is the directory containing the .m3u playlist files
///
/// - ipc_socket, which is a path where shuffled will a Unix domain socket used
///   for sending IPC requests
///
/// - tasks, which is an array of the services (watchdog/weather/clock) run by
///   shuffled
fn parse_service_section(root: &Value) -> Result<ServiceConfig, String> {
    let playlist_dir = root
        .require_at_path("service.playlist_dir")
        .and_then(|p| p.require_pathbuf("service.playlist_dir"))?;

    let ipc_socket = root
        .require_at_path("service.ipc_socket")
        .and_then(|p| p.require_pathbuf("service.ipc_socket"))?;

    let tasks = root
        .require_at_path("service.tasks")
        .and_then(|p| p.require_array("service.tasks"))?;

    let mut watchdog_enabled = false;
    let mut weather_enabled = false;
    let mut clock_enabled = false;

    for task in tasks {
        let task_name = task.require_str("service.tasks.*")?;

        match task_name {
            "watchdog" => watchdog_enabled = true,
            "weather" => weather_enabled = true,
            "clock" => clock_enabled = true,
            _ => {
                return Err(format!(
                    "Could not parse config: '{}' not valid task",
                    task_name
                ))
            }
        }
    }

    Ok(ServiceConfig {
        playlist_dir,
        ipc_socket,
        watchdog_enabled,
        weather_enabled,
        clock_enabled,
    })
}

/// Builds the special service section of the configuration, which contains the
/// following options:
///
/// - working_dir: Reports the path used by the weather and clock processes for
///   generating audio (default /tmp)
///
/// - interval_min: How many minutes to wait between playing the weather/clock
///   files (default 30)
fn parse_special_base(root: &Value) -> Result<SpecialBaseConfig, String> {
    match root.get_at_path("special") {
        Some(special) => special.require_table("special")?,
        None => {
            return Ok(SpecialBaseConfig {
                working_dir: PathBuf::from("/tmp"),
                interval: 30,
            })
        }
    };

    let working_dir = if let Some(entry) = root.get_at_path("special.working_dir") {
        entry.require_pathbuf("special.working_dir")?
    } else {
        PathBuf::from("/tmp")
    };

    let interval = if let Some(entry) = root.get_at_path("special.interval_min") {
        entry.require_int("special.interval_min").and_then(|i| {
            if i > 0 && i < (u32::MAX as i64) {
                Ok(i as u32)
            } else {
                Err("Could not parse config: 'special.interval_min' must be positive".to_string())
            }
        })?
    } else {
        30
    };

    Ok(SpecialBaseConfig {
        working_dir,
        interval,
    })
}

/// Builds the watchdog service section of the configuration, which contains the
/// following options:
///
/// - interval_min: How many minutes to wait between probes to the Icecast server
///   (default 5)
///
/// - service: The name of the systemd service to restart if the Icecast server
///   stops responding (required if this service is enabled)
///
/// - url: The URL where the stream is mounted on the Icecast server, this is
///   is probed every interval
fn parse_watchdog(root: &Value) -> Result<WatchdogConfig, String> {
    let interval = if let Some(entry) = root.get_at_path("watchdog.interval_min") {
        entry.require_int("watchdog.interval_min").and_then(|i| {
            if i > 0 && i < (u32::MAX as i64) {
                Ok(i as u32)
            } else {
                Err("Could not parse config: 'watchdog.interval_min' must be positive".to_string())
            }
        })?
    } else {
        5
    };

    let service = root
        .require_at_path("watchdog.service")
        .and_then(|p| p.require_str("watchdog.service"))?;

    let url = root
        .require_at_path("watchdog.url")
        .and_then(|u| u.require_str("watchdog.url"))?;

    let stream_endpoint = Url::parse(url).or(Err(
        "Could not parse config: 'watchdog.url' was not a valid URL".to_string(),
    ))?;

    if stream_endpoint.scheme() != "http" {
        return Err(
            "Could not parse config: 'watchdog.url' must refer to an HTTP endpoint".to_string(),
        );
    }

    let addr = stream_endpoint.socket_addrs(|| Some(80)).or_else(|_| {
        Err("Could not parse config: 'watchdog.url' could not be resolved".to_string())
    })?;

    if addr.len() == 0 {
        return Err(
            "Could not parse config: 'watchdog.url' did not resolve to any addresses".to_string(),
        );
    }

    Ok(WatchdogConfig {
        interval,
        service: service.to_string(),
        addr: addr[0],
        path: stream_endpoint.path().to_string(),
    })
}

/// Builds the weather service section of the configuration, which contains the
/// following options:
///
/// - region: The weather.gov grid ID and coordinates of the region to request
///   a forecast for (default RAH/57,62)
///
/// - duration_hr: How many hours to create a forecast summary for on each run
///   (default 12)
///
/// - interval_hr: How many hours to wait between fetching a forecast. Note that
///   this only controls the delay after a successful request; failed requests
///   trigger a retry after every hour until a success (default 8)
fn parse_weather(root: &Value) -> Result<SpecialWeatherConfig, String> {
    let region = if let Some(region) = root.get_at_path("weather.region") {
        region.require_str("weather.region")?
    } else {
        "RAH/57,62"
    };

    let duration = if let Some(duration) = root.get_at_path("weather.duration_hr") {
        duration.require_int("weather.duration_hr").and_then(|d| {
            if d > 0 && d < (u32::MAX as i64) {
                Ok(d as u32)
            } else {
                Err("Could not parse config: 'watchdog.duration_hr' must be positive".to_string())
            }
        })?
    } else {
        12
    };

    let interval = if let Some(interval) = root.get_at_path("weather.interval_hr") {
        interval.require_int("weather.interval_hr").and_then(|i| {
            if i > 0 && i < (u32::MAX as i64) {
                Ok(i as u32)
            } else {
                Err("Could not parse config: 'watchdog.interval_hr' must be positive".to_string())
            }
        })?
    } else {
        8
    };

    Ok(SpecialWeatherConfig {
        region: region.to_string(),
        duration,
        interval,
    })
}

pub fn parse(stream: &mut impl Read) -> Result<Config, String> {
    let mut buffer = Vec::new();
    if let Err(reason) = stream.read_to_end(&mut buffer) {
        return Err(format!("Could not read config: {}", reason));
    }

    let content = String::from_utf8(buffer)
        .or_else(|error| Err(format!("Could not load config: {}", error)))?;

    let table = &content
        .parse::<Value>()
        .or_else(|error| Err(format!("Could not parse config: {}", error)))?;

    let service = parse_service_section(&table)?;
    let special_base = parse_special_base(&table)?;

    let watchdog = if service.watchdog_enabled {
        parse_watchdog(&table)?
    } else {
        WatchdogConfig {
            interval: 0,
            service: "".to_string(),
            addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 80),
            path: "/".to_string(),
        }
    };

    let special_weather = parse_weather(&table)?;

    Ok(Config {
        service,
        special_base,
        special_weather,
        watchdog,
    })
}
