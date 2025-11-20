use std::{fs, path::PathBuf};

use anyhow::{Context, Result};
use rusqlite::{Connection, params};

pub struct Storage {
    conn: Connection,
}

impl Storage {
    pub fn new(path: &PathBuf) -> Result<Self> {
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

    pub fn insert_event(&mut self, new_event: NewEvent) -> Result<i64> {
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

    pub fn delete_by_id(&mut self, id: i64) -> Result<bool> {
        let affected = self
            .conn
            .execute("DELETE FROM events WHERE id = ?1", params![id])?;
        Ok(affected > 0)
    }

    pub fn delete_by_title(&mut self, title: &str) -> Result<usize> {
        let affected = self
            .conn
            .execute("DELETE FROM events WHERE title = ?1", params![title])?;
        Ok(affected as usize)
    }

    pub fn fetch_events(&self, day_range: Option<(String, String)>) -> Result<Vec<StoredEvent>> {
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
            let mut event = StoredEvent {
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

pub struct NewEvent {
    pub title: String,
    pub note: String,
    pub starts_at: String,
    pub ends_at: String,
    pub all_day: bool,
    pub tags: Vec<String>,
}

pub struct StoredEvent {
    pub id: i64,
    pub title: String,
    pub starts_at: String,
    pub ends_at: String,
    pub note: String,
    pub all_day: bool,
    pub tags: Vec<String>,
}
