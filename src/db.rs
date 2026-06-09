use parking_lot::Mutex;
use rusqlite::{params, Connection};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::model::SortKey;

#[derive(Clone)]
pub struct Db {
    path: PathBuf,
    conn: Arc<Mutex<Connection>>,
}

impl Db {
    pub fn init_default() -> Self {
        let home_dir = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let data_dir = PathBuf::from(home_dir)
            .join("Library")
            .join("Caches")
            .join("MacHunt");
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
            PRAGMA cache_size=-8192;
            PRAGMA temp_store=MEMORY;
            PRAGMA mmap_size=33554432;
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
        Self::ensure_fts5(conn);
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
        let _ = conn.execute_batch(
            "DELETE FROM files_fts; DELETE FROM files; DELETE FROM dirs;",
        );
    }

    /// Replace the current connection with a fresh temp database
    /// (at index.db.new). All subsequent inserts go to the temp DB.
    pub fn begin_rebuild(&self) {
        let temp_path = self.path.with_extension("db.new");
        let _ = fs::remove_file(&temp_path);
        let _ = fs::remove_file(temp_path.with_extension("db-wal"));
        let _ = fs::remove_file(temp_path.with_extension("db-shm"));

        let mut temp_conn = Connection::open(&temp_path).unwrap();
        Self::apply_pragmas(&temp_conn);
        Self::ensure_schema(&mut temp_conn);

        let mut guard = self.conn.lock();
        // Swap: old connection dropped → old DB fd closed.
        let _old = std::mem::replace(&mut *guard, temp_conn);
    }

    /// Atomically promote the temp database to the main one.
    ///  1. checkpoint the temp DB
    ///  2. close temp connection
    ///  3. rename index.db.new → index.db
    ///  4. reopen the main connection
    pub fn finish_rebuild(&self) -> Result<(), String> {
        let temp_path = self.path.with_extension("db.new");

        // Checkpoint + close temp connection.
        {
            let mut guard = self.conn.lock();
            guard
                .execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
                .map_err(|e| e.to_string())?;
            // Replace with a dummy in-memory connection so the temp fd is closed.
            let _temp = std::mem::replace(&mut *guard, Connection::open_in_memory().unwrap());
            // _temp dropped → sqlite3_close on temp DB.
        }

        // Atomic swap on APFS (same volume).
        if temp_path.exists() {
            let _ = fs::remove_file(self.path.with_extension("db-wal"));
            let _ = fs::remove_file(self.path.with_extension("db-shm"));
            fs::rename(&temp_path, &self.path).map_err(|e| e.to_string())?;
        }

        // Reopen main connection.
        let new_conn = Connection::open(&self.path).map_err(|e| e.to_string())?;
        Self::apply_pragmas(&new_conn);

        let mut guard = self.conn.lock();
        *guard = new_conn;

        Ok(())
    }

    pub fn insert(&self, fallback_name: &str, path: &Path) -> Option<i64> {
        let conn = self.conn.lock();
        let dir_path = Self::parent_key(path);
        let stored_name = Self::derive_name(path, fallback_name);
        let stored_name_lower = if fallback_name.is_empty() {
            stored_name.to_lowercase()
        } else {
            fallback_name.to_string()
        };
        if stored_name.is_empty() {
            return None;
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
            // Return the rowid for FTS sync.
            return Some(conn.last_insert_rowid());
        }
        None
    }

    pub fn delete(&self, path: &Path) {
        let conn = self.conn.lock();
        let dir_path = Self::parent_key(path);
        let file_name = match path.file_name().and_then(|n| n.to_str()) {
            Some(v) => v,
            None => return,
        };

        // Get the file id for FTS cleanup.
        let file_id: Option<i64> = conn
            .query_row(
                "SELECT f.id FROM files f
                 JOIN dirs d ON d.id = f.dir_id
                 WHERE f.name = ?1 AND d.path = ?2",
                params![file_name, dir_path.as_str()],
                |row| row.get(0),
            )
            .ok();

        let _ = conn.execute(
            "DELETE FROM files
             WHERE name = ?1
               AND dir_id = (SELECT id FROM dirs WHERE path = ?2)",
            params![file_name, dir_path.as_str()],
        );

        // Remove from FTS index.
        if let Some(id) = file_id {
            let _ = conn.execute(
                "INSERT INTO files_fts(files_fts, rowid, name_lower) VALUES('delete', ?1, '')",
                params![id],
            );
        }

        let _ = conn.execute(
            "DELETE FROM dirs
             WHERE path = ?1
               AND NOT EXISTS (SELECT 1 FROM files WHERE files.dir_id = dirs.id LIMIT 1)",
            params![dir_path.as_str()],
        );
    }

    pub fn delete_under_root(&self, root: &Path) {
        let conn = self.conn.lock();
        if root == Path::new("/") {
            let _ = conn.execute_batch(
                "DELETE FROM files_fts; DELETE FROM files; DELETE FROM dirs;",
            );
            return;
        }

        let root_text = root.to_string_lossy().trim_end_matches('/').to_string();
        if root_text.is_empty() {
            return;
        }
        let prefix = format!("{}/%", root_text);

        // Delete FTS entries for affected files.
        let _ = conn.execute(
            "INSERT INTO files_fts(files_fts, rowid, name_lower)
             SELECT 'delete', f.id, ''
             FROM files f
             JOIN dirs d ON d.id = f.dir_id
             WHERE d.path = ?1 OR d.path LIKE ?2",
            params![root_text, prefix],
        );

        let _ = conn.execute(
            "DELETE FROM files WHERE dir_id IN (
                SELECT id FROM dirs WHERE path = ?1 OR path LIKE ?2
            )",
            params![root_text, prefix],
        );

        let _ = conn.execute(
            "DELETE FROM dirs WHERE path = ?1 OR path LIKE ?2",
            params![root_text, prefix],
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

    pub fn count_files(&self) -> usize {
        let conn = self.conn.lock();
        conn.query_row("SELECT COUNT(*) FROM files", [], |row| row.get::<_, i64>(0))
            .unwrap_or(0) as usize
    }

    pub fn has_any_files(&self) -> bool {
        let conn = self.conn.lock();
        conn.query_row("SELECT EXISTS(SELECT 1 FROM files LIMIT 1)", [], |row| {
            row.get::<_, i64>(0)
        })
        .unwrap_or(0)
            != 0
    }

    pub fn list_all_paths(&self) -> Vec<(i64, String)> {
        let conn = self.conn.lock();
        let mut stmt = conn
            .prepare(
                "SELECT f.id,
                    CASE
                        WHEN d.path = '' THEN f.name
                        WHEN d.path = '/' THEN '/' || f.name
                        ELSE d.path || '/' || f.name
                    END AS full_path
                 FROM files f
                 JOIN dirs d ON d.id = f.dir_id",
            )
            .unwrap();
        let rows = stmt
            .query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)))
            .unwrap();
        rows.filter_map(|r| r.ok()).collect()
    }

    pub fn list_paths_after_id(&self, last_id: u64, limit: usize) -> Vec<(i64, String)> {
        let conn = self.conn.lock();
        let mut stmt = conn
            .prepare(
                "SELECT f.id,
                    CASE
                        WHEN d.path = '' THEN f.name
                        WHEN d.path = '/' THEN '/' || f.name
                        ELSE d.path || '/' || f.name
                    END AS full_path
                 FROM files f
                 JOIN dirs d ON d.id = f.dir_id
                 WHERE f.id > ?1
                 ORDER BY f.id ASC
                 LIMIT ?2",
            )
            .unwrap();
        let rows = stmt
            .query_map(params![last_id as i64, limit as i64], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })
            .unwrap();
        rows.filter_map(|r| r.ok()).collect()
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

    pub fn save_watch_roots(&self, roots: &[String]) {
        let conn = self.conn.lock();
        let value = serde_json::to_string(roots).unwrap_or_else(|_| "[]".to_string());
        let _ = conn.execute(
            "INSERT OR REPLACE INTO meta (key, value) VALUES ('watch_roots', ?1)",
            params![value],
        );
    }

    pub fn load_watch_roots(&self) -> Vec<String> {
        let conn = self.conn.lock();
        let raw = conn
            .query_row("SELECT value FROM meta WHERE key = 'watch_roots'", [], |row| {
                row.get::<_, String>(0)
            })
            .unwrap_or_else(|_| "[]".to_string());
        serde_json::from_str::<Vec<String>>(&raw).unwrap_or_default()
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

    // ── FTS5 trigram search ──

    /// Create the FTS5 trigram virtual table (idempotent).
    fn ensure_fts5(conn: &Connection) {
        let _ = conn.execute_batch(
            "CREATE VIRTUAL TABLE IF NOT EXISTS files_fts USING fts5(
                name_lower,
                content='',
                tokenize='trigram'
            );",
        );
    }

    /// Insert a single entry into the FTS index. rowid must match files.id.
    pub fn insert_fts(&self, rowid: i64, name_lower: &str) {
        let conn = self.conn.lock();
        let _ = conn.execute(
            "INSERT INTO files_fts(rowid, name_lower) VALUES (?1, ?2)",
            params![rowid, name_lower],
        );
    }

    /// Delete a single entry from the FTS index by rowid.
    pub fn delete_fts(&self, rowid: i64) {
        let conn = self.conn.lock();
        let _ = conn.execute(
            "INSERT INTO files_fts(files_fts, rowid, name_lower) VALUES('delete', ?1, '')",
            params![rowid],
        );
    }

    /// Rebuild FTS index from the files table (used after full rebuild).
    pub fn rebuild_fts(&self) {
        let conn = self.conn.lock();
        let _ = conn.execute_batch("DELETE FROM files_fts;");
        let _ = conn.execute_batch(
            "INSERT INTO files_fts(rowid, name_lower) SELECT id, name_lower FROM files;",
        );
    }

    fn map_row(row: &rusqlite::Row) -> rusqlite::Result<(String, String)> {
        Ok((row.get(0)?, row.get(1)?))
    }

    /// Execute a name/path query and collect results into a Vec,
    /// with or without a path-prefix parameter.
    fn exec_name_query<P1>(
        conn: &rusqlite::Connection,
        sql: &str,
        name_param: P1,
        path_param: &Option<String>,
        limit: i64,
    ) -> Vec<(String, String)>
    where
        P1: rusqlite::types::ToSql,
    {
        let mut stmt = conn.prepare(sql).unwrap();
        if let Some(ref pp) = path_param {
            stmt.query_map(params![name_param, pp, limit], Self::map_row as fn(&rusqlite::Row) -> _)
        } else {
            stmt.query_map(params![name_param, limit], Self::map_row as fn(&rusqlite::Row) -> _)
        }
        .unwrap()
        .filter_map(|r| r.ok())
        .collect()
    }

    /// Returns (sql_clause, param_value) for path-prefix filtering, or empty if none.
    fn path_prefix_clause(prefix: Option<&str>) -> (String, Option<String>) {
        match prefix {
            Some(p) if !p.is_empty() => {
                let escaped = p.replace('%', "\\%").replace('_', "\\_");
                (
                    " AND d.path LIKE ? ESCAPE '\\'".to_string(),
                    Some(format!("{}%", escaped)),
                )
            }
            _ => (String::new(), None),
        }
    }

    /// Build an extension filter clause for SQL.
    /// Returns SQL like ` AND (f.name_lower LIKE '%.pdf' OR f.name_lower LIKE '%.png')`.
    fn extension_sql(extensions: Option<&[String]>) -> String {
        match extensions {
            Some(exts) if !exts.is_empty() => {
                let conditions: Vec<String> = exts
                    .iter()
                    .map(|ext| format!("f.name_lower LIKE '%.{}'", ext.replace('\'', "''")))
                    .collect();
                format!(" AND ({})", conditions.join(" OR "))
            }
            _ => String::new(),
        }
    }

    /// Build an ORDER BY clause from sort key and direction.
    fn sort_clause(key: SortKey, ascending: bool) -> String {
        let dir = if ascending { "ASC" } else { "DESC" };
        match key {
            SortKey::Name => format!("ORDER BY f.name_lower {}, d.path {}", dir, dir),
            SortKey::Path => format!("ORDER BY d.path {}, f.name_lower {}", dir, dir),
            SortKey::Type => format!(
                "ORDER BY CASE WHEN INSTR(f.name_lower, '.') > 0 THEN SUBSTR(f.name_lower, INSTR(f.name_lower, '.') + 1) ELSE '' END {}, f.name_lower {}",
                dir, dir
            ),
            // Size and Modified cannot be sorted in SQL — engine re-sorts post-fetch
            SortKey::Size | SortKey::Modified => String::from("ORDER BY f.name_lower ASC, d.path ASC"),
        }
    }

    /// Search via FTS5 trigram. Returns (dir_path, file_name) pairs.
    /// Falls back to LIKE/GLOB for short / non-alphanumeric queries.
    pub fn search_fts(
        &self,
        query: &str,
        case_sensitive: bool,
        path_prefix: Option<&str>,
        extensions: Option<&[String]>,
        sort_key: SortKey,
        sort_ascending: bool,
        limit: usize,
    ) -> Vec<(String, String)> {
        let conn = self.conn.lock();
        let q = query.trim();
        if q.is_empty() || limit == 0 {
            return Vec::new();
        }

        let (path_clause, path_param) = Self::path_prefix_clause(path_prefix);
        let ext_clause = Self::extension_sql(extensions);
        let sort = Self::sort_clause(sort_key, sort_ascending);
        let lim = limit as i64;

        // Fall back to LIKE/GLOB when FTS5 trigram is unreliable:
        // - Fewer than 3 characters (chars, not bytes — 2 CJK chars = 0 trigrams)
        // - Non-ASCII (CJK, etc. — trigram tokenizer may not handle well)
        // - ASCII with special characters (dots, hyphens — tokenizer splits on these)
        if q.chars().count() < 3 || !q.is_ascii() || !q.chars().all(|c| c.is_alphanumeric()) {
            // If query is just "*", match everything (SQL "%").
            let is_match_all = q == "*";
            if case_sensitive {
                let sql = format!(
                    "SELECT d.path, f.name FROM files f
                     JOIN dirs d ON d.id = f.dir_id
                     WHERE f.name GLOB ?{}{} {} LIMIT ?",
                    path_clause, ext_clause, sort
                );
                let pattern = if is_match_all { "*".to_string() } else { format!("*{}*", q) };
                return Self::exec_name_query(&conn, &sql, pattern, &path_param, lim);
            } else {
                let sql = format!(
                    "SELECT d.path, f.name FROM files f
                     JOIN dirs d ON d.id = f.dir_id
                     WHERE f.name_lower LIKE ?{}{} {} LIMIT ?",
                    path_clause, ext_clause, sort
                );
                let pattern = if is_match_all { "%".to_string() } else { format!("%{}%", q.to_lowercase()) };
                return Self::exec_name_query(&conn, &sql, pattern, &path_param, lim);
            }
        }

        // FTS5 trigram search.
        if case_sensitive {
            let sql = format!(
                "SELECT d.path, f.name FROM files_fts
                 JOIN files f ON f.id = files_fts.rowid
                 JOIN dirs d ON d.id = f.dir_id
                 WHERE files_fts MATCH ? AND f.name GLOB ?{}{} {} LIMIT ?",
                path_clause, ext_clause, sort
            );
            let lowered = q.to_lowercase();
            let pattern = format!("*{}*", q);
            let mut stmt = match conn.prepare(&sql) {
                Ok(s) => s,
                Err(_) => return Vec::new(),
            };
            if let Some(ref pp) = path_param {
                stmt.query_map(params![lowered, pattern, pp, lim], Self::map_row as fn(&rusqlite::Row) -> _)
            } else {
                stmt.query_map(params![lowered, pattern, lim], Self::map_row as fn(&rusqlite::Row) -> _)
            }
            .unwrap()
            .filter_map(|r| r.ok())
            .collect()
        } else {
            let sql = format!(
                "SELECT d.path, f.name FROM files_fts
                 JOIN files f ON f.id = files_fts.rowid
                 JOIN dirs d ON d.id = f.dir_id
                 WHERE files_fts MATCH ?{}{} {} LIMIT ?",
                path_clause, ext_clause, sort
            );
            let mut stmt = match conn.prepare(&sql) {
                Ok(s) => s,
                Err(_) => return Vec::new(),
            };
            if let Some(ref pp) = path_param {
                stmt.query_map(params![q, pp, lim], Self::map_row as fn(&rusqlite::Row) -> _)
            } else {
                stmt.query_map(params![q, lim], Self::map_row as fn(&rusqlite::Row) -> _)
            }
            .unwrap()
            .filter_map(|r| r.ok())
            .collect()
        }
    }

    /// LIKE/GLOB-based candidate search — returns (dir_path, file_name).
    /// Used as fallback for non-ASCII queries and regex/pattern filtering.
    pub fn search_like(
        &self,
        pattern: &str,
        case_sensitive: bool,
        path_prefix: Option<&str>,
        extensions: Option<&[String]>,
        sort_key: SortKey,
        sort_ascending: bool,
        limit: usize,
    ) -> Vec<(String, String)> {
        let conn = self.conn.lock();
        let (path_clause, path_param) = Self::path_prefix_clause(path_prefix);
        let ext_clause = Self::extension_sql(extensions);
        let sort = Self::sort_clause(sort_key, sort_ascending);
        let lim = limit as i64;
        if case_sensitive {
            let sql = format!(
                "SELECT d.path, f.name FROM files f
                 JOIN dirs d ON d.id = f.dir_id
                 WHERE f.name GLOB ?{}{} {} LIMIT ?",
                path_clause, ext_clause, sort
            );
            Self::exec_name_query(
                &conn, &sql,
                pattern.replace('%', "*").replace('_', "?"), &path_param, lim,
            )
        } else {
            let sql = format!(
                "SELECT d.path, f.name FROM files f
                 JOIN dirs d ON d.id = f.dir_id
                 WHERE f.name_lower LIKE ?{}{} {} LIMIT ?",
                path_clause, ext_clause, sort
            );
            Self::exec_name_query(
                &conn, &sql,
                pattern.to_string(), &path_param, lim,
            )
        }
    }

    /// List all files in a directory (by dir path). Used for rename cleanup.
    pub fn list_files_in_dir(&self, dir_path: &str) -> Vec<(String, String)> {
        let conn = self.conn.lock();
        let mut stmt = conn
            .prepare(
                "SELECT f.name,
                    CASE
                        WHEN d.path = '/' THEN '/' || f.name
                        ELSE d.path || '/' || f.name
                    END AS full_path
                 FROM files f
                 JOIN dirs d ON d.id = f.dir_id
                 WHERE d.path = ?1",
            )
            .unwrap();
        stmt.query_map(params![dir_path], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .unwrap()
        .filter_map(|r| r.ok())
        .collect()
    }

    /// Delete a file entry by directory path and file name.
    pub fn delete_by_dir_and_name(&self, dir_path: &str, name: &str) {
        let conn = self.conn.lock();
        // Get file id for FTS cleanup.
        let file_id: Option<i64> = conn
            .query_row(
                "SELECT f.id FROM files f
                 JOIN dirs d ON d.id = f.dir_id
                 WHERE d.path = ?1 AND f.name = ?2",
                params![dir_path, name],
                |row| row.get(0),
            )
            .ok();

        let _ = conn.execute(
            "DELETE FROM files
             WHERE name = ?1
               AND dir_id = (SELECT id FROM dirs WHERE path = ?2)",
            params![name, dir_path],
        );

        if let Some(id) = file_id {
            let _ = conn.execute(
                "INSERT INTO files_fts(files_fts, rowid, name_lower) VALUES('delete', ?1, '')",
                params![id],
            );
        }
    }

    /// Broad query for fuzzy search: prefix-anchored LIKE + length filter.
    /// Levenshtein in engine.rs does the actual fuzzy scoring.
    pub fn search_fuzzy_candidates(
        &self,
        query_lower: &str,
        path_prefix: Option<&str>,
        extensions: Option<&[String]>,
        limit: usize,
    ) -> Vec<(String, String)> {
        let conn = self.conn.lock();
        let q_len = query_lower.chars().count();
        if q_len == 0 || limit == 0 {
            return Vec::new();
        }
        let (path_clause, path_param) = Self::path_prefix_clause(path_prefix);
        let ext_clause = Self::extension_sql(extensions);
        let prefix: String = query_lower.chars().take(2).collect();
        let len_min = q_len.saturating_sub(3).max(1) as i64;
        let len_max = (q_len + 3) as i64;
        let lim = limit as i64;
        let sql = format!(
            "SELECT d.path, f.name FROM files f
             JOIN dirs d ON d.id = f.dir_id
             WHERE f.name_lower LIKE ?
               AND LENGTH(f.name_lower) BETWEEN ? AND ?{}{} LIMIT ?",
            path_clause, ext_clause
        );
        let mut stmt = conn.prepare(&sql).unwrap();
        if let Some(ref pp) = path_param {
            stmt.query_map(
                params![format!("{}%", prefix), len_min, len_max, pp, lim],
                Self::map_row as fn(&rusqlite::Row) -> _,
            )
        } else {
            stmt.query_map(
                params![format!("{}%", prefix), len_min, len_max, lim],
                Self::map_row as fn(&rusqlite::Row) -> _,
            )
        }
        .unwrap()
        .filter_map(|r| r.ok())
        .collect()
    }
}
