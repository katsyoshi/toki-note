use std::{fs, path::PathBuf};

use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension, params};

pub struct Storage {
    conn: Connection,
}

impl Storage {
    pub fn new(path: &PathBuf) -> Result<Self> {
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
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
                all_day INTEGER NOT NULL DEFAULT 0,
                uid TEXT
            );
            CREATE TABLE IF NOT EXISTS event_tags (
                event_id INTEGER NOT NULL,
                tag TEXT NOT NULL,
                UNIQUE (event_id, tag),
                FOREIGN KEY (event_id) REFERENCES events(id) ON DELETE CASCADE
            );
            "#,
        )?;
        let _ = self
            .conn
            .execute("ALTER TABLE events ADD COLUMN uid TEXT", []);
        self.conn.execute(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_events_uid ON events(uid) WHERE uid IS NOT NULL",
            [],
        )?;
        Ok(())
    }

    pub fn fetch_event_by_id(&self, id: i64) -> Result<Option<StoredEvent>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, starts_at, ends_at, note, all_day, uid FROM events WHERE id = ?1",
        )?;
        let event = stmt
            .query_row(params![id], |row| {
                Ok(StoredEvent {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    starts_at: row.get(2)?,
                    ends_at: row.get(3)?,
                    note: row.get(4)?,
                    all_day: row.get::<_, i64>(5)? != 0,
                    uid: row.get(6)?,
                    tags: Vec::new(),
                })
            })
            .optional()?;
        if let Some(mut event) = event {
            event.tags = self.load_tags(event.id)?;
            Ok(Some(event))
        } else {
            Ok(None)
        }
    }

    pub fn fetch_events_by_title(&self, title: &str) -> Result<Vec<StoredEvent>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, starts_at, ends_at, note, all_day, uid FROM events \
             WHERE title = ?1 ORDER BY starts_at",
        )?;
        let mut rows = stmt.query(params![title])?;
        let mut events = Vec::new();
        while let Some(row) = rows.next()? {
            let mut event = StoredEvent {
                id: row.get(0)?,
                title: row.get(1)?,
                starts_at: row.get(2)?,
                ends_at: row.get(3)?,
                note: row.get(4)?,
                all_day: row.get::<_, i64>(5)? != 0,
                uid: row.get(6)?,
                tags: Vec::new(),
            };
            event.tags = self.load_tags(event.id)?;
            events.push(event);
        }
        Ok(events)
    }

    pub fn insert_event(&mut self, new_event: NewEvent) -> Result<i64> {
        let tx = self.conn.transaction()?;
        tx.execute(
            "INSERT INTO events (title, starts_at, ends_at, note, all_day, uid) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                new_event.title,
                new_event.starts_at,
                new_event.ends_at,
                new_event.note,
                new_event.all_day as i32,
                new_event.uid,
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
        Ok(affected)
    }

    pub fn has_event_with_uid(&self, uid: &str) -> Result<bool> {
        let exists: Option<i64> = self
            .conn
            .query_row(
                "SELECT 1 FROM events WHERE uid = ?1 LIMIT 1",
                params![uid],
                |row| row.get(0),
            )
            .optional()?;
        Ok(exists.is_some())
    }

    pub fn fetch_events(&self, day_range: Option<(String, String)>) -> Result<Vec<StoredEvent>> {
        let sql = if day_range.is_some() {
            "SELECT id, title, starts_at, ends_at, note, all_day, uid FROM events \
             WHERE starts_at < ?2 AND ends_at > ?1 ORDER BY starts_at"
        } else {
            "SELECT id, title, starts_at, ends_at, note, all_day, uid FROM events \
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
                uid: row.get(6)?,
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

    pub fn update_event_timing(
        &mut self,
        id: i64,
        starts_at: &str,
        ends_at: &str,
        all_day: bool,
    ) -> Result<bool> {
        let affected = self.conn.execute(
            "UPDATE events SET starts_at = ?1, ends_at = ?2, all_day = ?3 WHERE id = ?4",
            params![starts_at, ends_at, all_day as i32, id],
        )?;
        Ok(affected == 1)
    }

    fn load_tags(&self, event_id: i64) -> Result<Vec<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT tag FROM event_tags WHERE event_id = ?1 ORDER BY tag")?;
        let rows = stmt.query_map(params![event_id], |tag_row| tag_row.get(0))?;
        let mut tags = Vec::new();
        for tag in rows {
            tags.push(tag?);
        }
        Ok(tags)
    }
}

pub struct NewEvent {
    pub title: String,
    pub note: String,
    pub starts_at: String,
    pub ends_at: String,
    pub all_day: bool,
    pub tags: Vec<String>,
    pub uid: Option<String>,
}

pub struct StoredEvent {
    pub id: i64,
    pub title: String,
    pub starts_at: String,
    pub ends_at: String,
    pub note: String,
    pub all_day: bool,
    #[allow(dead_code)]
    pub uid: Option<String>,
    pub tags: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    struct TempStorage {
        _dir: tempfile::TempDir,
        storage: Storage,
    }

    impl TempStorage {
        fn new() -> Self {
            let dir = tempdir().expect("temp dir");
            let path = dir.path().join("db.sqlite");
            let storage = Storage::new(&path).expect("storage");
            Self { _dir: dir, storage }
        }
    }

    fn sample_event(title: &str, start: &str, end: &str) -> NewEvent {
        NewEvent {
            title: title.to_string(),
            note: String::new(),
            starts_at: start.to_string(),
            ends_at: end.to_string(),
            all_day: false,
            tags: Vec::new(),
            uid: None,
        }
    }

    #[test]
    fn insert_event_lowercases_and_deduplicates_tags() {
        let mut store = TempStorage::new();
        let mut event = sample_event(
            "Demo",
            "2025-01-01T09:00:00+00:00",
            "2025-01-01T10:00:00+00:00",
        );
        event.tags = vec!["Work".into(), "work".into(), "Home".into()];
        let id = store.storage.insert_event(event).unwrap();

        let events = store.storage.fetch_events(None).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].id, id);
        assert_eq!(events[0].tags, vec!["home", "work"]);
    }

    #[test]
    fn fetch_events_filters_by_day_range() {
        let mut store = TempStorage::new();
        let first = sample_event(
            "Inside",
            "2025-05-01T09:00:00+00:00",
            "2025-05-01T10:00:00+00:00",
        );
        let second = sample_event(
            "Outside",
            "2025-05-03T09:00:00+00:00",
            "2025-05-03T10:00:00+00:00",
        );
        store.storage.insert_event(first).unwrap();
        store.storage.insert_event(second).unwrap();

        let events = store
            .storage
            .fetch_events(Some((
                "2025-05-01T00:00:00+00:00".into(),
                "2025-05-02T00:00:00+00:00".into(),
            )))
            .unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].title, "Inside");
    }

    #[test]
    fn delete_by_title_removes_rows() {
        let mut store = TempStorage::new();
        let event_one = sample_event(
            "Repeat",
            "2025-01-01T09:00:00+00:00",
            "2025-01-01T10:00:00+00:00",
        );
        let event_two = sample_event(
            "Repeat",
            "2025-01-02T09:00:00+00:00",
            "2025-01-02T10:00:00+00:00",
        );
        store.storage.insert_event(event_one).unwrap();
        store.storage.insert_event(event_two).unwrap();

        let removed = store.storage.delete_by_title("Repeat").unwrap();
        assert_eq!(removed, 2);
        assert!(store.storage.fetch_events(None).unwrap().is_empty());
    }

    #[test]
    fn has_event_with_uid_detects_duplicates() {
        let mut store = TempStorage::new();
        let mut event = sample_event(
            "Has UID",
            "2025-01-01T09:00:00+00:00",
            "2025-01-01T10:00:00+00:00",
        );
        event.uid = Some("abc-123".into());
        store.storage.insert_event(event).unwrap();

        assert!(store.storage.has_event_with_uid("abc-123").unwrap());
        assert!(!store.storage.has_event_with_uid("missing").unwrap());
    }
}
