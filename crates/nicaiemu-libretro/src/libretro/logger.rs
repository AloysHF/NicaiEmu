// libretro logger: bridges the Rust `log` crate to the frontend.

use super::callbacks;
use super::types::retro_log_level;
use log::{Level, LevelFilter, Log, Metadata, Record};

struct LibretroLogger;

impl Log for LibretroLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Info
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        callbacks::log_message(map_level(record.level()), &record.args().to_string());
    }

    fn flush(&self) {}
}

fn map_level(level: Level) -> retro_log_level {
    match level {
        Level::Error => retro_log_level::RETRO_LOG_ERROR,
        Level::Warn => retro_log_level::RETRO_LOG_WARN,
        Level::Info => retro_log_level::RETRO_LOG_INFO,
        Level::Debug | Level::Trace => retro_log_level::RETRO_LOG_DEBUG,
    }
}

static LOGGER: LibretroLogger = LibretroLogger;

/// Forward Rust log records to the frontend log callback.
pub fn init() {
    // A frontend may initialize the core more than once in the same process.
    let _ = log::set_logger(&LOGGER);
    log::set_max_level(LevelFilter::Debug);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_rust_log_levels_to_libretro_levels() {
        assert_eq!(map_level(Level::Error), retro_log_level::RETRO_LOG_ERROR);
        assert_eq!(map_level(Level::Warn), retro_log_level::RETRO_LOG_WARN);
        assert_eq!(map_level(Level::Info), retro_log_level::RETRO_LOG_INFO);
        assert_eq!(map_level(Level::Debug), retro_log_level::RETRO_LOG_DEBUG);
        assert_eq!(map_level(Level::Trace), retro_log_level::RETRO_LOG_DEBUG);
    }

    #[test]
    fn forwards_info_and_above_only() {
        assert!(!LOGGER.enabled(&Metadata::builder().level(Level::Debug).build()));
        assert!(LOGGER.enabled(&Metadata::builder().level(Level::Info).build()));
        assert!(LOGGER.enabled(&Metadata::builder().level(Level::Error).build()));
    }
}
