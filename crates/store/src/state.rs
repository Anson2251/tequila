use base::config::RegisteredExecutable;
use base::error::{PrefixError, Result};
use rusqlite::{Connection, params};
use std::path::Path;
use std::sync::Mutex;

pub struct PrefixStore {
    db: Mutex<Connection>,
}

impl PrefixStore {
    pub fn open(db_path: &Path) -> Result<Self> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| PrefixError::Io(e))?;
        }
        let db = Connection::open(db_path).map_err(|e| {
            PrefixError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                e.to_string(),
            ))
        })?;
        Self::init_db(&db)?;
        Ok(Self { db: Mutex::new(db) })
    }

    fn init_db(db: &Connection) -> Result<()> {
        db.execute_batch(
            "CREATE TABLE IF NOT EXISTS registry_settings (
                prefix_path TEXT NOT NULL, section TEXT NOT NULL, key TEXT, value TEXT,
                PRIMARY KEY (prefix_path, section, key)
            );
            CREATE TABLE IF NOT EXISTS registry_hashes (
                prefix_path TEXT NOT NULL PRIMARY KEY,
                user_reg_hash TEXT NOT NULL,
                system_reg_hash TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS scanned_executables (
                prefix_path TEXT NOT NULL, executable_path TEXT NOT NULL, name TEXT NOT NULL,
                description TEXT, icon_path TEXT, file_version TEXT, product_version TEXT,
                company_name TEXT, file_description TEXT, product_name TEXT,
                PRIMARY KEY (prefix_path, executable_path)
            );
            PRAGMA foreign_keys = ON;
            PRAGMA journal_mode = WAL;",
        )
        .map_err(|e| {
            PrefixError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                e.to_string(),
            ))
        })
    }

    pub fn get_setting(
        &self,
        prefix_path: &str,
        section: &str,
        key: &str,
    ) -> Result<Option<String>> {
        let db = self.db.lock().unwrap();
        let mut stmt = db.prepare(
            "SELECT value FROM registry_settings WHERE prefix_path = ?1 AND section = ?2 AND key = ?3"
        ).map_err(map_err)?;
        match stmt.query_row(params![prefix_path, section, key], |row| {
            row.get::<_, Option<String>>("value")
        }) {
            Ok(val) => Ok(val),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(map_err(e)),
        }
    }

    pub fn get_settings_section(
        &self,
        prefix_path: &str,
        section: &str,
    ) -> Result<Vec<(String, Option<String>)>> {
        let db = self.db.lock().unwrap();
        let mut stmt = db.prepare(
            "SELECT key, value FROM registry_settings WHERE prefix_path = ?1 AND section = ?2 ORDER BY key"
        ).map_err(map_err)?;
        let rows = stmt
            .query_map(params![prefix_path, section], |row| {
                Ok((
                    row.get::<_, String>("key")?,
                    row.get::<_, Option<String>>("value")?,
                ))
            })
            .map_err(map_err)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(map_err)
    }

    pub fn has_registry_cache(&self, prefix_path: &str) -> bool {
        let db = self.db.lock().unwrap();
        let mut stmt =
            match db.prepare("SELECT COUNT(*) FROM registry_settings WHERE prefix_path = ?1") {
                Ok(s) => s,
                Err(_) => return false,
            };
        stmt.query_row(params![prefix_path], |row| row.get::<_, i64>(0))
            .map(|count| count > 0)
            .unwrap_or(false)
    }

    pub fn has_scanned_prefix(&self, prefix_path: &str) -> bool {
        let db = self.db.lock().unwrap();
        let mut stmt =
            match db.prepare("SELECT 1 FROM scanned_executables WHERE prefix_path = ?1 LIMIT 1") {
                Ok(s) => s,
                Err(_) => return false,
            };
        stmt.query_row(params![prefix_path], |_| Ok(())).is_ok()
    }

    pub fn save_setting(
        &self,
        prefix_path: &str,
        section: &str,
        key: &str,
        value: Option<&str>,
    ) -> Result<()> {
        let db = self.db.lock().unwrap();
        db.execute(
            "INSERT OR REPLACE INTO registry_settings (prefix_path, section, key, value) VALUES (?1, ?2, ?3, ?4)",
            params![prefix_path, section, key, value],
        ).map_err(map_err)?;
        Ok(())
    }

    pub fn invalidate_registry_cache(&self, prefix_path: &str) -> Result<()> {
        let db = self.db.lock().unwrap();
        db.execute(
            "DELETE FROM registry_settings WHERE prefix_path = ?1",
            params![prefix_path],
        )
        .map_err(map_err)?;
        db.execute(
            "DELETE FROM registry_hashes WHERE prefix_path = ?1",
            params![prefix_path],
        )
        .map_err(map_err)?;
        Ok(())
    }

    /// Check whether the stored registry hashes match the given ones.
    pub fn verify_registry_hashes(
        &self,
        prefix_path: &str,
        user_hash: &str,
        system_hash: &str,
    ) -> Result<bool> {
        let db = self.db.lock().unwrap();
        let mut stmt = db
            .prepare(
                "SELECT user_reg_hash, system_reg_hash FROM registry_hashes WHERE prefix_path = ?1",
            )
            .map_err(map_err)?;
        match stmt.query_row(params![prefix_path], |row| {
            let stored_user: String = row.get(0)?;
            let stored_system: String = row.get(1)?;
            Ok((stored_user, stored_system))
        }) {
            Ok((stored_user, stored_system)) => {
                Ok(stored_user == user_hash && stored_system == system_hash)
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(false),
            Err(e) => Err(map_err(e)),
        }
    }

    /// Save (or update) the hashes of the registry files for a prefix.
    pub fn save_registry_hashes(
        &self,
        prefix_path: &str,
        user_hash: &str,
        system_hash: &str,
    ) -> Result<()> {
        let db = self.db.lock().unwrap();
        db.execute(
            "INSERT OR REPLACE INTO registry_hashes (prefix_path, user_reg_hash, system_reg_hash)
             VALUES (?1, ?2, ?3)",
            params![prefix_path, user_hash, system_hash],
        )
        .map_err(map_err)?;
        Ok(())
    }

    pub fn save_scanned_executables(
        &self,
        prefix_path: &str,
        exes: &[RegisteredExecutable],
    ) -> Result<()> {
        let db = self.db.lock().unwrap();
        db.execute(
            "DELETE FROM scanned_executables WHERE prefix_path = ?1",
            params![prefix_path],
        )
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
                exe.icon_path
                    .as_ref()
                    .map(|p| p.to_string_lossy().to_string()),
                exe.file_version,
                exe.product_version,
                exe.company_name,
                exe.file_description,
                exe.product_name,
            ])
            .map_err(map_err)?;
        }
        Ok(())
    }

    pub fn list_scanned_executables(&self, prefix_path: &str) -> Result<Vec<RegisteredExecutable>> {
        let db = self.db.lock().unwrap();
        let mut stmt = db.prepare(
            "SELECT name, executable_path, description, icon_path, file_version, product_version, company_name, file_description, product_name
             FROM scanned_executables WHERE prefix_path = ?1 ORDER BY name"
        ).map_err(map_err)?;
        let exes = stmt
            .query_map(params![prefix_path], |row| {
                Ok(RegisteredExecutable {
                    name: row.get("name")?,
                    executable_path: std::path::PathBuf::from(
                        row.get::<_, String>("executable_path")?,
                    ),
                    description: row.get("description")?,
                    icon_path: row
                        .get::<_, Option<String>>("icon_path")?
                        .map(std::path::PathBuf::from),
                    file_version: row.get("file_version")?,
                    product_version: row.get("product_version")?,
                    company_name: row.get("company_name")?,
                    file_description: row.get("file_description")?,
                    product_name: row.get("product_name")?,
                    imported_modules: Vec::new(),
                    env_vars: std::collections::HashMap::new(),
                    cwd: None,
                })
            })
            .map_err(map_err)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(map_err)?;
        Ok(exes)
    }
}

fn map_err(e: rusqlite::Error) -> PrefixError {
    PrefixError::Io(std::io::Error::new(
        std::io::ErrorKind::Other,
        e.to_string(),
    ))
}

impl std::fmt::Debug for PrefixStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PrefixStore").finish()
    }
}
