use dashmap::DashMap;
use parking_lot::Mutex;
use rusqlite::{params, Connection};
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Clone)]
pub struct Db {
    path: PathBuf,
    conn: Arc<Mutex<Connection>>,
}

impl Db {
    pub fn init_default() -> Self {
        let home_dir = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let data_dir = PathBuf::from(home_dir).join(".machunt").join("data");
        let _ = fs::create_dir_all(&data_dir);
        let db_path = data_dir.join("index.db");

        let conn = Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "
            PRAGMA journal_mode=WAL;
            PRAGMA synchronous=NORMAL;
            PRAGMA cache_size=-65536;
            PRAGMA temp_store=MEMORY;
            PRAGMA mmap_size=268435456;
            CREATE TABLE IF NOT EXISTS files (
                id   INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                path TEXT NOT NULL UNIQUE
            );

            CREATE TABLE IF NOT EXISTS meta (
                key   TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
        ",
        )
        .unwrap();
        // The runtime search path is in-memory; this DB index is not needed and costs disk space.
        let _ = conn.execute("DROP INDEX IF EXISTS idx_name", []);

        Self {
            path: db_path,
            conn: Arc::new(Mutex::new(conn)),
        }
    }

    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    pub fn clear_files(&self) {
        let conn = self.conn.lock();
        let _ = conn.execute("DELETE FROM files", []);
    }

    pub fn insert(&self, name: &str, path: &Path) {
        let conn = self.conn.lock();
        let _ = conn.execute(
            "INSERT OR IGNORE INTO files (name, path) VALUES (?1, ?2)",
            params![name, path.to_string_lossy().as_ref()],
        );
    }

    pub fn delete(&self, path: &Path) {
        let conn = self.conn.lock();
        let _ = conn.execute(
            "DELETE FROM files WHERE path = ?1",
            params![path.to_string_lossy().as_ref()],
        );
    }

    pub fn insert_batch(&self, entries: &[(String, PathBuf)]) {
        if entries.is_empty() {
            return;
        }

        let mut conn = self.conn.lock();
        let _ = conn.execute_batch("PRAGMA synchronous=OFF;");
        let tx = conn.transaction().unwrap();
        {
            let mut stmt = tx
                .prepare_cached("INSERT OR IGNORE INTO files (name, path) VALUES (?1, ?2)")
                .unwrap();
            for (name, path) in entries {
                let _ = stmt.execute(params![name, path.to_string_lossy().as_ref()]);
            }
        }
        tx.commit().unwrap();
        let _ = conn.execute_batch("PRAGMA synchronous=NORMAL;");
    }

    pub fn load_index(&self, index: &Arc<DashMap<String, Vec<PathBuf>>>) -> usize {
        if !self.path.exists() {
            return 0;
        }

        index.clear();

        let conn = self.conn.lock();
        let mut stmt = conn.prepare("SELECT name, path FROM files").unwrap();
        let rows = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .unwrap();

        let mut count = 0usize;
        for row in rows.flatten() {
            let (name, path_str) = row;
            index.entry(name).or_default().push(PathBuf::from(path_str));
            count += 1;
        }
        count
    }

    pub fn has_any_files(&self) -> bool {
        let conn = self.conn.lock();
        conn.query_row("SELECT EXISTS(SELECT 1 FROM files LIMIT 1)", [], |row| {
            row.get::<_, i64>(0)
        })
        .unwrap_or(0)
            != 0
    }

    pub fn list_all_paths(&self) -> Vec<(String, String)> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare("SELECT name, path FROM files").unwrap();
        stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
            .unwrap()
            .flatten()
            .collect()
    }

    pub fn save_last_event_id(&self, event_id: u64) {
        let conn = self.conn.lock();
        let _ = conn.execute(
            "INSERT OR REPLACE INTO meta (key, value) VALUES ('last_event_id', ?1)",
            params![event_id.to_string()],
        );
    }

    pub fn load_last_event_id(&self) -> Option<u64> {
        let conn = self.conn.lock();
        conn.query_row(
            "SELECT value FROM meta WHERE key = 'last_event_id'",
            [],
            |row| row.get::<_, String>(0),
        )
        .ok()
        .and_then(|s| s.parse().ok())
    }

    pub fn save_include_dirs(&self, include_dirs: bool) {
        let conn = self.conn.lock();
        let value = if include_dirs { "1" } else { "0" };
        let _ = conn.execute(
            "INSERT OR REPLACE INTO meta (key, value) VALUES ('include_dirs', ?1)",
            params![value],
        );
    }

    pub fn load_include_dirs(&self) -> Option<bool> {
        let conn = self.conn.lock();
        conn.query_row("SELECT value FROM meta WHERE key = 'include_dirs'", [], |row| {
            row.get::<_, String>(0)
        })
        .ok()
        .map(|v| v == "1")
    }

    pub fn checkpoint_truncate(&self) {
        let conn = self.conn.lock();
        let _ = conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);");
    }

    pub fn vacuum(&self) {
        let conn = self.conn.lock();
        let _ = conn.execute_batch("VACUUM;");
    }
}
