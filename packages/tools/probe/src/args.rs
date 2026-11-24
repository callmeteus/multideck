//! Command line parsing, kept separate from the commands so it can be tested
//! without touching the filesystem or the network stack.

use std::path::PathBuf;

use crate::strings::{DEFAULT_LIMIT, DEFAULT_MIN_LEN};

/// Usage text printed by `probe help` and on argument errors_
pub const USAGE: &str = "\
multideck-probe - reconnaissance helper for the Stream Deck Mobile work

USAGE:
    multideck-probe strings <path> [--min N] [--grep TEXT] [--limit N]
    multideck-probe ports [--pid N]
    multideck-probe env
    multideck-probe help

COMMANDS:
    strings    Extract printable ASCII runs from a file.
               --min N      shortest run to report (default 6)
               --grep TEXT  case-insensitive substring filter
               --limit N    max lines printed (default 200)
               Exits 1 when nothing matches.
    ports      List TCP listeners (netstat on Windows, ss or /proc on Linux).
               --pid N      only listeners owned by this process id
    env        Print OS, architecture, build profile, Rust version and the
               data directory candidate.
    help       Print this text.";

/// What `main` should run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    /// Extract strings from a file.
    Strings(StringsArgs),
    /// List TCP listeners.
    Ports(PortsArgs),
    /// Print the environment report.
    Env,
    /// Print usage.
    Help,
}

/// Arguments for `strings`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StringsArgs {
    /// File to scan.
    pub path: PathBuf,
    /// Shortest run to report.
    pub min: usize,
    /// Optional case-insensitive substring filter.
    pub grep: Option<String>,
    /// Maximum number of printed lines.
    pub limit: usize,
}

/// Arguments for `ports`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PortsArgs {
    /// Optional process id filter.
    pub pid: Option<u32>,
}

/// Parses the arguments after the executable name.
///
/// # Errors
///
/// Returns a human readable message for unknown commands, unknown options,
/// missing option values and unparsable numbers. Callers print it and exit
/// with a failure code.
pub fn parse(args: &[String]) -> Result<Command, String> {
    let Some(command) = args.first().map(String::as_str) else {
        return Ok(Command::Help);
    };
    let rest = &args[1..];

    if matches!(command, "help" | "--help" | "-h") {
        return Ok(Command::Help);
    }
    if rest.iter().any(|arg| arg == "--help" || arg == "-h") {
        return Ok(Command::Help);
    }

    match command {
        "strings" => parse_strings(rest).map(Command::Strings),
        "ports" => parse_ports(rest).map(Command::Ports),
        "env" => {
            if rest.is_empty() {
                Ok(Command::Env)
            } else {
                Err(format!(
                    "`env` takes no arguments, got `{}`",
                    rest.join(" ")
                ))
            }
        }
        other => Err(format!("unknown command `{other}`, try `help`")),
    }
}

/// Parses the tail of a `strings` invocation.
fn parse_strings(rest: &[String]) -> Result<StringsArgs, String> {
    let mut path = None;
    let mut min = DEFAULT_MIN_LEN;
    let mut limit = DEFAULT_LIMIT;
    let mut grep = None;
    let mut index = 0;

    while index < rest.len() {
        let arg = rest[index].as_str();

        if let Some(value) = arg.strip_prefix("--min=") {
            min = parse_number("--min", value)?;
        } else if let Some(value) = arg.strip_prefix("--limit=") {
            limit = parse_number("--limit", value)?;
        } else if let Some(value) = arg.strip_prefix("--grep=") {
            grep = Some(value.to_string());
        } else {
            match arg {
                "--min" => {
                    min = parse_number("--min", value_at(rest, index)?)?;
                    index += 1;
                }
                "--limit" => {
                    limit = parse_number("--limit", value_at(rest, index)?)?;
                    index += 1;
                }
                "--grep" => {
                    grep = Some(value_at(rest, index)?.to_string());
                    index += 1;
                }
                flag if flag.starts_with('-') => {
                    return Err(format!("unknown option `{flag}`, try `help`"));
                }
                value => {
                    if path.is_some() {
                        return Err(format!("unexpected extra argument `{value}`"));
                    }
                    path = Some(PathBuf::from(value));
                }
            }
        }

        index += 1;
    }

    let path = path.ok_or_else(|| "`strings` needs a file path".to_string())?;
    Ok(StringsArgs {
        path,
        min,
        grep,
        limit,
    })
}

/// Parses the tail of a `ports` invocation.
fn parse_ports(rest: &[String]) -> Result<PortsArgs, String> {
    let mut pid = None;
    let mut index = 0;

    while index < rest.len() {
        let arg = rest[index].as_str();

        if let Some(value) = arg.strip_prefix("--pid=") {
            pid = Some(parse_number("--pid", value)?);
        } else if arg == "--pid" {
            pid = Some(parse_number("--pid", value_at(rest, index)?)?);
            index += 1;
        } else {
            return Err(format!("unknown option `{arg}`, try `help`"));
        }

        index += 1;
    }

    Ok(PortsArgs { pid })
}

/// Returns the value that must follow the option at `index`.
fn value_at(args: &[String], index: usize) -> Result<&str, String> {
    args.get(index + 1)
        .map(String::as_str)
        .ok_or_else(|| format!("`{}` needs a value", args[index]))
}

/// Parses a positive integer option value.
fn parse_number<T>(flag: &str, value: &str) -> Result<T, String>
where
    T: std::str::FromStr,
{
    value
        .parse::<T>()
        .map_err(|_| format!("`{flag}` expects a number, got `{value}`"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    #[test]
    fn no_arguments_is_help() {
        assert_eq!(parse(&[]), Ok(Command::Help));
    }

    #[test]
    fn help_variants_are_help() {
        for value in ["help", "--help", "-h"] {
            assert_eq!(parse(&args(&[value])), Ok(Command::Help));
        }
        assert_eq!(
            parse(&args(&["strings", "x.bin", "--help"])),
            Ok(Command::Help)
        );
    }

    #[test]
    fn strings_defaults() {
        let command = parse(&args(&["strings", "SD.exe"])).expect("parses");
        assert_eq!(
            command,
            Command::Strings(StringsArgs {
                path: PathBuf::from("SD.exe"),
                min: DEFAULT_MIN_LEN,
                grep: None,
                limit: DEFAULT_LIMIT,
            })
        );
    }

    #[test]
    fn strings_accepts_separate_flag_values() {
        let command = parse(&args(&[
            "strings", "--min", "8", "SD.exe", "--grep", "elg", "--limit", "5",
        ]))
        .expect("parses");
        match command {
            Command::Strings(args) => {
                assert_eq!(args.path, PathBuf::from("SD.exe"));
                assert_eq!(args.min, 8);
                assert_eq!(args.grep.as_deref(), Some("elg"));
                assert_eq!(args.limit, 5);
            }
            other => panic!("expected strings, got {other:?}"),
        }
    }

    #[test]
    fn strings_accepts_equals_form() {
        let command = parse(&args(&["strings", "a.bin", "--min=4", "--limit=1"])).expect("parses");
        match command {
            Command::Strings(args) => {
                assert_eq!(args.min, 4);
                assert_eq!(args.limit, 1);
            }
            other => panic!("expected strings, got {other:?}"),
        }
    }

    #[test]
    fn strings_rejects_missing_path_and_bad_values() {
        assert!(parse(&args(&["strings"])).is_err());
        assert!(parse(&args(&["strings", "a", "--min"])).is_err());
        assert!(parse(&args(&["strings", "a", "--min", "abc"])).is_err());
        assert!(parse(&args(&["strings", "a", "b"])).is_err());
        assert!(parse(&args(&["strings", "a", "--verbose"])).is_err());
    }

    #[test]
    fn ports_parses_pid_filter() {
        assert_eq!(
            parse(&args(&["ports", "--pid", "4242"])),
            Ok(Command::Ports(PortsArgs { pid: Some(4242) }))
        );
        assert_eq!(
            parse(&args(&["ports", "--pid=7"])),
            Ok(Command::Ports(PortsArgs { pid: Some(7) }))
        );
        assert_eq!(
            parse(&args(&["ports"])),
            Ok(Command::Ports(PortsArgs { pid: None }))
        );
    }

    #[test]
    fn ports_rejects_unknown_options() {
        assert!(parse(&args(&["ports", "--port", "80"])).is_err());
        assert!(parse(&args(&["ports", "--pid", "nope"])).is_err());
    }

    #[test]
    fn env_takes_no_arguments() {
        assert_eq!(parse(&args(&["env"])), Ok(Command::Env));
        assert!(parse(&args(&["env", "extra"])).is_err());
    }

    #[test]
    fn unknown_command_is_an_error() {
        assert!(parse(&args(&["explode"])).is_err());
    }
}
