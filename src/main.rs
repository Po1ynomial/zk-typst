use std::error::Error;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand, ValueEnum};
use serde::Serialize;
use thiserror::Error;
use zk::archive::Archive;
use zk::model::{Diagnostic, Envelope, Severity};
use zk::provider::Provider;

#[derive(Debug, Parser)]
#[command(name = "zk", version, about = "Manage a Typst Zettelkasten archive")]
struct Cli {
    /// Use this archive instead of discovering one from the current directory.
    #[arg(long, global = true, value_name = "PATH")]
    archive: Option<PathBuf>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Initialize an archive with an editable note template and Typst library.
    Init {
        /// Install bundled archive-local agent skills.
        #[arg(long)]
        agent_skills: bool,

        /// Directory to initialize.
        #[arg(default_value = ".")]
        path: PathBuf,
    },

    /// Create a Zettel with the next available timestamp ID.
    New,

    /// Check archive integrity.
    Check {
        /// Diagnostic output format.
        #[arg(long, value_enum, default_value = "text")]
        format: CheckFormat,
    },

    /// Query saved archive state.
    Query {
        #[command(subcommand)]
        query: QueryCommand,
    },

    /// Remove a Zettel when it has no incoming references.
    Remove {
        /// Zettel ID to remove.
        id: String,
    },

    /// Run the Zettelkasten language server over standard input and output.
    Lsp,

    /// Emit a complete disk-backed archive graph.
    Graph {
        /// Snapshot serialization format.
        #[arg(long, value_enum)]
        format: GraphFormat,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CheckFormat {
    Text,
    Json,
}

#[derive(Debug, Subcommand)]
enum QueryCommand {
    /// Return one Zettel and its metadata.
    Node { id: String },

    /// Return outgoing links for one Zettel.
    Links { id: String },

    /// Return incoming links for one Zettel.
    Backlinks { id: String },

    /// Search saved Zettel metadata.
    Search { query: String },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum GraphFormat {
    Json,
}

#[derive(Debug, Error)]
enum CliError {
    #[error("Zettel `{0}` does not exist")]
    MissingZettel(String),

    #[error("`--archive` cannot be used with `init`; pass the target to `zk init [PATH]`")]
    ArchiveWithInit,
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(error) => {
            eprintln!("zk: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<ExitCode, Box<dyn Error>> {
    let cli = Cli::parse();
    if cli.archive.is_some() && matches!(&cli.command, Command::Init { .. }) {
        return Err(CliError::ArchiveWithInit.into());
    }

    match cli.command {
        Command::Init { path, agent_skills } => {
            let archive = Archive::init(path)?;
            if agent_skills {
                for warning in archive.install_agent_skills() {
                    eprintln!("zk: warning: {warning}");
                }
            }
            println!("{}", archive.root().display());
        }
        Command::New => {
            let archive = discover_archive(cli.archive.as_deref())?;
            let path = archive.create_zettel()?;
            let display = path.strip_prefix(archive.root()).unwrap_or(&path);
            println!("{}", display.display());
        }
        Command::Check { format } => {
            let provider = load_provider(cli.archive.as_deref())?;
            match format {
                CheckFormat::Text => print_diagnostics(provider.diagnostics()),
                CheckFormat::Json => write_json(provider.diagnostics())?,
            }
            if provider
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.severity == Severity::Error)
            {
                return Ok(ExitCode::FAILURE);
            }
        }
        Command::Query { query } => {
            let provider = load_provider(cli.archive.as_deref())?;
            match query {
                QueryCommand::Node { id } => {
                    let node = provider
                        .node(&id)
                        .ok_or_else(|| CliError::MissingZettel(id.clone()))?;
                    write_json(node)?;
                }
                QueryCommand::Links { id } => {
                    require_node(&provider, &id)?;
                    write_json(&provider.links_from(&id))?;
                }
                QueryCommand::Backlinks { id } => {
                    require_node(&provider, &id)?;
                    write_json(&provider.links_to(&id))?;
                }
                QueryCommand::Search { query } => {
                    write_json(&provider.search_metadata(&query))?;
                }
            }
        }
        Command::Remove { id } => {
            let archive = discover_archive(cli.archive.as_deref())?;
            let provider = Provider::load(&archive)?;
            require_node(&provider, &id)?;
            let incoming = provider.links_to(&id);
            if !incoming.is_empty() {
                eprintln!("cannot remove Zettel `{id}`; incoming references exist:");
                for link in incoming {
                    let path = &provider
                        .node(&link.source)
                        .expect("link sources are provider nodes")
                        .path;
                    for span in link.spans {
                        eprintln!("  {path}:{}..{}", span.start, span.end);
                    }
                }
                return Ok(ExitCode::FAILURE);
            }
            let path = archive.remove_zettel(&id)?;
            let display = path.strip_prefix(archive.root()).unwrap_or(&path);
            println!("{}", display.display());
        }
        Command::Lsp => {
            let archive = discover_archive(cli.archive.as_deref())?;
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()?;
            return Ok(runtime.block_on(zk::lsp::serve(archive))?);
        }
        Command::Graph {
            format: GraphFormat::Json,
        } => {
            let provider = load_provider(cli.archive.as_deref())?;
            write_json(&provider.snapshot())?;
        }
    }

    Ok(ExitCode::SUCCESS)
}

fn discover_archive(explicit: Option<&Path>) -> Result<Archive, Box<dyn Error>> {
    let current_dir = std::env::current_dir()?;
    if let Some(path) = explicit {
        let root = if path.is_absolute() {
            path.to_path_buf()
        } else {
            current_dir.join(path)
        };
        return Ok(Archive::open(root)?);
    }
    Ok(Archive::discover(current_dir)?)
}

fn load_provider(explicit: Option<&Path>) -> Result<Provider, Box<dyn Error>> {
    Ok(Provider::load(&discover_archive(explicit)?)?)
}

fn require_node<'a>(
    provider: &'a Provider,
    id: &str,
) -> Result<&'a zk::model::ZettelNode, CliError> {
    provider
        .node(id)
        .ok_or_else(|| CliError::MissingZettel(id.to_owned()))
}

fn write_json<T: Serialize + ?Sized>(value: &T) -> Result<(), Box<dyn Error>> {
    let stdout = std::io::stdout();
    let mut output = stdout.lock();
    serde_json::to_writer_pretty(&mut output, &Envelope::new(value))?;
    output.write_all(b"\n")?;
    Ok(())
}

fn print_diagnostics(diagnostics: &[Diagnostic]) {
    let mut errors = 0;
    let mut warnings = 0;
    for diagnostic in diagnostics {
        let severity = match diagnostic.severity {
            Severity::Error => {
                errors += 1;
                "error"
            }
            Severity::Warning => {
                warnings += 1;
                "warning"
            }
        };
        match diagnostic.range {
            Some(range) => println!(
                "{severity}: {}:{}..{}: [{}] {}",
                diagnostic.path, range.start, range.end, diagnostic.code, diagnostic.message
            ),
            None => println!(
                "{severity}: {}: [{}] {}",
                diagnostic.path, diagnostic.code, diagnostic.message
            ),
        }
    }
    println!("{errors} error(s), {warnings} warning(s)");
}
