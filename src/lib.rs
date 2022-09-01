use regex::{Regex, RegexSet, RegexSetBuilder};
use std::collections::HashMap;
use std::env::args_os;
use std::error;
use std::fmt;
use std::io::{self, Write};
use std::path::PathBuf;
use std::result;
use tempfile::NamedTempFile;

type Error = Box<dyn error::Error + Send + Sync>;
type Result<T> = result::Result<T, Error>;

macro_rules! err {
    ($($tt:tt)*) => {
        Err(Error::from(format!($($tt)*)))
    }
}

/// IGNORES should not be included in the final history unless they are in EXCEPTIONS
const IGNORES: &[&str] = &[
    // short things
    "^.{1,3}$",
    // cd / ls with relative directories
    "^cd [^~/]",
    "^ls [^~/]",
    // annoying if accidentally re-executed at a later date
    "^(sudo)? reboot",
    "^(sudo)? shutdown",
    "^(sudo)? halt",
    // mouse esc codes
    "^0",
    // commands explicitly hidden by user
    "^ ",
    // frequent typos (see .aliases)

    // Sensitive looking lines
    "(api|token|key|secret|pass)",
];

/// EXCEPTIONS should be included in the history even if they also match IGNORES
const EXCEPTIONS: &[&str] = &[
    // password retrieval to clipboard
    "^pass -c",
];

#[derive(PartialEq, Eq, Ord)]
struct HistoryCommand {
    timestamp: u32,
    command: String,
}

/// HistoryCommand should be sorted by timestamp then alphabetically
impl PartialOrd for HistoryCommand {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(
            self.timestamp
                .cmp(&other.timestamp)
                .then_with(|| self.command.cmp(&other.command)),
        )
    }
}

struct HistoryIterator<'a> {
    data: std::str::Lines<'a>,
    timestamp_regex: Regex,
    next_timestamp: Option<&'a str>,
}

impl<'a> From<&'a str> for HistoryIterator<'a> {
    fn from(s: &'a str) -> Self {
        Self {
            data: s.lines(),
            timestamp_regex: Regex::new(r"^#\d+$").expect("You've got a bad regex there."),
            next_timestamp: None,
        }
    }
}

impl Iterator for HistoryIterator<'_> {
    type Item = Result<HistoryCommand>;
    fn next(&mut self) -> Option<Self::Item> {
        let mut command = String::new();
        let timestamp = loop {
            match self.data.next() {
                // Either new or duplicate timestamp, take the last while command is empty
                Some(line) if self.timestamp_regex.is_match(line) && command.is_empty() => {
                    self.next_timestamp = Some(line);
                }
                // New timestamp, return a completed command
                Some(line) if self.timestamp_regex.is_match(line) && !command.is_empty() => {
                    break self.next_timestamp.replace(line);
                }
                // Accumulate lines of command (if multiple)
                Some(line) => {
                    let trimmed_line = line.trim();
                    if !trimmed_line.is_empty() {
                        command += trimmed_line;
                        command += "; ";
                    }
                },
                // End of file
                None => {
                    break self.next_timestamp.take();
                }
            };
        };

        let timestamp: Option<Result<u32>> = timestamp.map(|v| {
                v.trim()
                .trim_start_matches('#')
                .parse()
                .map_err(Into::into)
        });

        // Get rid of differences in whitespace
        let command = command.split_whitespace().collect::<Vec<_>>().join(" ").trim_end_matches(';').to_owned();

        match (timestamp, command) {
            (Some(Ok(timestamp)), command) if command.is_empty() => {
                Some(err!("command was empty for timestamp {}", timestamp))
            }
            (Some(Ok(timestamp)), command) => Some(Ok(HistoryCommand { timestamp, command })),
            (Some(Err(e)), command) => Some(err!("{}, {}", e, command)),
            (None, command) if !command.is_empty() => Some(err!("missing timestamp for command: {}", command)),
            (None, _) => None,
        }
    }
}

/// Write the usage to stderr
pub fn usage() -> io::Result<()> {
    writeln!(
        io::stderr(),
        "cleanup-history :: Deduplicate bash history file\
        \n    USAGE: cleanup-history historyfile\
        "
    )
}

/// Ensure script was called with only one argument and parse the arg to path
fn parse_args<T, U>(args: &mut T) -> Result<PathBuf>
where
    T: Iterator<Item = U>,
    U: std::convert::AsRef<std::ffi::OsStr>,
{

    let _script = args.next();
    let history_file = args.next().ok_or_else(||
        Error::from("please supply the path to the bash_history file"))?;
    if args.next().is_some() {
        err!("this script only accepts one argument")?;
    }

    if let Some(s) = history_file.as_ref().to_str() {
        if ["-h", "--help"].contains(&s) {
            usage()?;
            std::process::exit(0);
        }
    }

    let path = PathBuf::from(&history_file);
    Ok(path)
}

fn is_valid(line: &str, exceptions: &RegexSet, ignores: &RegexSet) -> bool {
    if exceptions.is_match(line) {
        return true;
    } else if ignores.is_match(line) {
        return false;
    }
    true
}

struct HistoryCommands(Vec<HistoryCommand>);

impl fmt::Display for HistoryCommands {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> result::Result<(), fmt::Error> {
        for hc in self.0.iter() {
            writeln!(f, "#{}", hc.timestamp)?;
            writeln!(f, "{}", hc.command)?;
        }
        Ok(())
    }
}

fn clean_history(input: &str) -> Result<HistoryCommands> {
    let ignore_regex = RegexSetBuilder::new(IGNORES)
        .case_insensitive(true)
        .build()?;
    let exception_regex = RegexSetBuilder::new(EXCEPTIONS)
        .case_insensitive(true)
        .build()?;

    let mut history = <HashMap<String, u32>>::new();
    let iter: HistoryIterator = input.into();
    for hc in iter {
        match hc {
            Ok(hc) => {
                if is_valid(&hc.command, &exception_regex, &ignore_regex) {
                    let ts = history.entry(hc.command).or_insert(hc.timestamp);
                    if *ts < hc.timestamp {
                        *ts = hc.timestamp
                    }
                }
            }
            Err(e) => writeln!(io::stderr(), "{}", e)?,
        }
    }
    let mut new_commands = HistoryCommands(
        history
            .into_iter()
            .map(|(command, timestamp)| HistoryCommand { command, timestamp })
            .collect(),
    );
    new_commands.0.sort();
    Ok(new_commands)
}

fn write_history(history_file: &PathBuf, history: &HistoryCommands) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let dir = history_file.parent().unwrap_or(cwd.as_path());
    let mut file = NamedTempFile::new_in(dir)?;
    write!(file, "{}", history)?;
    file.persist(history_file)?;
    Ok(())
}

/// Expose a runner for the command line tool
pub fn run() -> Result<()> {
    let mut args = args_os();
    let history_file = parse_args(&mut args)?;
    let input = std::fs::read_to_string(&history_file)?;
    let history = clean_history(&input)?;
    write_history(&history_file, &history)
}

#[cfg(test)]
mod tests;
