use std::path::PathBuf;

use anyhow::{Context, Result, anyhow};
use chrono::{DateTime, NaiveDate, Utc};
use clap::{Args, Parser, Subcommand};
use humantime::parse_duration;
use rusqlite::{Connection, params};

fn main() -> Result<()> {
    let cli = Cli::parse();
    let mut storage = Storage::new(&cli.database)?;

    match cli.command {
        Command::Add(cmd) => add_event(&mut storage, cmd),
    }
}

fn add_event(storage: &mut Storage, cmd: AddCommand) -> Result<()> {
    let timing = if cmd.all_day {
        if cmd.duration.is_some() {
            return Err(anyhow!("--duration cannot be used with --all-day"));
        }
        parse_all_day_range(&cmd)?
    } else {
        parse_timed_range(&cmd)?
    };

    let new_event = NewEvent {
        title: cmd.title,
        note: cmd.note.unwrap_or_default(),
        starts_at: timing.starts_at,
        ends_at: timing.ends_at,
        all_day: cmd.all_day,
        tags: cmd.tags,
    };

    let row_id = storage.insert_event(new_event)?;
    println!("Stored event #{row_id}");
    Ok(())
}

fn parse_all_day_range(cmd: &AddCommand) -> Result<EventTiming> {
    let start_date = parse_date(&cmd.start)?;
    let end_date = if let Some(end) = cmd.end.as_deref() {
        parse_date(end)?
    } else {
        start_date
    };

    let start_dt = start_date
        .and_hms_opt(0, 0, 0)
        .ok_or_else(|| anyhow!("invalid start date"))?
        .and_utc();

    let end_dt = end_date
        .succ_opt()
        .ok_or_else(|| anyhow!("date overflow"))?
        .and_hms_opt(0, 0, 0)
        .ok_or_else(|| anyhow!("invalid end date"))?
        .and_utc();

    Ok(EventTiming {
        starts_at: start_dt.to_rfc3339(),
        ends_at: end_dt.to_rfc3339(),
    })
}

fn parse_date(input: &str) -> Result<NaiveDate> {
    NaiveDate::parse_from_str(input, "%Y-%m-%d")
        .with_context(|| format!("expected YYYY-MM-DD date, got '{input}'"))
}

fn parse_timed_range(cmd: &AddCommand) -> Result<EventTiming> {
    let start_dt = DateTime::parse_from_rfc3339(&cmd.start)
        .with_context(|| format!("expected RFC3339 timestamp, got '{}'", cmd.start))?
        .with_timezone(&Utc);

    let end_dt = if let Some(end_value) = cmd.end.as_deref() {
        DateTime::parse_from_rfc3339(end_value)
            .with_context(|| format!("expected RFC3339 timestamp, got '{end_value}'"))?
            .with_timezone(&Utc)
    } else if let Some(duration_value) = cmd.duration.as_deref() {
        let parsed = parse_duration(duration_value)
            .with_context(|| format!("failed to parse duration '{duration_value}'"))?;
        let chrono_dur = chrono::Duration::from_std(parsed)
            .map_err(|_| anyhow!("duration '{duration_value}' is too large"))?;
        start_dt
            .checked_add_signed(chrono_dur)
            .ok_or_else(|| anyhow!("duration pushes end time out of range"))?
    } else {
        return Err(anyhow!(
            "provide either --end or --duration (or --all-day for date-based events)"
        ));
    };

    if end_dt <= start_dt {
        return Err(anyhow!("--end must be later than --start"));
    }

    Ok(EventTiming {
        starts_at: start_dt.to_rfc3339(),
        ends_at: end_dt.to_rfc3339(),
    })
}

struct EventTiming {
    starts_at: String,
    ends_at: String,
}

#[derive(Parser)]
#[command(version, about = "CLI scheduler backed by SQLite")]
struct Cli {
    /// Path to the SQLite database file
    #[arg(long, default_value = "toki-note.db", global = true)]
    database: PathBuf,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Add a schedule entry
    Add(AddCommand),
}

#[derive(Args)]
struct AddCommand {
    /// Event title
    #[arg(long)]
    title: String,
    /// Start instant in RFC3339 (e.g. 2024-06-01T09:00:00+09:00) or YYYY-MM-DD when --all-day is set
    #[arg(long)]
    start: String,
    /// End instant in RFC3339; optional for --all-day
    #[arg(long)]
    end: Option<String>,
    /// Optional note or description
    #[arg(long)]
    note: Option<String>,
    /// Repeatable tag values (e.g. --tag work --tag urgent)
    #[arg(long = "tag", action = clap::ArgAction::Append)]
    tags: Vec<String>,
    /// Store event as all-day entry (start/end treated as dates)
    #[arg(long)]
    all_day: bool,
    /// Duration syntax like 30m, 2h, 1h30m; ignored when --end is provided
    #[arg(long)]
    duration: Option<String>,
}

struct Storage {
    conn: Connection,
}

impl Storage {
    fn new(path: &PathBuf) -> Result<Self> {
        let conn = Connection::open(path)
            .with_context(|| format!("failed to open database at {}", path.display()))?;
        let storage = Self { conn };
        storage.init_schema()?;
        Ok(storage)
    }

    fn init_schema(&self) -> Result<()> {
        self.conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                title TEXT NOT NULL,
                starts_at TEXT NOT NULL,
                ends_at TEXT NOT NULL,
                note TEXT NOT NULL DEFAULT '',
                all_day INTEGER NOT NULL DEFAULT 0
            );
            CREATE TABLE IF NOT EXISTS event_tags (
                event_id INTEGER NOT NULL,
                tag TEXT NOT NULL,
                UNIQUE (event_id, tag),
                FOREIGN KEY (event_id) REFERENCES events(id) ON DELETE CASCADE
            );
            "#,
        )?;
        Ok(())
    }

    fn insert_event(&mut self, new_event: NewEvent) -> Result<i64> {
        let tx = self.conn.transaction()?;
        tx.execute(
            "INSERT INTO events (title, starts_at, ends_at, note, all_day) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                new_event.title,
                new_event.starts_at,
                new_event.ends_at,
                new_event.note,
                new_event.all_day as i32,
            ],
        )?;
        let id = tx.last_insert_rowid();
        for tag in new_event.tags {
            let tag_value = tag.to_lowercase();
            tx.execute(
                "INSERT OR IGNORE INTO event_tags (event_id, tag) VALUES (?1, ?2)",
                params![id, tag_value],
            )?;
        }
        tx.commit()?;
        Ok(id)
    }
}

struct NewEvent {
    title: String,
    note: String,
    starts_at: String,
    ends_at: String,
    all_day: bool,
    tags: Vec<String>,
}
