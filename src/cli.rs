use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

#[derive(Parser)]
#[command(version, about = "CLI scheduler backed by SQLite")]
pub struct Cli {
    /// Path to the SQLite database file
    #[arg(long, global = true)]
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
    /// Emit events as an RSS feed
    Rss(RssCommand),
    /// Emit an iCalendar (.ics) feed
    Ical(IcalCommand),
}

#[derive(Args)]
pub struct AddCommand {
    /// Event title
    #[arg(long)]
    pub title: String,
    /// Start instant in RFC3339 (e.g. 2024-06-01T09:00:00+09:00) or YYYY-MM-DD when --all-day is set
    #[arg(long)]
    pub start: String,
    /// End instant in RFC3339; optional for --all-day
    #[arg(long)]
    pub end: Option<String>,
    /// Optional note or description
    #[arg(long)]
    pub note: Option<String>,
    /// Repeatable tag values (e.g. --tag work --tag urgent)
    #[arg(long = "tag", action = clap::ArgAction::Append)]
    pub tags: Vec<String>,
    /// Store event as all-day entry (start/end treated as dates)
    #[arg(long)]
    pub all_day: bool,
    /// Duration syntax like 30m, 2h, 1h30m; ignored when --end is provided
    #[arg(long)]
    pub duration: Option<String>,
}

#[derive(Args)]
pub struct ListCommand {
    /// Filter by a specific day (UTC) e.g. 2025-06-01
    #[arg(long)]
    pub day: Option<String>,
    /// Timezone for display, e.g. Europe/Paris; defaults to local system zone
    #[arg(long = "tz")]
    pub tz: Option<String>,
}

#[derive(Args)]
pub struct RssCommand {
    /// Optional day filter (UTC)
    #[arg(long)]
    pub day: Option<String>,
    /// Override timezone used inside descriptions
    #[arg(long = "tz")]
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
    #[arg(long)]
    pub output: Option<PathBuf>,
}

#[derive(Args)]
pub struct IcalCommand {
    /// Optional day filter (UTC)
    #[arg(long)]
    pub day: Option<String>,
    /// Override timezone used for timed events
    #[arg(long = "tz")]
    pub tz: Option<String>,
    /// Write ICS to this file instead of stdout
    #[arg(long)]
    pub output: Option<PathBuf>,
}
