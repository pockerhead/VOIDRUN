use once_cell::sync::Lazy;
use std::sync::Mutex;

// Потокобезопасный глобальный logger (упростили: убрали Arc, он не нужен для static)
static LOGGER: Lazy<Mutex<Option<Box<dyn LogPrinter>>>> =
    Lazy::new(|| Mutex::new(None));

pub static LOGGER_LEVEL: Lazy<Mutex<LogLevel>> = Lazy::new(|| Mutex::new(LogLevel::Debug));

pub fn set_logger(logger: Box<dyn LogPrinter>) {
    *LOGGER.lock().unwrap() = Some(logger);
}

pub fn set_log_level(level: LogLevel) {
    *LOGGER_LEVEL.lock().unwrap() = level;
}

pub fn set_logger_if_needed(logger: Box<dyn LogPrinter>) {
    if LOGGER.lock().unwrap().is_none() {
        set_logger(logger);
    }
}

pub enum LogLevel {
    Debug,
    Info,
    Warning,
    Error,
}

impl PartialOrd for LogLevel {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for LogLevel {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.as_int().cmp(&other.as_int())
    }
}

impl PartialEq for LogLevel {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == std::cmp::Ordering::Equal
    }
}

impl Eq for LogLevel {}

impl LogLevel {
    pub fn as_str(&self) -> &str {
        match self {
            LogLevel::Debug => "DEBUG",
            LogLevel::Info => "INFO",
            LogLevel::Warning => "WARNING",
            LogLevel::Error => "ERROR",
        }
    }

    pub fn as_int(&self) -> i32 {
        match self {
            LogLevel::Debug => 0,
            LogLevel::Info => 1,
            LogLevel::Warning => 2,
            LogLevel::Error => 3,
        }
    }
}

pub trait LogPrinter: Send + Sync {
    fn log(&self, level: LogLevel, message: &str);
}

pub fn log(message: &str) {
    // Лочим mutex, достаём logger, вызываем log (timestamp добавляем здесь, не в GodotLogger)
    log_with_level(LogLevel::Debug, message);
}

pub fn log_info(message: &str) {
    // Лочим mutex, достаём logger, вызываем log (timestamp добавляем здесь, не в GodotLogger)
    log_with_level(LogLevel::Info, message);
}

pub fn log_warning(message: &str) {
    log_with_level(LogLevel::Warning, message);
}

pub fn log_error(message: &str) {
    log_with_level(LogLevel::Error, message);
}

pub fn log_with_level(level: LogLevel, message: &str) {
    // Лочим mutex, достаём logger, вызываем log (timestamp добавляем здесь, не в GodotLogger)
    if let Some(logger) = LOGGER.lock().unwrap().as_ref() {
        let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
        logger.log(level, &format!("[{}] {}", timestamp, message));
    }
}

pub struct ConsoleLogger;

impl LogPrinter for ConsoleLogger {
    fn log(&self, level: LogLevel, message: &str) {
        println!("[{}] {}", level.as_str(), message);
    }
}

pub fn init_logger() {
    set_logger_if_needed(Box::new(ConsoleLogger));
}
