use crate::config::SpecialWeatherConfig;
use crate::utils;
use chrono::{DateTime, Local, Timelike};
use json::JsonValue;
use reqwest::blocking::Client;
use reqwest::header::{ACCEPT, USER_AGENT};
use std::fmt::Write;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

/// A textual forecast that applies to a specific region of time
#[derive(Debug, PartialEq)]
struct Forecast {
    /// The start of the time when the forecast applies
    start_time: DateTime<Local>,

    /// The end of the time when the forecast applies
    end_time: DateTime<Local>,

    /// A textual description of the forecast
    description: String,
}

/// Utility functions used for coercing JSON values to their complex types
trait JsonValueExt {
    /// Returns the object underlying this value, or None if it isn't an object
    fn as_object(&self) -> Option<&json::object::Object>;

    /// Returns the array underlying this value, or None if it isn't an array
    fn as_array(&self) -> Option<&Vec<JsonValue>>;
}

impl JsonValueExt for JsonValue {
    fn as_object(&self) -> Option<&json::object::Object> {
        match self {
            JsonValue::Object(object) => Some(object),
            _ => None,
        }
    }

    fn as_array(&self) -> Option<&Vec<JsonValue>> {
        match self {
            JsonValue::Array(array) => Some(array),
            _ => None,
        }
    }
}

/// Parses a JSON document representing a Forecast into a full Forecast value
fn parse_forecast(obj: &json::object::Object) -> Result<Forecast, ()> {
    let description = obj
        .get("detailedForecast")
        .and_then(|val| val.as_str())
        .ok_or_else(|| {
            eprintln!("[weather] Could not read /properties/periods/*/detailedForecast");
            ()
        })?;

    let start_time = obj
        .get("startTime")
        .and_then(|val| val.as_str())
        .ok_or_else(|| {
            eprintln!("[weather] Could not read /properties/periods/*/startTime");
            ()
        })
        .and_then(|txt| {
            DateTime::parse_from_str(txt, "%Y-%m-%dT%H:%M:%S%:z").or_else(|_| {
                eprintln!("[weather] Could not parse /properties/periods/*/startTime");
                Err(())
            })
        })?;

    let end_time = obj
        .get("endTime")
        .and_then(|val| val.as_str())
        .ok_or_else(|| {
            eprintln!("[weather] Could not read /properties/periods/*/endTime");
            ()
        })
        .and_then(|txt| {
            DateTime::parse_from_str(txt, "%Y-%m-%dT%H:%M:%S%:z").or_else(|_| {
                eprintln!("[weather] Could not parse /properties/periods/*/endTime");
                Err(())
            })
        })?;

    Ok(Forecast {
        description: description.to_string(),
        start_time: start_time.with_timezone(&Local),
        end_time: end_time.with_timezone(&Local),
    })
}

/// Fetches the current forecast from the weather.gov API and unpacks the
/// resulting JSON into a series of Forecast entries containing the forecast
/// strings and the time slots they apply to
fn fetch_forecasts(url: &str) -> Result<Vec<Forecast>, ()> {
    let client = Client::new();
    let response = client
        .get(url)
        .header(ACCEPT, "application/geo+json")
        .header(USER_AGENT, "shuffled Weather Fetcher")
        .send()
        .or_else(|error| {
            eprintln!("[weather] Could not fetch forecast: {}", error);
            Err(())
        })?;

    let status = response.status();
    if !(200..300).contains(&status.as_u16()) {
        eprintln!(
            "[weather] API returned unexpected status code {}",
            status.as_u16()
        );
        return Err(());
    }

    let entity = response.text().or_else(|error| {
        eprintln!("[weather] Could not decode API response: {}", error);
        return Err(());
    })?;

    let document = json::parse(&entity).or_else(|error| {
        eprintln!("[weather] Could not parse API response: {}", error);
        return Err(());
    })?;

    let raw_periods = document
        .as_object()
        .and_then(|obj| obj.get("properties"))
        .and_then(|val| val.as_object())
        .and_then(|obj| obj.get("periods"))
        .and_then(|val| val.as_array())
        .ok_or_else(|| {
            eprintln!("[weather] Could not read /properties/periods");
            ()
        })?;

    let mut periods = raw_periods
        .iter()
        .map(|raw| {
            let obj = raw.as_object().ok_or_else(|| {
                eprintln!("[weather] Could not read /properties/periods/*");
                ()
            })?;

            parse_forecast(obj)
        })
        .collect::<Vec<_>>();

    for (i, period) in periods.iter().enumerate() {
        if period.is_err() {
            eprintln!("[weather] Parsing error occurred in entry {}", i);
            return Err(());
        }
    }

    Ok(periods
        .drain(..)
        .map(|period| period.unwrap())
        .collect::<Vec<_>>())
}

/// Generates a single weather string from a slice of a complete forecast.
fn generate_weather_string(
    forecasts: &Vec<Forecast>,
    start_time: DateTime<Local>,
    end_time: DateTime<Local>,
) -> String {
    let mut buffer = String::new();

    let range_forecasts = forecasts.iter().filter(|forecast| {
        (forecast.start_time >= start_time && forecast.start_time < end_time)
            || (forecast.start_time < start_time && forecast.end_time >= start_time)
    });

    for forecast in range_forecasts {
        write!(
            &mut buffer,
            "At {:02}, {} ",
            forecast.start_time.hour(),
            &forecast.description
        )
        .unwrap();
    }

    buffer
}

/// The path of the weather MP3 file within the special working directory
pub const WEATHER_MP3_FILE: &str = "weather-stereo.mp3";

/// Perdiodically queries the Weather.gov API and produces an audio summary of
/// the forecast which can be played in the stream
pub fn weather_worker(working_dir: PathBuf, config: SpecialWeatherConfig) {
    let url = format!(
        "https://api.weather.gov/gridpoints/{}/forecast",
        config.region
    );

    let temp_files = utils::FileOutputs {
        mono_wav: &working_dir.join("weather-mono.wav"),
        stereo_wav: &working_dir.join("weather-stereo.wav"),
        lame_mp3: &working_dir.join(WEATHER_MP3_FILE),
    };

    let wait_interval = Duration::from_secs(60 * 60);
    let mut sleep_intervals = if temp_files.lame_mp3.is_file() { 1 } else { 0 };

    loop {
        if sleep_intervals > 0 {
            thread::sleep(wait_interval);
            sleep_intervals -= 1;
        }

        if sleep_intervals > 0 {
            continue;
        }

        let forecasts = if let Ok(forecasts) = fetch_forecasts(&url) {
            forecasts
        } else {
            sleep_intervals = 1;
            continue;
        };

        let start_time = Local::now();
        let end_time = start_time + chrono::Duration::hours(config.duration as i64);
        let forecast_str = generate_weather_string(&forecasts, start_time, end_time);
        if let Err(error) =
            utils::read_text_announcement(&forecast_str, &temp_files, "Weather Report")
        {
            eprintln!("[weather] {}", error);
            sleep_intervals = 1;
            continue;
        }

        sleep_intervals = config.interval;
    }
}
