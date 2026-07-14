use std::path::Path;

use rusqlite::{params, Connection, Result as SqlResult};

pub struct TestCache {
    conn: Connection,
}

impl TestCache {
    pub fn new(path: &Path) -> SqlResult<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS test_results (
                book_source_url TEXT NOT NULL,
                book_source_name TEXT NOT NULL,
                status TEXT NOT NULL,
                reason TEXT,
                tested_at INTEGER NOT NULL,
                retry_count INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (book_source_url, book_source_name)
            );
            CREATE INDEX IF NOT EXISTS idx_results_status ON test_results(status);
            CREATE TABLE IF NOT EXISTS pipeline_data (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );"
        )?;
        Ok(Self { conn })
    }

    pub fn check(&self, url: &str, name: &str) -> SqlResult<Option<(String, Option<String>)>> {
        let mut stmt = self.conn.prepare(
            "SELECT status, reason FROM test_results WHERE book_source_url = ?1 AND book_source_name = ?2"
        )?;
        let mut rows = stmt.query(params![url, name])?;
        if let Some(row) = rows.next()? {
            let status: String = row.get(0)?;
            let reason: Option<String> = row.get(1)?;
            Ok(Some((status, reason)))
        } else {
            Ok(None)
        }
    }

    /// Returns per-status counts from the test cache.
    pub fn summary(&self) -> SqlResult<Vec<(String, usize)>> {
        let mut stmt = self.conn.prepare(
            "SELECT status, COUNT(*) FROM test_results GROUP BY status"
        )?;
        let rows = stmt.query_map([], |row| {
            let status: String = row.get(0)?;
            let count: usize = row.get(1)?;
            Ok((status, count))
        })?;
        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }
        Ok(result)
    }

    /// Save a key-value metadata entry (e.g. "eligible", "report").
    pub fn save_meta(&self, key: &str, value: &str) -> SqlResult<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO pipeline_data (key, value) VALUES (?1, ?2)",
            params![key, value],
        )?;
        Ok(())
    }

    /// Load a metadata entry by key.
    pub fn load_meta(&self, key: &str) -> SqlResult<Option<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT value FROM pipeline_data WHERE key = ?1"
        )?;
        let mut rows = stmt.query(params![key])?;
        if let Some(row) = rows.next()? {
            Ok(Some(row.get(0)?))
        } else {
            Ok(None)
        }
    }

    pub fn save(&self, url: &str, name: &str, status: &str, reason: Option<&str>, retry_count: u32) -> SqlResult<()> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        self.conn.execute(
            "INSERT OR REPLACE INTO test_results (book_source_url, book_source_name, status, reason, tested_at, retry_count)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![url, name, status, reason, now, retry_count],
        )?;
        Ok(())
    }
}
