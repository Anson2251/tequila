use rusqlite::{Connection, params};
use std::path::PathBuf;
use std::sync::Mutex;

pub struct IconCache {
    db: Mutex<Connection>,
    cache_dir: PathBuf,
}

impl IconCache {
    pub fn open(cache_dir: PathBuf) -> Result<Self, String> {
        std::fs::create_dir_all(&cache_dir)
            .map_err(|e| format!("Failed to create icon cache dir: {}", e))?;
        let db_path = cache_dir.join("icons.db");
        let db = Connection::open(&db_path)
            .map_err(|e| format!("Failed to open icon cache DB: {}", e))?;

        let needs_migration = db
            .prepare("SELECT icon_blob FROM icons LIMIT 1")
            .and_then(|mut stmt| stmt.query_row([], |row| row.get::<_, Vec<u8>>(0)))
            .map(|data| data.get(0..4) == Some(b"\x00\x00\x01\x00"))
            .unwrap_or(false);

        if needs_migration {
            db.execute_batch("DROP TABLE IF EXISTS icons;").ok();
            for entry in std::fs::read_dir(&cache_dir)
                .into_iter()
                .flatten()
                .flatten()
            {
                let path = entry.path();
                let ext = path.extension().and_then(|e| e.to_str());
                if ext == Some("ico") || ext == Some("png") {
                    std::fs::remove_file(path).ok();
                }
            }
        }

        db.execute_batch(
            "CREATE TABLE IF NOT EXISTS icons (
                sha256 TEXT PRIMARY KEY,
                icon_blob BLOB NOT NULL,
                created_at INTEGER NOT NULL DEFAULT (unixepoch())
            );",
        )
        .map_err(|e| format!("Failed to create icons table: {}", e))?;

        Ok(Self {
            db: Mutex::new(db),
            cache_dir,
        })
    }

    pub fn get(&self, sha256: &str) -> Option<Vec<u8>> {
        let db = self.db.lock().ok()?;
        let mut stmt = db
            .prepare("SELECT icon_blob FROM icons WHERE sha256 = ?1")
            .ok()?;
        stmt.query_row(params![sha256], |row| row.get::<_, Vec<u8>>(0))
            .ok()
    }

    pub fn put(&self, sha256: &str, png_data: &[u8]) -> Result<(), String> {
        let db = self.db.lock().map_err(|e| format!("Lock error: {}", e))?;
        db.execute(
            "INSERT OR REPLACE INTO icons (sha256, icon_blob) VALUES (?1, ?2)",
            params![sha256, png_data],
        )
        .map_err(|e| format!("Failed to store icon: {}", e))?;
        Ok(())
    }

    pub fn icon_path(&self, sha256: &str) -> Option<PathBuf> {
        let png_path = self.cache_dir.join(format!("{}.png", sha256));
        if png_path.exists() {
            if png_path.metadata().ok().map(|m| m.len()).unwrap_or(0) > 0 {
                return Some(png_path);
            }
            let _ = std::fs::remove_file(&png_path);
            return None;
        }
        let data = self.get(sha256)?;
        if data.is_empty() {
            return None;
        }
        std::fs::write(&png_path, &data).ok()?;
        Some(png_path)
    }

    pub fn has_icon(&self, sha256: &str) -> Option<bool> {
        let data = self.get(sha256)?;
        Some(!data.is_empty())
    }
}
