use dashmap::DashMap;
use parking_lot::Mutex;
use rusqlite::{params, Connection};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
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

        let mut conn = Connection::open(&db_path).unwrap();
        Self::apply_pragmas(&conn);
        Self::ensure_schema(&mut conn);

        Self {
            path: db_path,
            conn: Arc::new(Mutex::new(conn)),
        }
    }

    fn apply_pragmas(conn: &Connection) {
        conn.execute_batch(
            "
            PRAGMA journal_mode=WAL;
            PRAGMA synchronous=NORMAL;
            PRAGMA cache_size=-65536;
            PRAGMA temp_store=MEMORY;
            PRAGMA mmap_size=268435456;
            PRAGMA foreign_keys=ON;
        ",
        )
        .unwrap();
    }

    fn ensure_schema(conn: &mut Connection) {
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS meta (
                key   TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );
        ",
        )
        .unwrap();

        if !Self::table_exists(conn, "files") {
            Self::create_v2_tables(conn);
            Self::save_schema_version(conn, 2);
        } else if Self::files_has_column(conn, "path") {
            Self::migrate_v1_to_v2(conn);
        } else {
            Self::create_v2_tables(conn);
            Self::save_schema_version(conn, 2);
        }
        Self::ensure_name_lower_column(conn);

        // v1 leftover index; no longer used.
        let _ = conn.execute("DROP INDEX IF EXISTS idx_name", []);
    }

    fn table_exists(conn: &Connection, table_name: &str) -> bool {
        conn.query_row(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1 LIMIT 1",
            params![table_name],
            |_| Ok(()),
        )
        .is_ok()
    }

    fn files_has_column(conn: &Connection, column_name: &str) -> bool {
        let mut stmt = conn.prepare("PRAGMA table_info(files)").unwrap();
        let names = stmt
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .flatten()
            .collect::<Vec<_>>();
        names.into_iter().any(|name| name == column_name)
    }

    fn create_v2_tables(conn: &Connection) {
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS dirs (
                id   INTEGER PRIMARY KEY,
                path TEXT NOT NULL UNIQUE
            );

            CREATE TABLE IF NOT EXISTS files (
                id     INTEGER PRIMARY KEY,
                name   TEXT NOT NULL,
                name_lower TEXT NOT NULL,
                dir_id INTEGER NOT NULL REFERENCES dirs(id) ON DELETE CASCADE,
                UNIQUE(dir_id, name)
            );
        ",
        )
        .unwrap();
    }

    fn ensure_name_lower_column(conn: &Connection) {
        let mut needs_backfill = false;
        if !Self::files_has_column(conn, "name_lower") {
            let _ = conn.execute("ALTER TABLE files ADD COLUMN name_lower TEXT", []);
            needs_backfill = true;
        }

        let backfill_done = conn
            .query_row(
                "SELECT value FROM meta WHERE key = 'name_lower_backfill_done'",
                [],
                |row| row.get::<_, String>(0),
            )
            .map(|v| v == "1")
            .unwrap_or(false);

        if needs_backfill || !backfill_done {
            let _ = conn.execute(
                "UPDATE files SET name_lower = LOWER(name) WHERE name_lower IS NULL OR name_lower = ''",
                [],
            );
            let _ = conn.execute(
                "INSERT OR REPLACE INTO meta (key, value) VALUES ('name_lower_backfill_done', '1')",
                [],
            );
        }
    }

    fn save_schema_version(conn: &Connection, version: i64) {
        let _ = conn.execute(
            "INSERT OR REPLACE INTO meta (key, value) VALUES ('schema_version', ?1)",
            params![version.to_string()],
        );
    }

    fn parent_key(path: &Path) -> String {
        path.parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default()
    }

    fn parent_key_from_str(path: &str) -> String {
        Path::new(path)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default()
    }

    fn derive_name(path: &Path, fallback: &str) -> String {
        path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(fallback)
            .to_string()
    }

    fn migrate_v1_to_v2(conn: &mut Connection) {
        println!("Migrating index database schema to v2 (dirs/files)...");

        let tx = conn.transaction().unwrap();
        tx.execute_batch(
            "
            DROP TABLE IF EXISTS files_v2;
            DROP TABLE IF EXISTS dirs;

            CREATE TABLE dirs (
                id   INTEGER PRIMARY KEY,
                path TEXT NOT NULL UNIQUE
            );

            CREATE TABLE files_v2 (
                id     INTEGER PRIMARY KEY,
                name   TEXT NOT NULL,
                name_lower TEXT NOT NULL,
                dir_id INTEGER NOT NULL REFERENCES dirs(id) ON DELETE CASCADE,
                UNIQUE(dir_id, name)
            );
        ",
        )
        .unwrap();

        let mut select_stmt = tx.prepare("SELECT path FROM files").unwrap();
        let mut rows = select_stmt.query([]).unwrap();

        let mut insert_dir_stmt = tx
            .prepare_cached("INSERT OR IGNORE INTO dirs (path) VALUES (?1)")
            .unwrap();
        let mut select_dir_stmt = tx
            .prepare_cached("SELECT id FROM dirs WHERE path = ?1")
            .unwrap();
        let mut insert_file_stmt = tx
            .prepare_cached(
                "INSERT OR IGNORE INTO files_v2 (name, name_lower, dir_id) VALUES (?1, ?2, ?3)",
            )
            .unwrap();

        let mut dir_cache: HashMap<String, i64> = HashMap::new();
        let mut migrated = 0usize;

        while let Some(row) = rows.next().unwrap() {
            let path_str: String = match row.get(0) {
                Ok(v) => v,
                Err(_) => continue,
            };
            if path_str.is_empty() {
                continue;
            }

            let path = Path::new(&path_str);
            let name = match path.file_name().and_then(|n| n.to_str()) {
                Some(v) => v,
                None => continue,
            };
            let name_lower = name.to_lowercase();
            let dir_path = Self::parent_key_from_str(&path_str);

            let dir_id = if let Some(id) = dir_cache.get(&dir_path) {
                *id
            } else {
                let _ = insert_dir_stmt.execute(params![dir_path.as_str()]);
                let id: i64 =
                    match select_dir_stmt.query_row(params![dir_path.as_str()], |r| r.get(0)) {
                        Ok(v) => v,
                        Err(_) => continue,
                    };
                dir_cache.insert(dir_path.clone(), id);
                id
            };

            let _ = insert_file_stmt.execute(params![name, name_lower, dir_id]);
            migrated += 1;

            if migrated.is_multiple_of(500_000) {
                println!("Migrated {} rows...", migrated);
            }
        }

        drop(rows);
        drop(select_stmt);
        drop(insert_file_stmt);
        drop(select_dir_stmt);
        drop(insert_dir_stmt);

        tx.execute("DROP TABLE files", []).unwrap();
        tx.execute("ALTER TABLE files_v2 RENAME TO files", [])
            .unwrap();
        let _ = tx.execute(
            "INSERT OR REPLACE INTO meta (key, value) VALUES ('schema_version', '2')",
            [],
        );
        tx.commit().unwrap();

        println!(
            "Schema migration to v2 finished, migrated {} rows",
            migrated
        );
    }

    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    pub fn clear_files(&self) {
        let conn = self.conn.lock();
        let _ = conn.execute_batch("DELETE FROM files; DELETE FROM dirs;");
    }

    pub fn insert(&self, fallback_name: &str, path: &Path) {
        let conn = self.conn.lock();
        let dir_path = Self::parent_key(path);
        let stored_name = Self::derive_name(path, fallback_name);
        let stored_name_lower = if fallback_name.is_empty() {
            stored_name.to_lowercase()
        } else {
            fallback_name.to_string()
        };
        if stored_name.is_empty() {
            return;
        }

        let _ = conn.execute(
            "INSERT OR IGNORE INTO dirs (path) VALUES (?1)",
            params![dir_path.as_str()],
        );

        let dir_id: Option<i64> = conn
            .query_row(
                "SELECT id FROM dirs WHERE path = ?1",
                params![dir_path.as_str()],
                |row| row.get(0),
            )
            .ok();

        if let Some(dir_id) = dir_id {
            let _ = conn.execute(
                "INSERT OR IGNORE INTO files (name, name_lower, dir_id) VALUES (?1, ?2, ?3)",
                params![stored_name, stored_name_lower, dir_id],
            );
        }
    }

    pub fn delete(&self, path: &Path) {
        let conn = self.conn.lock();
        let dir_path = Self::parent_key(path);
        let file_name = match path.file_name().and_then(|n| n.to_str()) {
            Some(v) => v,
            None => return,
        };

        let _ = conn.execute(
            "
            DELETE FROM files
            WHERE name = ?1
              AND dir_id = (SELECT id FROM dirs WHERE path = ?2)
        ",
            params![file_name, dir_path.as_str()],
        );

        let _ = conn.execute(
            "
            DELETE FROM dirs
            WHERE path = ?1
              AND NOT EXISTS (
                  SELECT 1 FROM files WHERE files.dir_id = dirs.id LIMIT 1
              )
        ",
            params![dir_path.as_str()],
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
            let mut insert_dir_stmt = tx
                .prepare_cached("INSERT OR IGNORE INTO dirs (path) VALUES (?1)")
                .unwrap();
            let mut select_dir_stmt = tx
                .prepare_cached("SELECT id FROM dirs WHERE path = ?1")
                .unwrap();
            let mut insert_file_stmt = tx
                .prepare_cached(
                    "INSERT OR IGNORE INTO files (name, name_lower, dir_id) VALUES (?1, ?2, ?3)",
                )
                .unwrap();

            let mut dir_cache: HashMap<String, i64> = HashMap::new();

            for (fallback_name, path) in entries {
                let stored_name = Self::derive_name(path, fallback_name);
                let stored_name_lower = if fallback_name.is_empty() {
                    stored_name.to_lowercase()
                } else {
                    fallback_name.clone()
                };
                if stored_name.is_empty() {
                    continue;
                }

                let dir_path = Self::parent_key(path.as_path());
                let dir_id = if let Some(id) = dir_cache.get(&dir_path) {
                    *id
                } else {
                    let _ = insert_dir_stmt.execute(params![dir_path.as_str()]);
                    let id: i64 =
                        match select_dir_stmt.query_row(params![dir_path.as_str()], |r| r.get(0)) {
                            Ok(v) => v,
                            Err(_) => continue,
                        };
                    dir_cache.insert(dir_path.clone(), id);
                    id
                };

                let _ = insert_file_stmt.execute(params![stored_name, stored_name_lower, dir_id]);
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
        let mut stmt = conn
            .prepare(
                "
                SELECT
                    f.name_lower,
                    CASE
                        WHEN d.path = '' THEN f.name
                        WHEN d.path = '/' THEN '/' || f.name
                        ELSE d.path || '/' || f.name
                    END AS full_path
                FROM files f
                JOIN dirs d ON d.id = f.dir_id
            ",
            )
            .unwrap();
        let mut rows = stmt.query([]).unwrap();
        let mut count = 0usize;
        while let Some(row) = rows.next().unwrap() {
            let name_lower = match row.get_ref(0).ok().and_then(|v| v.as_str().ok()) {
                Some(v) => v,
                None => continue,
            };
            let full_path = match row.get_ref(1).ok().and_then(|v| v.as_str().ok()) {
                Some(v) => v,
                None => continue,
            };
            let path_buf = PathBuf::from(full_path);
            if let Some(mut bucket) = index.get_mut(name_lower) {
                bucket.push(path_buf);
            } else {
                index.insert(name_lower.to_owned(), vec![path_buf]);
            }
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
        let mut stmt = conn
            .prepare(
                "
                SELECT
                    f.name_lower,
                    CASE
                        WHEN d.path = '' THEN f.name
                        WHEN d.path = '/' THEN '/' || f.name
                        ELSE d.path || '/' || f.name
                    END AS full_path
                FROM files f
                JOIN dirs d ON d.id = f.dir_id
            ",
            )
            .unwrap();
        let rows = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .unwrap();
        let mut out = Vec::new();
        for row in rows.flatten() {
            let (name_lower, full_path) = row;
            out.push((name_lower, full_path));
        }
        out
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
        conn.query_row(
            "SELECT value FROM meta WHERE key = 'include_dirs'",
            [],
            |row| row.get::<_, String>(0),
        )
        .ok()
        .map(|v| v == "1")
    }

    pub fn save_exclude_exact_dirs(&self, dirs: &[String]) {
        let conn = self.conn.lock();
        let value = serde_json::to_string(dirs).unwrap_or_else(|_| "[]".to_string());
        let _ = conn.execute(
            "INSERT OR REPLACE INTO meta (key, value) VALUES ('exclude_exact_dirs', ?1)",
            params![value],
        );
    }

    pub fn load_exclude_exact_dirs(&self) -> Vec<String> {
        let conn = self.conn.lock();
        let raw = conn
            .query_row(
                "SELECT value FROM meta WHERE key = 'exclude_exact_dirs'",
                [],
                |row| row.get::<_, String>(0),
            )
            .unwrap_or_else(|_| "[]".to_string());
        serde_json::from_str::<Vec<String>>(&raw).unwrap_or_default()
    }

    pub fn save_exclude_pattern_dirs(&self, dirs: &[String]) {
        let conn = self.conn.lock();
        let value = serde_json::to_string(dirs).unwrap_or_else(|_| "[]".to_string());
        let _ = conn.execute(
            "INSERT OR REPLACE INTO meta (key, value) VALUES ('exclude_pattern_dirs', ?1)",
            params![value],
        );
    }

    pub fn load_exclude_pattern_dirs(&self) -> Vec<String> {
        let conn = self.conn.lock();
        let raw = conn
            .query_row(
                "SELECT value FROM meta WHERE key = 'exclude_pattern_dirs'",
                [],
                |row| row.get::<_, String>(0),
            )
            .unwrap_or_else(|_| "[]".to_string());
        serde_json::from_str::<Vec<String>>(&raw).unwrap_or_default()
    }

    pub fn checkpoint_truncate(&self) {
        let conn = self.conn.lock();
        let _ = conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);");
    }

    pub fn maybe_vacuum_after_rebuild(
        &self,
        min_file_bytes: u64,
        min_free_pages: u64,
        min_free_ratio: f64,
    ) -> bool {
        let file_size = fs::metadata(&self.path).map(|meta| meta.len()).unwrap_or(0);
        if file_size < min_file_bytes {
            return false;
        }

        let (page_count, freelist_count, page_size) = {
            let conn = self.conn.lock();
            let page_count = conn
                .query_row("PRAGMA page_count", [], |row| row.get::<_, i64>(0))
                .unwrap_or(0)
                .max(0) as u64;
            let freelist_count = conn
                .query_row("PRAGMA freelist_count", [], |row| row.get::<_, i64>(0))
                .unwrap_or(0)
                .max(0) as u64;
            let page_size = conn
                .query_row("PRAGMA page_size", [], |row| row.get::<_, i64>(0))
                .unwrap_or(0)
                .max(0) as u64;
            (page_count, freelist_count, page_size)
        };

        if page_count == 0 || freelist_count < min_free_pages || page_size == 0 {
            return false;
        }

        let free_ratio = freelist_count as f64 / page_count as f64;
        if free_ratio < min_free_ratio {
            return false;
        }

        self.vacuum();
        true
    }

    pub fn vacuum(&self) {
        let conn = self.conn.lock();
        let _ = conn.execute_batch("VACUUM;");
    }
}
