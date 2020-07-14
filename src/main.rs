use cleanup_history::{run, usage};

use std::io::{self, Result, Write};

fn main() -> Result<()> {
    if let Err(err) = run() {
        writeln!(io::stderr(), "Error: {}", err)?;
        usage()?;
        std::process::exit(1);
    }
    Ok(())
}
