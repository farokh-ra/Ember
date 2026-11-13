//! Embedded, ordered, transactional schema migrations.

use rusqlite::{Connection, TransactionBehavior, params};
use sha2::{Digest, Sha256};

use crate::error::{StoreError, StoreResult};
use crate::util::{hex_lower, now_secs};

/// One embedded migration.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Migration {
    pub(crate) version: i64,
    pub(crate) name: &'static str,
    pub(crate) sql: &'static str,
}

/// Every migration shipped by this build, in ascending version order.
pub(crate) const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "initial metadata schema",
        sql: include_str!("../migrations/0001_initial.sql"),
    },
    Migration {
        version: 2,
        name: "FTS5 search index",
        sql: include_str!("../migrations/0002_search_index.sql"),
    },
];

/// Newest schema version understood by this build.
pub(crate) const LATEST_VERSION: i64 = 2;

/// Runs all outstanding migrations on the writer connection.
pub(crate) fn run(conn: &mut Connection) -> StoreResult<()> {
    run_with(conn, MIGRATIONS)
}

/// Runs a supplied migration list. Used by rollback and upgrade tests.
pub(crate) fn run_with(conn: &mut Connection, migrations: &[Migration]) -> StoreResult<()> {
    let user_version: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    let history_exists = table_exists(conn, "schema_migrations")?;
    let applied = if history_exists {
        load_history(conn)?
    } else {
        Vec::new()
    };

    if user_version < 0 {
        return Err(StoreError::UnsupportedSchema(
            "negative user_version is not valid".to_owned(),
        ));
    }

    if !history_exists && user_version > 0 {
        return Err(StoreError::UnsupportedSchema(format!(
            "user_version {user_version} has no migration history; refusing to continue"
        )));
    }

    let latest_supplied = migrations.last().map_or(0, |migration| migration.version);
    if user_version > latest_supplied {
        return Err(StoreError::UnsupportedSchema(format!(
            "database schema version {user_version} is newer than this build supports ({latest_supplied})"
        )));
    }

    for record in &applied {
        if record.version > user_version {
            return Err(StoreError::UnsupportedSchema(format!(
                "migration history contains version {} but user_version is {user_version}",
                record.version
            )));
        }
        if let Some(migration) = migrations.iter().find(|item| item.version == record.version) {
            let checksum = checksum(migration.sql);
            if checksum != record.checksum {
                return Err(StoreError::UnsupportedSchema(format!(
                    "migration {} checksum does not match the embedded migration",
                    record.version
                )));
            }
        } else {
            return Err(StoreError::UnsupportedSchema(format!(
                "migration history contains unknown version {}",
                record.version
            )));
        }
    }

    let mut current = user_version;
    for migration in migrations {
        if migration.version <= current {
            continue;
        }
        if migration.version != current + 1 {
            return Err(StoreError::UnsupportedSchema(format!(
                "migration versions must be contiguous; expected {} but found {}",
                current + 1,
                migration.version
            )));
        }
        apply(conn, migration)?;
        current = migration.version;
    }

    let final_version: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if final_version != current {
        return Err(StoreError::UnsupportedSchema(format!(
            "user_version {final_version} does not match migrated version {current}"
        )));
    }
    Ok(())
}

fn apply(conn: &mut Connection, migration: &Migration) -> StoreResult<()> {
    let transaction = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|error| migration_error(migration, &error))?;

    let result = (|| -> Result<(), rusqlite::Error> {
        transaction.execute_batch(migration.sql)?;
        transaction.execute(
            "INSERT INTO schema_migrations (version, name, applied_at, checksum) VALUES (?1, ?2, ?3, ?4)",
            params![
                migration.version,
                migration.name,
                i64::try_from(now_secs()).unwrap_or(i64::MAX),
                checksum(migration.sql)
            ],
        )?;
        transaction.pragma_update(None, "user_version", migration.version)?;
        transaction.commit()
    })();

    result.map_err(|error| migration_error(migration, &error))
}

fn migration_error(migration: &Migration, error: &rusqlite::Error) -> StoreError {
    StoreError::Migration {
        version: migration.version,
        name: migration.name,
        message: error.to_string(),
    }
}

fn checksum(sql: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(sql.as_bytes());
    hex_lower(hasher.finalize().as_slice())
}

fn table_exists(conn: &Connection, name: &str) -> Result<bool, rusqlite::Error> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type IN ('table', 'view') AND name = ?1",
        [name],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

struct HistoryRow {
    version: i64,
    checksum: String,
}

fn load_history(conn: &Connection) -> Result<Vec<HistoryRow>, rusqlite::Error> {
    let mut statement = conn.prepare("SELECT version, checksum FROM schema_migrations ORDER BY version ASC")?;
    let rows = statement.query_map([], |row| {
        Ok(HistoryRow {
            version: row.get(0)?,
            checksum: row.get(1)?,
        })
    })?;
    rows.collect()
}

#[cfg(test)]
mod tests {
    use super::{LATEST_VERSION, MIGRATIONS, Migration, run_with};
    use crate::error::StoreError;
    use rusqlite::Connection;

    fn user_version(conn: &Connection) -> i64 {
        conn.pragma_query_value(None, "user_version", |row| row.get(0))
            .expect("read user_version")
    }

    fn table_exists(conn: &Connection, name: &str) -> bool {
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM sqlite_master WHERE name = ?1", [name], |row| {
                row.get(0)
            })
            .expect("query sqlite_master");
        count > 0
    }

    #[test]
    fn creates_schema_and_records_history() {
        let mut conn = Connection::open_in_memory().expect("open memory database");
        run_with(&mut conn, MIGRATIONS).expect("run migrations");

        assert_eq!(user_version(&conn), LATEST_VERSION);
        for table in [
            "identity_refs",
            "profiles",
            "publications",
            "tombstones",
            "contents",
            "content_refs",
            "downloads",
            "replicas",
            "peers",
            "settings",
            "schema_migrations",
            "search_index",
        ] {
            assert!(table_exists(&conn, table), "missing table {table}");
        }

        let history: i64 = conn
            .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| row.get(0))
            .expect("count history");
        assert_eq!(history, i64::try_from(MIGRATIONS.len()).expect("migration count"));
    }

    #[test]
    fn upgrades_a_version_one_database_without_data_loss() {
        let mut conn = Connection::open_in_memory().expect("open memory database");
        run_with(&mut conn, &MIGRATIONS[..1]).expect("run version one");
        assert_eq!(user_version(&conn), 1);
        assert!(!table_exists(&conn, "search_index"));

        conn.execute(
            "INSERT INTO settings (key, value, updated_at) VALUES ('theme', 'dark', 42)",
            [],
        )
        .expect("insert setting");

        run_with(&mut conn, MIGRATIONS).expect("upgrade to latest");
        assert_eq!(user_version(&conn), 2);
        assert!(table_exists(&conn, "search_index"));

        let theme: String = conn
            .query_row("SELECT value FROM settings WHERE key = 'theme'", [], |row| row.get(0))
            .expect("read setting");
        assert_eq!(theme, "dark");
    }

    #[test]
    fn forced_failure_rolls_back_and_preserves_previous_schema() {
        let mut conn = Connection::open_in_memory().expect("open memory database");
        run_with(&mut conn, &MIGRATIONS[..1]).expect("run version one");

        let broken = Migration {
            version: 2,
            name: "broken test migration",
            sql: "CREATE TABLE should_rollback (id INTEGER PRIMARY KEY);\n\
                  INSERT INTO table_that_does_not_exist (id) VALUES (1);",
        };
        let error = run_with(&mut conn, &[MIGRATIONS[0], broken]).expect_err("migration must fail");
        assert!(matches!(error, StoreError::Migration { version: 2, .. }));

        assert_eq!(user_version(&conn), 1);
        assert!(!table_exists(&conn, "should_rollback"));
        let history: i64 = conn
            .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| row.get(0))
            .expect("count history");
        assert_eq!(history, 1);
    }

    #[test]
    fn rejects_a_newer_schema_without_downgrading() {
        let mut conn = Connection::open_in_memory().expect("open memory database");
        run_with(&mut conn, MIGRATIONS).expect("run migrations");
        conn.pragma_update(None, "user_version", LATEST_VERSION + 1)
            .expect("set newer version");

        let error = run_with(&mut conn, MIGRATIONS).expect_err("must reject newer schema");
        assert!(matches!(error, StoreError::UnsupportedSchema(_)));
        assert_eq!(user_version(&conn), LATEST_VERSION + 1);
    }
}
