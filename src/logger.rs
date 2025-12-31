use chrono::Local;
use std::collections::VecDeque;
use std::fs::{self, File};
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

const LOG_PATH: &str = "app.log";
const MAX_ENTRIES: usize = 5_000;

static LOGGER: OnceLock<SimpleLogger> = OnceLock::new();

/// Append a log entry with an automatic timestamp.
///
/// This function is thread-safe and lazily initializes the underlying logger
/// the first time it is called. The log file (`app.log`) keeps only the most
/// recent 5,000 entries.
pub fn log(message: impl AsRef<str>) {
    LOGGER
        .get_or_init(|| SimpleLogger::new(PathBuf::from(LOG_PATH), MAX_ENTRIES))
        .append(message.as_ref());
}

struct SimpleLogger {
    path: PathBuf,
    max_entries: usize,
    buffer: Mutex<VecDeque<String>>,
}

impl SimpleLogger {
    fn new(path: PathBuf, max_entries: usize) -> Self {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                if let Err(err) = fs::create_dir_all(parent) {
                    eprintln!("Logger: failed to create {}: {}", parent.display(), err);
                }
            }
        }

        let buffer = Self::read_existing(&path, max_entries).unwrap_or_else(|err| {
            eprintln!(
                "Logger: failed to read existing log {}: {}",
                path.display(),
                err
            );
            VecDeque::with_capacity(max_entries)
        });

        Self {
            path,
            max_entries,
            buffer: Mutex::new(buffer),
        }
    }

    fn read_existing(path: &Path, max_entries: usize) -> io::Result<VecDeque<String>> {
        if !path.exists() {
            return Ok(VecDeque::with_capacity(max_entries));
        }

        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let mut lines = VecDeque::with_capacity(max_entries);
        for line in reader.lines() {
            let line = line?;
            if lines.len() == max_entries {
                lines.pop_front();
            }
            lines.push_back(line);
        }
        Ok(lines)
    }

    fn append(&self, message: &str) {
        // Use chrono with numeric-only format to avoid locale issues on Windows
        // Avoid %b, %B, %a, %A which can produce non-ASCII characters
        let now = Local::now();
        let timestamp = format!(
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}.{:04}",
            now.format("%Y"),
            now.format("%m"),
            now.format("%d"),
            now.format("%H"),
            now.format("%M"),
            now.format("%S"),
            now.timestamp_subsec_millis()
        );
        let entry = format!("{} - {}", timestamp, message);

        let mut buffer = match self.buffer.lock() {
            Ok(guard) => guard,
            Err(poisoned) => {
                eprintln!("Logger: mutex poisoned, recovering buffer");
                poisoned.into_inner()
            }
        };

        buffer.push_back(entry);
        if buffer.len() > self.max_entries {
            buffer.pop_front();
        }

        if let Err(err) = Self::write_all(&self.path, &buffer) {
            eprintln!("Logger: failed to write {}: {}", self.path.display(), err);
        }
    }

    fn write_all(path: &Path, lines: &VecDeque<String>) -> io::Result<()> {
        let mut file = File::create(path)?;
        for line in lines {
            writeln!(file, "{}", line)?;
        }
        Ok(())
    }
}
