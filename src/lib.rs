use regex::{Regex, RegexSet, RegexSetBuilder};
use std::collections::{HashMap, HashSet};
use std::env::args_os;
use std::error::Error;
use std::fmt;
use std::io::{self, Write};
use std::path::PathBuf;
use std::result;
use tempfile::NamedTempFile;

pub type Result<T> = result::Result<T, Box<dyn Error>>;

macro_rules! err {
    ($($tt:tt)*) => {
        Err(Box::<dyn Error>::from(format!($($tt)*)))
    }
}

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
const EXCEPTIONS: &[&str] = &[
    // password retrieval to clipboard
    "^pass -c",
];

#[derive(PartialEq, Eq, Ord)]
pub struct HistoryCommand {
    timestamp: u32,
    command: String,
}

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
        return Self {
            data: s.lines(),
            timestamp_regex: Regex::new(r"^#\d+$").expect("You've got a bad regex there."),
            next_timestamp: None,
        };
    }
}

impl Iterator for HistoryIterator<'_> {
    type Item = Result<HistoryCommand>;
    fn next(&mut self) -> Option<Self::Item> {
        let mut command = String::new();
        let timestamp: Option<&str>;
        loop {
            match self.data.next() {
                // Either new or duplicate timestamp, take the last while command is empty
                Some(line) if self.timestamp_regex.is_match(line) && command.is_empty() => {
                    self.next_timestamp = Some(line)
                }
                // New timestamp, return a completed command
                Some(line) if self.timestamp_regex.is_match(line) && !command.is_empty() => {
                    timestamp = self.next_timestamp;
                    self.next_timestamp = Some(line);
                    break;
                }
                // Accumulate lines of command (if multiple)
                Some(line) => command += line,

                None => {
                    timestamp = self.next_timestamp;
                    self.next_timestamp = None;
                    break;
                }
            };
        }

        let timestamp: Result<u32> = match timestamp {
            None => return None,
            Some(v) => v
                .trim()
                .trim_start_matches("#")
                .parse()
                .map_err(|e: std::num::ParseIntError| e.into()),
        };

        // Get rid of differences in whitespace
        let command = command.split_whitespace().collect::<Vec<_>>().join(" ");

        match (timestamp, command) {
            (Ok(timestamp), command) if command.is_empty() => {
                return Some(err!("command was empty for timestamp {}", timestamp))
            }
            (Ok(timestamp), command) => return Some(Ok(HistoryCommand { timestamp, command })),
            (Err(e), command) => return Some(err!("{}, {}", e, command)),
        }
    }
}

pub fn usage() -> io::Result<()> {
    writeln!(
        io::stderr(),
        "cleanup-history :: Deduplicate bash history file\
        \n    USAGE: cleanup-history historyfile\
        "
    )
}

pub fn parse_args<T, U>(args: &mut T) -> Result<PathBuf>
where
    T: Iterator<Item = U>,
    U: std::convert::AsRef<std::ffi::OsStr>,
{
    let _script = args.next();
    let history_file = match args.next() {
        Some(path) => path,
        _ => return err!("please supply the path to the bash_history file"),
    };
    if args.next().is_some() {
        return err!("this script only accepts one argument");
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

fn old_clean_history(history_file: &PathBuf) -> Result<Vec<String>> {
    let ignore_regex = RegexSetBuilder::new(IGNORES)
        .case_insensitive(true)
        .build()?;
    let exception_regex = RegexSetBuilder::new(EXCEPTIONS)
        .case_insensitive(true)
        .build()?;

    let mut seen = HashSet::new();
    let mut new_lines = Vec::new();
    let input = std::fs::read_to_string(history_file)?;

    for line in input.lines().rev() {
        let line = line.split_whitespace().collect::<Vec<_>>().join(" ");
        if is_valid(&line, &exception_regex, &ignore_regex) && !seen.contains(&line) {
            seen.insert(line.clone());
            new_lines.push(line);
        }
    }
    Ok(new_lines.into_iter().rev().collect())
}

pub struct HistoryCommands(Vec<HistoryCommand>);
impl fmt::Display for HistoryCommands {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> result::Result<(), fmt::Error> {
        for hc in self.0.iter() {
            writeln!(f, "#{}", hc.timestamp)?;
            writeln!(f, "{}", hc.command)?;
        }
        Ok(())
    }
}

pub fn clean_history(input: &str) -> Result<HistoryCommands> {
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

fn old_write_history(history_file: &PathBuf, history: &[String]) -> Result<()> {
    let mut file = NamedTempFile::new()?;
    writeln!(file, "{}", history.join("\n"))?;
    file.persist(history_file)?;
    Ok(())
}

pub fn write_history(history_file: &PathBuf, history: &HistoryCommands) -> Result<()> {
    let mut file = NamedTempFile::new()?;
    write!(file, "{}", history)?;
    file.persist(history_file)?;
    Ok(())
}

pub fn run() -> Result<()> {
    let mut args = args_os();
    let history_file = parse_args(&mut args)?;
    let input = std::fs::read_to_string(&history_file)?;
    let history = clean_history(&input)?;
    write_history(&history_file, &history)
}
