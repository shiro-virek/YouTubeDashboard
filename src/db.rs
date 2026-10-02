//! SQLite persistence. Everything is stored in a single file that lives next to
//! the executable when possible, which is what makes the app portable.

use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::{params, Connection, Result, Transaction};

use crate::model::Channel;

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS channels (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    name       TEXT    NOT NULL,
    url        TEXT    NOT NULL,
    position   INTEGER NOT NULL,
    created_at TEXT    NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS tags (
    id   INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE COLLATE NOCASE
);

CREATE TABLE IF NOT EXISTS channel_tags (
    channel_id INTEGER NOT NULL REFERENCES channels(id) ON DELETE CASCADE,
    tag_id     INTEGER NOT NULL REFERENCES tags(id)     ON DELETE CASCADE,
    PRIMARY KEY (channel_id, tag_id)
);

CREATE INDEX IF NOT EXISTS idx_channels_position ON channels(position);
CREATE INDEX IF NOT EXISTS idx_channel_tags_tag  ON channel_tags(tag_id);
CREATE INDEX IF NOT EXISTS idx_channel_tags_channel ON channel_tags(channel_id);
"#;

const SNAPSHOT: &str = "SELECT id, name, url, position FROM channels ORDER BY position, id";

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent).ok();
            }
        }
        Self::init(Connection::open(path)?)
    }

    #[cfg(test)]
    pub fn open_in_memory() -> Self {
        Self::init(Connection::open_in_memory().expect("in-memory database"))
            .expect("in-memory schema")
    }

    fn init(conn: Connection) -> Result<Self> {
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA foreign_keys = ON;",
        )?;

        let db = Self { conn };
        db.conn.execute_batch(SCHEMA)?;
        Ok(db)
    }

    pub fn snapshot(&self) -> Result<Vec<Channel>> {
        let mut stmt = self.conn.prepare(SNAPSHOT)?;
        let rows = stmt.query_map([], |row| {
            Ok(Channel {
                id: row.get(0)?,
                name: row.get(1)?,
                url: row.get(2)?,
                position: row.get(3)?,
                tags: Vec::new(),
            })
        })?;

        let mut channels = Vec::new();
        for row in rows {
            channels.push(row?);
        }

        let by_channel = self.tags_by_channel()?;
        for channel in &mut channels {
            if let Some(tags) = by_channel.get(&channel.id) {
                channel.tags = tags.clone();
            }
        }

        Ok(channels)
    }

    pub fn tags_by_channel(&self) -> Result<HashMap<i64, Vec<String>>> {
        let mut stmt = self.conn.prepare(
            "SELECT ct.channel_id, t.name
               FROM channel_tags ct
               JOIN tags t ON t.id = ct.tag_id
              ORDER BY t.name COLLATE NOCASE",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?;

        let mut map: HashMap<i64, Vec<String>> = HashMap::new();
        for row in rows {
            let (channel_id, name) = row?;
            map.entry(channel_id).or_default().push(name);
        }
        Ok(map)
    }

    pub fn all_tags(&self) -> Result<Vec<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT name FROM tags ORDER BY name COLLATE NOCASE")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        rows.collect()
    }

    pub fn insert_channel(&self, name: &str, url: &str, tags: &[String]) -> Result<i64> {
        let tx = self.conn.unchecked_transaction()?;
        let position: i64 = tx.query_row(
            "SELECT COALESCE(MAX(position), -1) + 1 FROM channels",
            [],
            |row| row.get(0),
        )?;

        tx.execute(
            "INSERT INTO channels (name, url, position) VALUES (?1, ?2, ?3)",
            params![name, url, position],
        )?;
        let id = tx.last_insert_rowid();

        Self::sync_tags(&tx, id, tags)?;
        tx.commit()?;
        Ok(id)
    }

    pub fn update_channel(&self, id: i64, name: &str, url: &str, tags: &[String]) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        tx.execute(
            "UPDATE channels SET name = ?2, url = ?3 WHERE id = ?1",
            params![id, name, url],
        )?;
        Self::sync_tags(&tx, id, tags)?;
        tx.commit()?;
        Ok(())
    }

    pub fn delete_channel(&self, id: i64) -> Result<()> {
        self.conn
            .execute("DELETE FROM channels WHERE id = ?1", params![id])?;
        Ok(())
    }

    /// Persists a brand new ordering for every channel in one transaction.
    pub fn set_order(&self, ids: &[i64]) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        {
            let mut stmt = tx.prepare("UPDATE channels SET position = ?1 WHERE id = ?2")?;
            for (position, id) in ids.iter().enumerate() {
                stmt.execute(params![position as i64, id])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Drops tags that no channel references anymore.
    pub fn prune_tags(&self) -> Result<()> {
        self.conn.execute(
            "DELETE FROM tags WHERE id NOT IN (SELECT tag_id FROM channel_tags)",
            [],
        )?;
        Ok(())
    }

    /// Folds the write-ahead log back into the main database file.
    pub fn checkpoint(&self) {
        self.conn
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
            .ok();
    }

    fn sync_tags(tx: &Transaction<'_>, channel_id: i64, tags: &[String]) -> Result<()> {
        tx.execute(
            "DELETE FROM channel_tags WHERE channel_id = ?1",
            params![channel_id],
        )?;
        for tag in tags {
            tx.execute(
                "INSERT OR IGNORE INTO tags (name) VALUES (?1)",
                params![tag],
            )?;
            let tag_id: i64 = tx.query_row(
                "SELECT id FROM tags WHERE name = ?1 COLLATE NOCASE",
                params![tag],
                |row| row.get(0),
            )?;
            tx.execute(
                "INSERT OR IGNORE INTO channel_tags (channel_id, tag_id) VALUES (?1, ?2)",
                params![channel_id, tag_id],
            )?;
        }
        Ok(())
    }
}

/// Where the database lives.
///
/// Portable by default: the file sits next to the executable so the whole app
/// can live on a USB stick. `YTDASH_DB` or `--db` win over that, and when the
/// executable directory is read-only we fall back to the XDG data directory.
pub fn resolve_db_path(explicit: Option<PathBuf>) -> PathBuf {
    if let Some(path) = explicit {
        return path;
    }
    if let Some(path) = env::var_os("YTDASH_DB").filter(|value| !value.is_empty()) {
        return PathBuf::from(path);
    }
    if let Some(dir) = portable_dir() {
        return dir.join("ytdash.db");
    }
    data_dir().join("ytdash.db")
}

fn portable_dir() -> Option<PathBuf> {
    let exe = env::current_exe().ok()?;
    let dir = exe.parent()?.to_path_buf();
    if is_writable(&dir) {
        Some(dir)
    } else {
        None
    }
}

fn data_dir() -> PathBuf {
    if let Some(base) = env::var_os("XDG_DATA_HOME").filter(|value| !value.is_empty()) {
        return PathBuf::from(base).join("ytdash");
    }
    env::var_os("HOME").map_or_else(
        || PathBuf::from("."),
        |home| PathBuf::from(home).join(".local/share/ytdash"),
    )
}

fn is_writable(dir: &Path) -> bool {
    if !dir.is_dir() {
        return false;
    }
    let probe = dir.join(".ytdash-write-test");
    match fs::write(&probe, b"") {
        Ok(()) => {
            fs::remove_file(&probe).ok();
            true
        }
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn memory_db() -> Database {
        Database::open_in_memory()
    }

    #[test]
    fn insert_update_delete_roundtrip() {
        let db = memory_db();

        let id = db
            .insert_channel(
                "Rust",
                "https://www.youtube.com/@rust-lang",
                &["dev".into(), "news".into()],
            )
            .unwrap();
        assert!(id > 0);

        let channels = db.snapshot().unwrap();
        assert_eq!(channels.len(), 1);
        assert_eq!(channels[0].name, "Rust");
        assert_eq!(channels[0].tags, vec!["dev", "news"]);
        assert_eq!(channels[0].position, 0);

        db.update_channel(
            id,
            "Rust Lang",
            "https://www.youtube.com/@rustlang",
            &["dev".into()],
        )
        .unwrap();
        let channels = db.snapshot().unwrap();
        assert_eq!(channels[0].name, "Rust Lang");
        assert_eq!(channels[0].tags, vec!["dev"]);

        db.delete_channel(id).unwrap();
        assert!(db.snapshot().unwrap().is_empty());

        // Tags survive the channel, so pruning is what removes them.
        assert_eq!(db.all_tags().unwrap().len(), 2);
        db.prune_tags().unwrap();
        assert!(db.all_tags().unwrap().is_empty());
    }

    #[test]
    fn tags_are_case_insensitively_unique() {
        let db = memory_db();
        let first = db
            .insert_channel("A", "https://www.youtube.com/@a", &["Tech".into()])
            .unwrap();
        let second = db
            .insert_channel("B", "https://www.youtube.com/@b", &["tech".into()])
            .unwrap();

        let tags = db.all_tags().unwrap();
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0], "Tech");

        let channels = db.snapshot().unwrap();
        let by_id = |id: i64| channels.iter().find(|c| c.id == id).unwrap().tags.clone();
        assert_eq!(by_id(first), vec!["Tech"]);
        assert_eq!(by_id(second), vec!["Tech"]);
    }

    #[test]
    fn order_is_persisted() {
        let db = memory_db();
        let a = db
            .insert_channel("A", "https://www.youtube.com/@a", &[])
            .unwrap();
        let b = db
            .insert_channel("B", "https://www.youtube.com/@b", &[])
            .unwrap();
        let c = db
            .insert_channel("C", "https://www.youtube.com/@c", &[])
            .unwrap();

        db.set_order(&[c, a, b]).unwrap();

        let order: Vec<i64> = db.snapshot().unwrap().iter().map(|c| c.id).collect();
        assert_eq!(order, vec![c, a, b]);
    }

    #[test]
    fn db_path_prefers_explicit_argument() {
        let explicit = PathBuf::from("/tmp/explicit.db");
        assert_eq!(resolve_db_path(Some(explicit.clone())), explicit);
    }
}
