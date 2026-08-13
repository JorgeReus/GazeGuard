use serde::{Deserialize, Serialize};
use std::fmt::{self, Arguments, Display, Formatter};
use std::io::{self, Write};

#[cfg(test)]
use std::cell::RefCell;

#[cfg(test)]
thread_local! {
    static TEST_LOG_CAPTURE: RefCell<Option<Vec<u8>>> = const { RefCell::new(None) };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Off,
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl LogLevel {
    pub fn parse(raw: Option<&str>) -> Self {
        let Some(raw) = raw.map(str::trim) else {
            return Self::Off;
        };

        if raw.eq_ignore_ascii_case("off") {
            Self::Off
        } else if raw.eq_ignore_ascii_case("error") {
            Self::Error
        } else if raw.eq_ignore_ascii_case("warn") {
            Self::Warn
        } else if raw.eq_ignore_ascii_case("info") {
            Self::Info
        } else if raw.eq_ignore_ascii_case("debug") {
            Self::Debug
        } else if raw.eq_ignore_ascii_case("trace") {
            Self::Trace
        } else {
            Self::Off
        }
    }

    pub fn allows(self, message_level: LogLevel) -> bool {
        self != Self::Off && self >= message_level
    }
}

fn write_log<W: Write>(
    writer: &mut W,
    level: LogLevel,
    configured: LogLevel,
    target: &str,
    args: Arguments<'_>,
) -> io::Result<()> {
    if !configured.allows(level) {
        return Ok(());
    }

    writeln!(writer, "[gazeguard][{target}][{level}] {args}")
}

pub fn log(level: LogLevel, configured: LogLevel, target: &str, args: Arguments<'_>) {
    #[cfg(test)]
    let captured = TEST_LOG_CAPTURE.with(|capture| {
        let mut capture = capture.borrow_mut();
        let Some(buffer) = capture.as_mut() else {
            return false;
        };

        let _ = write_log(buffer, level, configured, target, args);
        true
    });

    #[cfg(test)]
    if captured {
        return;
    }

    let mut stderr = io::stderr().lock();
    let _ = write_log(&mut stderr, level, configured, target, args);
}

impl Display for LogLevel {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::Off => "off",
            Self::Error => "error",
            Self::Warn => "warn",
            Self::Info => "info",
            Self::Debug => "debug",
            Self::Trace => "trace",
        };

        f.write_str(label)
    }
}

#[cfg(test)]
mod tests {
    use super::{log, LogLevel};

    fn capture_output<F>(f: F) -> String
    where
        F: FnOnce(),
    {
        super::TEST_LOG_CAPTURE.with(|capture| *capture.borrow_mut() = Some(Vec::new()));
        f();
        let bytes = super::TEST_LOG_CAPTURE.with(|capture| {
            capture
                .borrow_mut()
                .take()
                .expect("test log capture should be installed")
        });
        String::from_utf8(bytes).unwrap()
    }

    #[test]
    fn parses_known_log_levels() {
        assert_eq!(LogLevel::parse(Some("off")), LogLevel::Off);
        assert_eq!(LogLevel::parse(Some("error")), LogLevel::Error);
        assert_eq!(LogLevel::parse(Some("warn")), LogLevel::Warn);
        assert_eq!(LogLevel::parse(Some("info")), LogLevel::Info);
        assert_eq!(LogLevel::parse(Some("debug")), LogLevel::Debug);
        assert_eq!(LogLevel::parse(Some("trace")), LogLevel::Trace);
    }

    #[test]
    fn missing_or_invalid_log_level_defaults_to_off() {
        assert_eq!(LogLevel::parse(None), LogLevel::Off);
        assert_eq!(LogLevel::parse(Some("")), LogLevel::Off);
        assert_eq!(LogLevel::parse(Some("verbose")), LogLevel::Off);
    }

    #[test]
    fn higher_levels_enable_lower_levels() {
        assert!(LogLevel::Debug.allows(LogLevel::Info));
        assert!(LogLevel::Trace.allows(LogLevel::Debug));
        assert!(!LogLevel::Warn.allows(LogLevel::Info));
        assert!(!LogLevel::Off.allows(LogLevel::Error));
    }

    #[test]
    fn log_suppresses_messages_above_configured_level() {
        let output = capture_output(|| {
            log(
                LogLevel::Debug,
                LogLevel::Info,
                "desktop_signals",
                format_args!("suppressed"),
            );
        });

        assert!(output.is_empty());
    }

    #[test]
    fn log_emits_expected_output_format_when_enabled() {
        let output = capture_output(|| {
            log(
                LogLevel::Info,
                LogLevel::Info,
                "desktop_signals",
                format_args!("idle_active={}", true),
            );
        });

        assert_eq!(
            output,
            "[gazeguard][desktop_signals][info] idle_active=true\n"
        );
    }
}
