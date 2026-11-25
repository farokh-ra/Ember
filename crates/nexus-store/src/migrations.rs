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
    Migration {
        version: 3,
        name: "independent signed tombstones",
        sql: include_str!("../migrations/0003_signed_tombstones.sql"),
    },
    Migration {
        version: 4,
        name: "local import references",
        sql: include_str!("../migrations/0004_local_import_refs.sql"),
    },
    Migration {
        version: 5,
        name: "download transfer metadata",
        sql: include_str!("../migrations/0005_download_transfer_metadata.sql"),
    },
    Migration {
        version: 6,
        name: "signed profile cache and conflicts",
        sql: include_str!("../migrations/0006_profiles.sql"),
    },
    Migration {
        version: 7,
        name: "explicit publication serving state",
        sql: include_str!("../migrations/0007_publications.sql"),
    },
    Migration {
        version: 8,
        name: "replica verification and availability",
        sql: include_str!("../migrations/0008_replication.sql"),
    },
    Migration {
        version: 9,
        name: "live search index state",
        sql: include_str!("../migrations/0009_search_state.sql"),
    },
];

/// Newest schema version understood by this build.
pub(crate) const LATEST_VERSION: i64 = 9;

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
            "search_meta",
            "search_docs",
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
        assert_eq!(user_version(&conn), LATEST_VERSION);
        assert!(table_exists(&conn, "search_index"));

        let theme: String = conn
            .query_row("SELECT value FROM settings WHERE key = 'theme'", [], |row| row.get(0))
            .expect("read setting");
        assert_eq!(theme, "dark");
    }

    #[test]
    fn upgrades_version_two_tombstones_with_publication_attribution() {
        let mut conn = Connection::open_in_memory().expect("open memory database");
        run_with(&mut conn, &MIGRATIONS[..2]).expect("run version two");

        let content_id = "a".repeat(64);
        conn.execute(
            "INSERT INTO identity_refs (identity_id, public_key, key_file, label, created_at)
             VALUES ('nx1testidentity', zeroblob(32), 'secrets/identity.enc', NULL, 1)",
            [],
        )
        .expect("insert identity");
        conn.execute(
            "INSERT INTO contents (content_id, size_bytes, state, blob_path, verified_at, created_at, updated_at)
             VALUES (?1, 1, 'verified', 'blobs/aa/aa/blob', 1, 1, 1)",
            [&content_id],
        )
        .expect("insert content");
        conn.execute(
            "INSERT INTO publications
             (publication_id, identity_id, content_id, revision, record_bytes, title, description,
              filename, published_at, updated_at, tombstoned)
             VALUES ('pub-001', 'nx1testidentity', ?1, 1, x'01', 'Title', NULL, 'file.bin', 1, 1, 0)",
            [&content_id],
        )
        .expect("insert publication");
        conn.execute(
            "INSERT INTO tombstones (publication_id, revision, record_bytes, created_at)
             VALUES ('pub-001', 2, x'02', 5)",
            [],
        )
        .expect("insert version two tombstone");

        run_with(&mut conn, MIGRATIONS).expect("upgrade to latest schema");
        assert_eq!(user_version(&conn), LATEST_VERSION);
        let (identity, content, revision, created): (String, String, i64, i64) = conn
            .query_row(
                "SELECT identity_id, content_id, revision, created_at FROM tombstones WHERE publication_id = 'pub-001'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .expect("read upgraded tombstone");
        assert_eq!(identity, "nx1testidentity");
        assert_eq!(content, content_id);
        assert_eq!(revision, 2);
        assert_eq!(created, 5);
    }

    #[test]
    fn upgrades_version_six_publications_with_explicit_serving_state() {
        let mut conn = Connection::open_in_memory().expect("open memory database");
        run_with(&mut conn, &MIGRATIONS[..6]).expect("run version six");

        let content_id = "a".repeat(64);
        conn.execute(
            "INSERT INTO identity_refs (identity_id, public_key, key_file, label, created_at)
             VALUES ('nx1v7identity', zeroblob(32), 'secrets/identity.enc', NULL, 1)",
            [],
        )
        .expect("insert identity");
        conn.execute(
            "INSERT INTO contents (content_id, size_bytes, state, blob_path, verified_at, created_at, updated_at)
             VALUES (?1, 4321, 'verified', 'blobs/aa/aa/blob', 1, 1, 1)",
            [&content_id],
        )
        .expect("insert content");
        for (publication_id, tombstoned) in [("pub-live", 0_i64), ("pub-dead", 1_i64)] {
            conn.execute(
                "INSERT INTO publications
                 (publication_id, identity_id, content_id, revision, record_bytes, title, description,
                  filename, published_at, updated_at, tombstoned)
                 VALUES (?1, 'nx1v7identity', ?2, 1, x'01', 'Title', NULL, 'file.bin', 10, 10, ?3)",
                rusqlite::params![publication_id, &content_id, tombstoned],
            )
            .expect("insert publication");
        }
        conn.execute(
            "INSERT INTO replicas
             (replica_id, content_id, publication_id, state, last_verified_at, created_at, updated_at)
             VALUES ('replica-1', ?1, 'pub-live', 'active', 2, 1, 2)",
            [&content_id],
        )
        .expect("insert replica");

        run_with(&mut conn, MIGRATIONS).expect("upgrade to latest schema");
        assert_eq!(user_version(&conn), LATEST_VERSION);
        assert!(table_exists(&conn, "publication_tags"));

        let live: (String, i64, i64, i64, i64, String) = conn
            .query_row(
                "SELECT category, size_bytes, local, serving, announced, availability
                 FROM publications WHERE publication_id = 'pub-live'",
                [],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                    ))
                },
            )
            .expect("read live publication");
        assert_eq!(live, ("other".to_owned(), 4321, 1, 1, 0, "local".to_owned()));

        let dead: (i64, i64, i64, String) = conn
            .query_row(
                "SELECT local, serving, announced, availability
                 FROM publications WHERE publication_id = 'pub-dead'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .expect("read tombstoned publication");
        assert_eq!(dead, (1, 0, 0, "record_only".to_owned()));

        let replica: (String, String) = conn
            .query_row(
                "SELECT publication_id, state FROM replicas WHERE replica_id = 'replica-1'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("read replica");
        assert_eq!(replica, ("pub-live".to_owned(), "active".to_owned()));

        // Publication attribution on a replica is no longer a foreign key.
        conn.execute(
            "INSERT INTO replicas
             (replica_id, content_id, publication_id, state, last_verified_at, created_at, updated_at)
             VALUES ('replica-2', ?1, 'pub-not-cached', 'active', NULL, 3, 3)",
            [&content_id],
        )
        .expect("replica may reference a publication not cached locally");
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
