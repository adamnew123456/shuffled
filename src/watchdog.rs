use crate::config::WatchdogConfig;
use std::io::{Read, Write};
use std::net;
use std::process::Command;
use std::str;
use std::thread;
use std::time::Duration;

/// Try to connect to the Icecast server and issue an HTTP request. Any
/// condition that prevents retrieving audio data (socket-level or bad HTTP
/// response) returns an Err.
fn probe_icecast(addr: &net::SocketAddr, path: &str, timeout_sec: u32) -> Result<(), ()> {
    let timeout = Duration::from_secs(timeout_sec as u64);

    let mut sock = net::TcpStream::connect_timeout(&addr, timeout).or_else(|error| {
        eprintln!("[watchdog] Could not connect to {}: {}", addr, error);
        Err(())
    })?;

    if let Err(error) = sock.set_read_timeout(Some(timeout)) {
        eprintln!(
            "[watchdog] Warning, could not set socket timeout: {}",
            error
        );
    };

    if let Err(error) = sock.set_write_timeout(Some(timeout)) {
        eprintln!(
            "[watchdog] Warning, could not set socket timeout: {}",
            error
        );
    };

    let request = format!("GET {} HTTP/1.0\r\nUser-Agent: shuffled/0.1\r\n\r\n", path);
    sock.write_all(request.as_bytes()).or_else(|error| {
        eprintln!(
            "[watchdog] Could not send HTTP request to {}@{}: {}",
            path, addr, error
        );
        Err(())
    })?;

    let mut response = [0; 1024];
    let mut offset = 0;
    while offset < response.len() {
        let consumed = sock.read(&mut response[offset..]).or_else(|error| {
            eprintln!(
                "[watchdog] Could not read HTTP response from {}@{}: {}",
                path, addr, error
            );
            Err(())
        })?;

        if consumed == 0 {
            eprintln!(
                "[watchdog] Unexpected EOF when reading HTTP response from {}@{}",
                path, addr
            );
            return Err(());
        }

        let just_received = &response[offset..offset + consumed];
        offset += consumed;
        if let Some(_) = just_received.iter().position(|x| *x == 10) {
            break;
        }
    }

    let status_slice = &response[..offset];
    let status_start = status_slice.iter().position(|x| *x == 32)
        .ok_or_else(|| {
            eprintln!(
                "[watchdog] Could not find first space character in HTTP response to {}@{}",
                path, addr
            );
            ()
        })?;

    let status_end = status_slice[status_start + 1..]
        .iter()
        .position(|x| *x == 32)
        .ok_or_else(|| {
            eprintln!(
                "[watchdog] Could not find second space character in HTTP response to {}@{}",
                path, addr
            );
            ()
        })? + status_start + 1;

    let status = str::from_utf8(&status_slice[status_start + 1..status_end]).or_else(|error| {
        eprintln!(
            "[watchdog] Could not decode HTTP response from {}@{}: {}",
            path, addr, error
        );
        Err(())
    })?;

    match u16::from_str_radix(status, 10) {
        Ok(status) if status >= 200 && status < 300 => Ok(()),
        Ok(status) => {
            eprintln!(
                "[watchdog] {}@{} returned HTTP status {}",
                path, addr, status
            );
            Err(())
        }
        Err(_) => {
            eprintln!(
                "[watchdog] Could not parse HTTP status from {}@{}: {}",
                path, addr, status
            );
            Err(())
        }
    }
}

/// Restarts the ezstream service via systemd
fn restart_ezstream(service: &str) {
    match Command::new("/bin/systemctl")
        .arg("restart")
        .arg(service)
        .spawn()
    {
        Ok(mut child) => {
            if let Err(error) = child.wait() {
                eprintln!("[watchdog] systemctl invocation failed: {}", error);
            }
        }
        _ => (),
    }
}

/// Periodically performs a probe against Icecast and restarts the ezstream
/// service as necessary
pub fn watchdog_worker(config: WatchdogConfig) {
    let interval = Duration::from_secs(config.interval as u64 * 60);

    loop {
        thread::sleep(interval);
        if let Err(_) = probe_icecast(&config.addr, &config.path, 10) {
            restart_ezstream(&config.service);
        }
    }
}
