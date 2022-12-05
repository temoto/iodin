//! Simple logger module.
use log::{LevelFilter, Metadata, Record, SetLoggerError};

struct SimpleLogger;

impl log::Log for SimpleLogger {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        eprintln!("{}", record.args());
    }

    fn flush(&self) {}
}

pub fn init(max_level: LevelFilter) -> Result<(), SetLoggerError> {
    log::set_boxed_logger(Box::new(SimpleLogger)).map(|()| log::set_max_level(max_level))
}
