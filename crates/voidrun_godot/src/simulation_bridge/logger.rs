//! GodotLogger implementation
//!
//! Bridges Rust logging to Godot's godot_print!/godot_error! + logs/game.log file.

use voidrun_simulation::{LogLevel, LogPrinter};

pub struct GodotLogger;

impl LogPrinter for GodotLogger {
    fn log(&self, level: LogLevel, message: &str) {
        if level >= *voidrun_simulation::LOGGER_LEVEL.lock().unwrap() {
            self._log_message(level, message);
        }
    }
}

impl GodotLogger {
    pub fn clear_log_file() {
        let log_path = std::path::Path::new("../logs/game.log");
        if let Some(_parent) = log_path.parent() {
            let _ = std::fs::remove_file(log_path);
        }
    }

    fn _log_message(&self, level: LogLevel, message: &str) {
        use std::io::Write;
        if level == LogLevel::Error {
            godot::prelude::godot_error!("[{}] {}", level.as_str(), message);
        } else {
        }
        godot::prelude::godot_print!("[{}] {}", level.as_str(), message);

        // Пишем в файл logs/game.log (append mode)
        // Godot запускается из godot/ директории, поэтому путь относительно project root
        let log_path = std::path::Path::new("../logs/game.log");

        // Создаём директорию если не существует
        if let Some(parent) = log_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        match std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path)
        {
            Ok(mut file) => {
                let _ = writeln!(file, "{}", message);
            }
            Err(e) => {
                // Логируем ошибку только один раз (первый раз)
                static mut ERROR_LOGGED: bool = false;
                unsafe {
                    if !ERROR_LOGGED {
                        godot::prelude::godot_error!("❌ Failed to open log file {:?}: {}", log_path, e);
                        ERROR_LOGGED = true;
                    }
                }
            }
        }
    }
}
