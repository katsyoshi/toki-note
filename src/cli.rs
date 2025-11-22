use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

#[derive(Parser)]
#[command(version, about = "CLI scheduler backed by SQLite")]
pub struct Cli {
    /// Path to the SQLite database file
    #[arg(long, short = 'b', global = true)]
    pub database: Option<PathBuf>,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Add a schedule entry
    Add(AddCommand),
    /// List stored schedule entries
    List(ListCommand),
    /// Delete a schedule entry
    Delete(DeleteCommand),
    /// Emit events as an RSS feed
    Rss(RssCommand),
    /// Emit an iCalendar (.ics) feed
    Ical(IcalCommand),
    /// Import events from an .ics file
    Import(ImportCommand),
}

#[derive(Args)]
pub struct AddCommand {
    /// Event title
    #[arg(long, short = 't')]
    pub title: String,
    /// Start instant in RFC3339 (e.g. 2024-06-01T09:00:00+09:00) or YYYY-MM-DD when --all-day is set
    #[arg(long, short = 's')]
    pub start: String,
    /// End instant in RFC3339; optional for --all-day
    #[arg(long, short = 'e')]
    pub end: Option<String>,
    /// Optional note or description
    #[arg(long, short = 'n')]
    pub note: Option<String>,
    /// Repeatable tag values (e.g. --tag work --tag urgent)
    #[arg(long = "tag", action = clap::ArgAction::Append)]
    pub tags: Vec<String>,
    /// Store event as all-day entry (start/end treated as dates)
    #[arg(long, short = 'a')]
    pub all_day: bool,
    /// Duration syntax like 30m, 2h, 1h30m; ignored when --end is provided
    #[arg(long, short = 'u')]
    pub duration: Option<String>,
}

#[derive(Args)]
pub struct ListCommand {
    /// Filter by a specific day (UTC) e.g. 2025-06-01
    #[arg(long, short = 'd')]
    pub day: Option<String>,
    /// Timezone for display, e.g. Europe/Paris; defaults to local system zone
    #[arg(long = "tz", short = 'z')]
    pub tz: Option<String>,
}

#[derive(Args)]
pub struct RssCommand {
    /// Optional day filter (UTC)
    #[arg(long, short = 'd')]
    pub day: Option<String>,
    /// Override timezone used inside descriptions
    #[arg(long = "tz", short = 'z')]
    pub tz: Option<String>,
    /// Channel title
    #[arg(long)]
    pub title: Option<String>,
    /// Channel link
    #[arg(long)]
    pub link: Option<String>,
    /// Channel description
    #[arg(long)]
    pub description: Option<String>,
    /// Write RSS XML to this file instead of stdout
    #[arg(long, short = 'o')]
    pub output: Option<PathBuf>,
}

#[derive(Args)]
pub struct IcalCommand {
    /// Optional day filter (UTC)
    #[arg(long, short = 'd')]
    pub day: Option<String>,
    /// Override timezone used for timed events
    #[arg(long = "tz", short = 'z')]
    pub tz: Option<String>,
    /// Write ICS to this file instead of stdout
    #[arg(long, short = 'o')]
    pub output: Option<PathBuf>,
}

#[derive(Args)]
pub struct DeleteCommand {
    /// Numeric event id to remove
    #[arg(long, short = 'i')]
    pub id: Option<i64>,
    /// Event title to remove (deletes matching rows)
    #[arg(long, short = 't')]
    pub title: Option<String>,
}

#[derive(Args)]
pub struct ImportCommand {
    /// Path to the .ics file to import
    #[arg(long = "path", short = 'p')]
    pub path: Option<PathBuf>,
}
