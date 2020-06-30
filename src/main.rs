use regex::{Regex, RegexSet, RegexSetBuilder};
use std::collections::HashSet;
use std::env::args_os;
use std::error::Error;
use std::io::{self, Write};
use std::path::PathBuf;
use std::result;
use std::str::FromStr;
use tempfile::NamedTempFile;

type Result<T> = result::Result<T, Box<dyn Error>>;

macro_rules! err {
    ($($tt:tt)*) => {
        Err(Box::<dyn Error>::from(format!($($tt)*)))
    }
}

const IGNORES: &[&str] = &[
    // short things
    "^.{1,3}$",
    // changing into relative directories
    r"^cd [^~/]",
    "^ls($| )",
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
struct HistoryCommand {
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

impl FromStr for HistoryCommand {
    type Err = Box<dyn Error>;
    fn from_str(s: &str) -> Result<Self> {
        Ok(HistoryCommand {
            timestamp: 1,
            command: "".to_owned(),
        })
    }
}

fn usage() -> io::Result<()> {
    writeln!(
        io::stderr(),
        "cleanup-history :: Deduplicate bash history file\
        \n    USAGE: cleanup-history historyfile\
        "
    )
}

fn parse_args() -> Result<PathBuf> {
    let mut args = args_os();
    let _script = args.next();
    let history_file = match args.next() {
        Some(path) => path,
        _ => return err!("Err: please supply the path to the bash_history file"),
    };
    let path = PathBuf::from(&history_file);
    if !path.is_file() {
        let pathstr = path.to_str().unwrap_or("[invalid utf8]");
        return err!("Err: {} is not a file", pathstr);
    }
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

fn clean_history(history_file: &PathBuf) -> Result<Vec<String>> {
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

fn write_history(history_file: &PathBuf, history: &[String]) -> Result<()> {
    let mut file = NamedTempFile::new()?;
    writeln!(file, "{}", history.join("\n"))?;
    file.persist(history_file)?;
    Ok(())
}

fn main() -> Result<()> {
    let history_file = match parse_args() {
        Ok(file) => file,
        Err(e) => {
            writeln!(io::stderr(), "{}", e)?;
            usage()?;
            std::process::exit(1);
        }
    };
    let history = clean_history(&history_file)?;
    write_history(&history_file, &history)?;
    Ok(())
}
