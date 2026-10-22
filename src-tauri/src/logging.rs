//! Logs to stdout and, when `LGTM_LOG_DIR` is set, to hourly files there. The last 24 hours
//! of files are kept, and an hour's file stops growing at `HOUR_CAP`, so a runaway TRACE
//! can't fill the disk.

use std::io::Write;
use std::path::Path;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

/// The most one hour's file takes: a day of files tops out near 1.2 GB.
const HOUR_CAP: u64 = 50 * 1024 * 1024;
const HOURS_KEPT: usize = 24;

/// Drops writes past `cap` bytes until the hour changes, as the files roll at UTC hours.
struct Capped<W> {
    inner: W,
    cap: u64,
    hour: u64,
    written: u64,
    now: fn() -> u64,
}

fn utc_hour() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() / 3600)
}

impl<W: Write> Write for Capped<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let hour = (self.now)();
        if hour != self.hour {
            self.hour = hour;
            self.written = 0;
        }
        if self.written >= self.cap {
            return Ok(buf.len());
        }
        let n = self.inner.write(buf)?;
        self.written += n as u64;
        if self.written >= self.cap {
            self.inner
                .write_all(b"lgtm: this hour's log is full; logging resumes next hour\n")?;
        }
        Ok(n)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

fn file_writer(dir: &Path) -> anyhow::Result<Capped<RollingFileAppender>> {
    let inner = RollingFileAppender::builder()
        .rotation(Rotation::HOURLY)
        .filename_prefix("lgtm")
        .filename_suffix("log")
        .max_log_files(HOURS_KEPT)
        .build(dir)?;
    Ok(Capped {
        inner,
        cap: HOUR_CAP,
        hour: utc_hour(),
        written: 0,
        now: utc_hour,
    })
}

/// Logs at `RUST_LOG`'s levels, else INFO for lgtm and its UI and WARN for its libraries.
/// Not for `lgtm mcp` and `lgtm hook`, whose stdout talks to Claude Code.
pub fn init() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("lgtm_lib=info,ui=info,warn"));
    let dir = std::env::var_os("LGTM_LOG_DIR").filter(|d| !d.is_empty());
    let (file, failed) = match dir.as_deref().map(|d| file_writer(Path::new(d))) {
        Some(Ok(w)) => (Some(w), None),
        Some(Err(e)) => (None, Some(e)),
        None => (None, None),
    };
    let file = file.map(|w| {
        tracing_subscriber::fmt::layer()
            .with_ansi(false)
            .with_writer(Mutex::new(w))
    });
    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer())
        .with(file)
        .init();
    if let Some(e) = failed {
        tracing::warn!("no log files: {e:#}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_hour_stops_at_the_cap_and_the_next_starts_over() {
        static HOUR: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(7);
        let now = || HOUR.load(std::sync::atomic::Ordering::Relaxed);
        let mut w = Capped {
            inner: Vec::new(),
            cap: 10,
            hour: 7,
            written: 0,
            now,
        };
        w.write_all(b"12345678").unwrap();
        w.write_all(b"abcd").unwrap();
        w.write_all(b"dropped").unwrap();
        let full = String::from_utf8(w.inner.clone()).unwrap();
        assert!(full.starts_with("12345678abcdlgtm: this hour's log is full"), "{full}");
        assert!(!full.contains("dropped"));

        HOUR.store(8, std::sync::atomic::Ordering::Relaxed);
        w.inner.clear();
        w.write_all(b"next").unwrap();
        assert_eq!(w.inner, b"next");
    }
}
