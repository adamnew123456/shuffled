mod config;
mod server;
mod utils;
mod watchdog;
mod weather;

use std::fs;
use std::path::PathBuf;
use std::thread;

fn main() -> Result<(), String> {
    let mut config_path = PathBuf::from("/etc/shuffled.conf");
    for arg in std::env::args().skip(1) {
        config_path = PathBuf::from(arg);
    }

    eprintln!("Loading configuration...");
    let mut config_file = fs::File::open(&config_path).or_else(|error| {
        Err(format!(
            "Could not open configuration at {}: {}",
            config_path.display(),
            error
        ))
    })?;

    let config = config::parse(&mut config_file)?;
    let watchdog_config = config.watchdog;
    let weather_config = config.special_weather;
    let special_working_dir = config.special_base.working_dir.to_path_buf();

    if config.service.watchdog_enabled {
        eprintln!("Spawning watchdog worker...");
        thread::spawn(move || watchdog::watchdog_worker(watchdog_config));
    }

    if config.service.weather_enabled {
        eprintln!("Spawning weather worker...");
        thread::spawn(move || weather::weather_worker(special_working_dir, weather_config));
    }

    eprintln!("Spawning IPC worker...");
    server::server_worker(config.service, config.special_base);

    Ok(())
}
