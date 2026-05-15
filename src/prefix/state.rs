use rusqlite::{Connection, params};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use crate::prefix::config::{PrefixConfig, RegisteredExecutable};
use crate::prefix::error::{Result, PrefixError};

/// SQLite-backed persistent store for Wine prefix state.
/// Caches prefix metadata, registered executables, and registry settings
/// so they don't need to be re-scanned on every launch.
pub struct PrefixStore {
    db: Mutex<Connection>,
}

impl PrefixStore {
    pub fn open_in_memory() -> Result<Self> {
        let db = Connection::open_in_memory()
            .map_err(|e| PrefixError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?;
        Self::init_db(&db)?;
        Ok(Self { db: Mutex::new(db) })
    }

    pub fn open(db_path: &Path) -> Result<Self> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| PrefixError::Io(e))?;
        }

        let db = Connection::open(db_path)
            .map_err(|e| PrefixError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?;
        Self::init_db(&db)?;
        Ok(Self { db: Mutex::new(db) })
    }

    fn init_db(db: &Connection) -> Result<()> {
        db.execute_batch(
            "CREATE TABLE IF NOT EXISTS prefixes (
                path TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                architecture TEXT NOT NULL DEFAULT 'win64',
                wine_version TEXT,
                description TEXT,
                config_version TEXT NOT NULL DEFAULT '1.0.0',
                created_at TEXT NOT NULL,
                modified_at TEXT NOT NULL,
                last_synced_at TEXT NOT NULL DEFAULT ''
            );

            CREATE TABLE IF NOT EXISTS executables (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                prefix_path TEXT NOT NULL REFERENCES prefixes(path) ON DELETE CASCADE,
                name TEXT NOT NULL,
                executable_path TEXT NOT NULL,
                description TEXT,
                icon_path TEXT,
                file_version TEXT,
                product_version TEXT,
                company_name TEXT,
                file_description TEXT,
                product_name TEXT
            );

            CREATE TABLE IF NOT EXISTS imported_modules (
                executable_id INTEGER NOT NULL REFERENCES executables(id) ON DELETE CASCADE,
                module_name TEXT NOT NULL,
                PRIMARY KEY (executable_id, module_name)
            );

            CREATE TABLE IF NOT EXISTS registry_settings (
                prefix_path TEXT NOT NULL REFERENCES prefixes(path) ON DELETE CASCADE,
                section TEXT NOT NULL,
                key TEXT,
                value TEXT,
                PRIMARY KEY (prefix_path, section, key)
            );

            CREATE TABLE IF NOT EXISTS scanned_executables (
                prefix_path TEXT NOT NULL REFERENCES prefixes(path) ON DELETE CASCADE,
                executable_path TEXT NOT NULL,
                name TEXT NOT NULL,
                description TEXT,
                icon_path TEXT,
                file_version TEXT,
                product_version TEXT,
                company_name TEXT,
                file_description TEXT,
                product_name TEXT,
                PRIMARY KEY (prefix_path, executable_path)
            );

            PRAGMA foreign_keys = ON;
            PRAGMA journal_mode = WAL;"
        ).map_err(|e| PrefixError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))
    }

    // ── Prefixes ──

    pub fn list_prefixes(&self) -> Result<Vec<(String, PrefixConfig)>> {
        let (rows_data, exe_map) = {
            let db = self.db.lock().unwrap();

            // Load all prefixes
            let mut stmt = db.prepare(
                "SELECT path, name, architecture, wine_version, description, config_version, created_at, modified_at
                 FROM prefixes ORDER BY name"
            ).map_err(map_err)?;

            let rows: Vec<(String, PrefixConfig)> = stmt.query_map([], |row| {
                let path: String = row.get("path")?;
                Ok((path, PrefixConfig {
                    version: row.get::<_, String>("config_version")?,
                    name: row.get("name")?,
                    creation_date: parse_dt(&row.get::<_, String>("created_at")?),
                    last_modified: parse_dt(&row.get::<_, String>("modified_at")?),
                    wine_version: row.get("wine_version")?,
                    architecture: row.get("architecture")?,
                    description: row.get("description")?,
                    registered_executables: Vec::new(),
                }))
            }).map_err(map_err)?.collect::<std::result::Result<Vec<_>, _>>().map_err(map_err)?;

            // Load all executables in one query
            let mut exe_stmt = db.prepare(
                "SELECT id, prefix_path, name, executable_path, description, icon_path, file_version, product_version, company_name, file_description, product_name
                 FROM executables ORDER BY prefix_path"
            ).map_err(map_err)?;

            let exes: Vec<(i64, String, RegisteredExecutable)> = exe_stmt.query_map([], |row| {
                let id: i64 = row.get("id")?;
                let prefix_path: String = row.get("prefix_path")?;
                Ok((id, prefix_path, RegisteredExecutable {
                    name: row.get("name")?,
                    executable_path: PathBuf::from(row.get::<_, String>("executable_path")?),
                    description: row.get("description")?,
                    icon_path: row.get::<_, Option<String>>("icon_path")?.map(PathBuf::from),
                    file_version: row.get("file_version")?,
                    product_version: row.get("product_version")?,
                    company_name: row.get("company_name")?,
                    file_description: row.get("file_description")?,
                    product_name: row.get("product_name")?,
                    imported_modules: Vec::new(),
                }))
            }).map_err(map_err)?.collect::<std::result::Result<Vec<_>, _>>().map_err(map_err)?;

            // Load imported modules
            let mut mod_stmt = db.prepare(
                "SELECT executable_id, module_name FROM imported_modules ORDER BY executable_id"
            ).map_err(map_err)?;
            let mut module_map: HashMap<i64, Vec<String>> = HashMap::new();
            let mods: Vec<(i64, String)> = mod_stmt.query_map([], |row| {
                Ok((row.get::<_, i64>("executable_id")?, row.get::<_, String>("module_name")?))
            }).map_err(map_err)?.filter_map(|r| r.ok()).collect();
            for (exe_id, module) in mods {
                module_map.entry(exe_id).or_default().push(module);
            }

            // Attach modules to executables
            let mut exe_by_prefix: HashMap<String, Vec<RegisteredExecutable>> = HashMap::new();
            for (id, prefix_path, mut exe) in exes {
                exe.imported_modules = module_map.remove(&id).unwrap_or_default();
                exe_by_prefix.entry(prefix_path).or_default().push(exe);
            }

            (rows, exe_by_prefix)
        }; // lock released

        // Assemble final result
        let configs = rows_data.into_iter().map(|(path, mut config)| {
            config.registered_executables = exe_map.get(&path).cloned().unwrap_or_default();
            (path, config)
        }).collect();

        Ok(configs)
    }

    pub fn get_prefix(&self, path: &str) -> Result<Option<PrefixConfig>> {
        let db = self.db.lock().unwrap();
        let mut stmt = db.prepare(
            "SELECT name, architecture, wine_version, description, config_version, created_at, modified_at
             FROM prefixes WHERE path = ?1"
        ).map_err(map_err)?;

        let result = stmt.query_row(params![path], |row| {
            Ok(PrefixConfig {
                version: row.get::<_, String>("config_version")?,
                name: row.get("name")?,
                creation_date: parse_dt(&row.get::<_, String>("created_at")?),
                last_modified: parse_dt(&row.get::<_, String>("modified_at")?),
                wine_version: row.get("wine_version")?,
                architecture: row.get("architecture")?,
                description: row.get("description")?,
                registered_executables: Vec::new(),
            })
        });

        match result {
            Ok(config) => Ok(Some(config)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_err(e)),
        }
    }

    pub fn save_prefix(&self, path: &str, config: &PrefixConfig) -> Result<()> {
        {
            let db = self.db.lock().unwrap();
            db.execute(
                "INSERT OR REPLACE INTO prefixes (path, name, architecture, wine_version, description, config_version, created_at, modified_at, last_synced_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    path,
                    config.name,
                    config.architecture,
                    config.wine_version,
                    config.description,
                    config.version,
                    fmt_dt(config.creation_date),
                    fmt_dt(config.last_modified),
                    fmt_dt(chrono::Utc::now()),
                ],
            ).map_err(map_err)?;
        } // lock dropped before calling save_executables

        self.save_executables(path, &config.registered_executables)?;
        Ok(())
    }

    // ── Executables ──

    fn save_executables(&self, prefix_path: &str, exes: &[RegisteredExecutable]) -> Result<()> {
        let db = self.db.lock().unwrap();
        db.execute("DELETE FROM executables WHERE prefix_path = ?1", params![prefix_path])
            .map_err(map_err)?;

        let mut stmt = db.prepare(
            "INSERT INTO executables (prefix_path, name, executable_path, description, icon_path, file_version, product_version, company_name, file_description, product_name)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)"
        ).map_err(map_err)?;

        let mut module_stmt = db.prepare(
            "INSERT OR IGNORE INTO imported_modules (executable_id, module_name) VALUES (?1, ?2)"
        ).map_err(map_err)?;

        for exe in exes {
            stmt.execute(params![
                prefix_path,
                exe.name,
                exe.executable_path.to_string_lossy().to_string(),
                exe.description,
                exe.icon_path.as_ref().map(|p| p.to_string_lossy().to_string()),
                exe.file_version,
                exe.product_version,
                exe.company_name,
                exe.file_description,
                exe.product_name,
            ]).map_err(map_err)?;

            let exe_id = db.last_insert_rowid();
            for module in &exe.imported_modules {
                module_stmt.execute(params![exe_id, module]).map_err(map_err)?;
            }
        }

        Ok(())
    }

    // ── Registry Settings ──

    pub fn get_setting(&self, prefix_path: &str, section: &str, key: &str) -> Result<Option<String>> {
        let db = self.db.lock().unwrap();
        let mut stmt = db.prepare(
            "SELECT value FROM registry_settings WHERE prefix_path = ?1 AND section = ?2 AND key = ?3"
        ).map_err(map_err)?;

        match stmt.query_row(params![prefix_path, section, key], |row| row.get::<_, Option<String>>("value")) {
            Ok(val) => Ok(val),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_err(e)),
        }
    }

    pub fn get_settings_section(&self, prefix_path: &str, section: &str) -> Result<Vec<(String, Option<String>)>> {
        let db = self.db.lock().unwrap();
        let mut stmt = db.prepare(
            "SELECT key, value FROM registry_settings WHERE prefix_path = ?1 AND section = ?2 ORDER BY key"
        ).map_err(map_err)?;

        let rows = stmt.query_map(params![prefix_path, section], |row| {
            Ok((row.get::<_, String>("key")?, row.get::<_, Option<String>>("value")?))
        }).map_err(map_err)?;

        rows.collect::<std::result::Result<Vec<_>, _>>().map_err(map_err)
    }

    pub fn save_setting(&self, prefix_path: &str, section: &str, key: &str, value: Option<&str>) -> Result<()> {
        let db = self.db.lock().unwrap();
        db.execute(
            "INSERT OR REPLACE INTO registry_settings (prefix_path, section, key, value) VALUES (?1, ?2, ?3, ?4)",
            params![prefix_path, section, key, value],
        ).map_err(map_err)?;
        Ok(())
    }

    // ── Sync / Delete ──

    pub fn delete_prefix(&self, path: &str) -> Result<()> {
        let db = self.db.lock().unwrap();
        db.execute("DELETE FROM prefixes WHERE path = ?1", params![path]).map_err(map_err)?;
        Ok(())
    }

    // ── Scanned Executables (refresh-time scan cache) ──

    pub fn save_scanned_executables(&self, prefix_path: &str, exes: &[RegisteredExecutable]) -> Result<()> {
        let db = self.db.lock().unwrap();
        db.execute("DELETE FROM scanned_executables WHERE prefix_path = ?1", params![prefix_path])
            .map_err(map_err)?;

        if exes.is_empty() {
            return Ok(());
        }

        let mut stmt = db.prepare(
            "INSERT OR IGNORE INTO scanned_executables (prefix_path, executable_path, name, description, icon_path, file_version, product_version, company_name, file_description, product_name)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)"
        ).map_err(map_err)?;

        for exe in exes {
            stmt.execute(params![
                prefix_path,
                exe.executable_path.to_string_lossy().to_string(),
                exe.name,
                exe.description,
                exe.icon_path.as_ref().map(|p| p.to_string_lossy().to_string()),
                exe.file_version,
                exe.product_version,
                exe.company_name,
                exe.file_description,
                exe.product_name,
            ]).map_err(map_err)?;
        }

        Ok(())
    }

    /// Load scanned (available) executables for a prefix, excluding already-registered ones.
    pub fn list_scanned_executables(&self, prefix_path: &str) -> Result<Vec<RegisteredExecutable>> {
        let db = self.db.lock().unwrap();
        let mut stmt = db.prepare(
            "SELECT s.name, s.executable_path, s.description, s.icon_path, s.file_version, s.product_version, s.company_name, s.file_description, s.product_name
             FROM scanned_executables s
             WHERE s.prefix_path = ?1
               AND s.executable_path NOT IN (SELECT executable_path FROM executables WHERE prefix_path = ?1)
             ORDER BY s.name"
        ).map_err(map_err)?;

        let exes: Vec<RegisteredExecutable> = stmt.query_map(params![prefix_path], |row| {
            Ok(RegisteredExecutable {
                name: row.get("name")?,
                executable_path: PathBuf::from(row.get::<_, String>("executable_path")?),
                description: row.get("description")?,
                icon_path: row.get::<_, Option<String>>("icon_path")?.map(PathBuf::from),
                file_version: row.get("file_version")?,
                product_version: row.get("product_version")?,
                company_name: row.get("company_name")?,
                file_description: row.get("file_description")?,
                product_name: row.get("product_name")?,
                imported_modules: Vec::new(),
            })
        }).map_err(map_err)?.collect::<std::result::Result<Vec<_>, _>>().map_err(map_err)?;

        Ok(exes)
    }
}

fn map_err(e: rusqlite::Error) -> PrefixError {
    PrefixError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))
}

fn parse_dt(s: &str) -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&chrono::Utc))
        .unwrap_or_else(|_| chrono::Utc::now())
}

fn fmt_dt(dt: chrono::DateTime<chrono::Utc>) -> String {
    dt.to_rfc3339()
}
