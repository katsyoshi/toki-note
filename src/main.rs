use std::{fs, path::PathBuf};

use anyhow::{Context, Result, anyhow};
use chrono::{DateTime, Local, NaiveDate, Utc};
use chrono_tz::Tz;
use clap::{Args, Parser, Subcommand};
use directories::ProjectDirs;
use humantime::parse_duration;
use rusqlite::{Connection, params};
use serde::Deserialize;

fn main() -> Result<()> {
    let cli = Cli::parse();
    let config = load_config()?;
    let db_path = resolve_database_path(cli.database.or(config.database))?;
    let mut storage = Storage::new(&db_path)?;

    match cli.command {
        Command::Add(cmd) => add_event(&mut storage, cmd),
        Command::List(cmd) => list_events(&storage, cmd),
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

fn list_events(storage: &Storage, cmd: ListCommand) -> Result<()> {
    let range = if let Some(day) = cmd.day {
        Some(day_range(&day)?)
    } else {
        None
    };
    let events = storage.fetch_events(range)?;
    let tz = parse_timezone(cmd.tz.as_deref())?;

    if events.is_empty() {
        println!("No events found");
        return Ok(());
    }

    for event in events {
        let timing = format_event_timing(&event, &tz)?;
        println!("#{} {}", event.id, event.title);
        println!("  {timing}");
        if !event.tags.is_empty() {
            println!("  tags: {}", event.tags.join(", "));
        }
        if !event.note.is_empty() {
            println!("  note: {}", event.note);
        }
        println!();
    }

    Ok(())
}

fn format_event_timing(event: &Event, zone: &DisplayZone) -> Result<String> {
    let start_utc = parse_utc(&event.starts_at)?;
    let end_utc = parse_utc(&event.ends_at)?;
    match zone {
        DisplayZone::Local => {
            if event.all_day {
                let start = start_utc.with_timezone(&Local);
                let end = end_utc.with_timezone(&Local);
                let end_inclusive = end
                    .date_naive()
                    .pred_opt()
                    .unwrap_or_else(|| end.date_naive());
                Ok(format!(
                    "{} -> {} (all-day, local)",
                    start.date_naive(),
                    end_inclusive
                ))
            } else {
                let start = start_utc.with_timezone(&Local);
                let end = end_utc.with_timezone(&Local);
                Ok(format!(
                    "{} -> {} ({})",
                    start.format("%Y-%m-%d %H:%M %Z"),
                    end.format("%Y-%m-%d %H:%M %Z"),
                    start.offset()
                ))
            }
        }
        DisplayZone::Named(tz) => {
            if event.all_day {
                let start = start_utc.with_timezone(tz);
                let end = end_utc.with_timezone(tz);
                let end_inclusive = end
                    .date_naive()
                    .pred_opt()
                    .unwrap_or_else(|| end.date_naive());
                Ok(format!(
                    "{} -> {} (all-day, {})",
                    start.date_naive(),
                    end_inclusive,
                    tz
                ))
            } else {
                let start = start_utc.with_timezone(tz);
                let end = end_utc.with_timezone(tz);
                Ok(format!(
                    "{} -> {} ({})",
                    start.format("%Y-%m-%d %H:%M %Z"),
                    end.format("%Y-%m-%d %H:%M %Z"),
                    tz
                ))
            }
        }
    }
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

fn day_range(day: &str) -> Result<(String, String)> {
    let date = parse_date(day)?;
    let start = date
        .and_hms_opt(0, 0, 0)
        .ok_or_else(|| anyhow!("invalid day"))?
        .and_utc();
    let end = date
        .succ_opt()
        .ok_or_else(|| anyhow!("date overflow"))?
        .and_hms_opt(0, 0, 0)
        .ok_or_else(|| anyhow!("invalid day"))?
        .and_utc();
    Ok((start.to_rfc3339(), end.to_rfc3339()))
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

fn parse_utc(value: &str) -> Result<DateTime<Utc>> {
    Ok(DateTime::parse_from_rfc3339(value)
        .with_context(|| format!("invalid timestamp '{value}'"))?
        .with_timezone(&Utc))
}

fn parse_timezone(input: Option<&str>) -> Result<DisplayZone> {
    if let Some(value) = input {
        let tz = value
            .parse::<Tz>()
            .map_err(|_| anyhow!("unknown timezone '{value}'"))?;
        Ok(DisplayZone::Named(tz))
    } else {
        Ok(DisplayZone::Local)
    }
}

fn resolve_database_path(input: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(path) = input {
        return Ok(path);
    }

    if let Some(project_dirs) = ProjectDirs::from("dev", "toki-note", "toki-note") {
        let mut path = project_dirs.data_dir().to_path_buf();
        path.push("toki-note.db");
        Ok(path)
    } else {
        Ok(PathBuf::from("toki-note.db"))
    }
}

fn load_config() -> Result<Config> {
    if let Some(project_dirs) = ProjectDirs::from("dev", "toki-note", "toki-note") {
        let path = project_dirs.config_dir().join("config.toml");
        if path.exists() {
            let contents = fs::read_to_string(&path)
                .with_context(|| format!("failed to read config {}", path.display()))?;
            let cfg: Config = toml::from_str(&contents)
                .with_context(|| format!("failed to parse {}", path.display()))?;
            return Ok(cfg);
        }
    }
    Ok(Config::default())
}

#[derive(Parser)]
#[command(version, about = "CLI scheduler backed by SQLite")]
struct Cli {
    /// Path to the SQLite database file
    #[arg(long, global = true)]
    database: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Clone, Debug, Default, Deserialize)]
struct Config {
    database: Option<PathBuf>,
}

enum DisplayZone {
    Local,
    Named(Tz),
}

#[derive(Subcommand)]
enum Command {
    /// Add a schedule entry
    Add(AddCommand),
    /// List stored schedule entries
    List(ListCommand),
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

#[derive(Args)]
struct ListCommand {
    /// Filter by a specific day (UTC) e.g. 2025-06-01
    #[arg(long)]
    day: Option<String>,
    /// Timezone for display, e.g. Europe/Paris; defaults to local system zone
    #[arg(long = "tz")]
    tz: Option<String>,
}

struct Storage {
    conn: Connection,
}

impl Storage {
    fn new(path: &PathBuf) -> Result<Self> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("failed to create {}", parent.display()))?;
            }
        }

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

    fn fetch_events(&self, day_range: Option<(String, String)>) -> Result<Vec<Event>> {
        let sql = if day_range.is_some() {
            "SELECT id, title, starts_at, ends_at, note, all_day FROM events \
             WHERE starts_at < ?2 AND ends_at > ?1 ORDER BY starts_at"
        } else {
            "SELECT id, title, starts_at, ends_at, note, all_day FROM events \
             ORDER BY starts_at"
        };

        let mut stmt = self.conn.prepare(sql)?;
        let mut rows = if let Some((start, end)) = day_range {
            stmt.query(params![start, end])?
        } else {
            stmt.query([])?
        };

        let mut events = Vec::new();
        let mut tag_stmt = self
            .conn
            .prepare("SELECT tag FROM event_tags WHERE event_id = ?1 ORDER BY tag")?;

        while let Some(row) = rows.next()? {
            let mut event = Event {
                id: row.get(0)?,
                title: row.get(1)?,
                starts_at: row.get(2)?,
                ends_at: row.get(3)?,
                note: row.get(4)?,
                all_day: row.get::<_, i64>(5)? != 0,
                tags: Vec::new(),
            };

            let tag_rows = tag_stmt.query_map(params![event.id], |tag_row| tag_row.get(0))?;
            for tag in tag_rows {
                event.tags.push(tag?);
            }

            events.push(event);
        }

        Ok(events)
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

struct Event {
    id: i64,
    title: String,
    starts_at: String,
    ends_at: String,
    note: String,
    all_day: bool,
    tags: Vec<String>,
}
