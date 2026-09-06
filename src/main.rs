use std::error::Error;
use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand, ValueEnum};
use zk::archive::Archive;
use zk::provider::Provider;

#[derive(Debug, Parser)]
#[command(name = "zk", version, about = "Manage a Typst Zettelkasten archive")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Initialize an archive using the fixed version-one layout.
    Init {
        /// Directory to initialize.
        #[arg(default_value = ".")]
        path: PathBuf,
    },

    /// Create a Zettel with the next available timestamp ID.
    New,

    /// Emit a complete disk-backed archive graph.
    Graph {
        /// Snapshot serialization format.
        #[arg(long, value_enum)]
        format: GraphFormat,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum GraphFormat {
    Json,
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("zk: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    match Cli::parse().command {
        Command::Init { path } => {
            let archive = Archive::init(path)?;
            println!("{}", archive.root().display());
        }
        Command::New => {
            let current = std::env::current_dir()?;
            let archive = Archive::discover(current)?;
            let path = archive.create_zettel()?;
            let display = path.strip_prefix(archive.root()).unwrap_or(&path);
            println!("{}", display.display());
        }
        Command::Graph {
            format: GraphFormat::Json,
        } => {
            let current = std::env::current_dir()?;
            let archive = Archive::discover(current)?;
            let provider = Provider::load(&archive)?;
            let snapshot = provider.snapshot();
            let stdout = std::io::stdout();
            let mut output = stdout.lock();
            serde_json::to_writer_pretty(&mut output, &snapshot)?;
            output.write_all(b"\n")?;
        }
    }

    Ok(())
}
