//! Logger module for configuring and initializing logging functionality
//! Source: [`sparrow`](https://github.com/JeroenGar/sparrow/tree/main)

use crate::config::{EPOCH, LOG_LEVEL_FILTER_DEBUG, LOG_LEVEL_FILTER_RELEASE, OUTPUT_DIR};
use log::{LevelFilter, debug};
use std::fs;
use std::sync::Once;
use std::thread::Thread;
use std::time::Duration;

static INIT: Once = std::sync::Once::new();

/// Initializes the logging system with the specified log level filter
/// This function is safe to call multiple times - it will only initialize once
pub fn init_logger(level_filter: LevelFilter) {
    INIT.call_once(|| {
        // Remove old log file
        let _ = fs::remove_file(format!("{}/log.txt", OUTPUT_DIR));
        fern::Dispatch::new()
            // Perform allocation-free log formatting
            .format(|out, message, record| {
                let handle: Thread = std::thread::current();
                let thread_name: &str = handle.name().unwrap_or("-");

                let duration: Duration = EPOCH.elapsed();
                let sec: u64 = duration.as_secs() % 60;
                let min: u64 = (duration.as_secs() / 60) % 60;
                let hours: u64 = (duration.as_secs() / 60) / 60;

                let prefix: String = format!(
                    "[{}] [{:0>2}:{:0>2}:{:0>2}] <{}>",
                    record.level(),
                    hours,
                    min,
                    sec,
                    thread_name,
                );

                out.finish(format_args!("{:<25}{}", prefix, message))
            })
            // Add blanket level filter - all messages at or above the specified level will be logged
            .level(level_filter)
            .chain(std::io::stdout())
            .chain(fern::log_file(format!("{OUTPUT_DIR}/log.txt")).unwrap())
            .apply()
            .expect("could not initialize logger");
        debug!("[EPOCH]: {}", jiff::Timestamp::now().to_string());
    });
}

pub fn setup_environment() {
    fs::create_dir_all(OUTPUT_DIR).expect("Could not create output directory");

    match cfg!(debug_assertions) {
        true => init_logger(LOG_LEVEL_FILTER_DEBUG),
        false => init_logger(LOG_LEVEL_FILTER_RELEASE),
    }
}
