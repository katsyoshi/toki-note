mod cli;
mod commands;
mod config;
mod storage;

use anyhow::Result;
use clap::Parser;
use cli::{Cli, Command};
use commands::{add_event, delete_event, generate_ical, generate_rss, import_ics, list_events};
use config::{load_config, resolve_database_path};
use storage::Storage;

fn main() -> Result<()> {
    let cli = Cli::parse();
    let config = load_config()?;
    let db_path = resolve_database_path(cli.database.or(config.database_path()))?;
    let mut storage = Storage::new(&db_path)?;

    match cli.command {
        Command::Add(cmd) => add_event(&mut storage, cmd),
        Command::List(cmd) => list_events(&storage, cmd),
        Command::Delete(cmd) => delete_event(&mut storage, cmd),
        Command::Rss(mut cmd) => {
            if cmd.output.is_none() {
                cmd.output = config.rss_output_path();
            }
            generate_rss(&storage, cmd)
        }
        Command::Ical(mut cmd) => {
            if cmd.output.is_none() {
                cmd.output = config.ical_output_path();
            }
            generate_ical(&storage, cmd)
        }
        Command::Import(mut cmd) => {
            if cmd.path.is_none() {
                cmd.path = config.import_source_path();
            }
            import_ics(&mut storage, cmd)
        }
    }
}
