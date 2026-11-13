//! Serialized SQLite access: one writer thread and a small read pool.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Sender};
use std::sync::{Mutex, MutexGuard};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use rusqlite::{Connection, OpenFlags};

use crate::error::{StoreError, StoreResult};
use crate::migrations;
use crate::util::unique_suffix;

type WriteJob = Box<dyn FnOnce(&mut Connection) + Send + 'static>;

struct WriterHandle {
    tx: Sender<WriteJob>,
    join: JoinHandle<()>,
}

/// SQLite database with a dedicated writer thread and pooled read connections.
pub(crate) struct Database {
    path: PathBuf,
    busy_timeout: Duration,
    cache_size_kib: i64,
    writer: Mutex<Option<WriterHandle>>,
    readers: Mutex<Vec<Connection>>,
}

impl Database {
    /// Opens, configures, and migrates the database, then starts the writer
    /// thread.
    pub(crate) fn open(path: &Path, busy_timeout: Duration, cache_size_kib: i64) -> StoreResult<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut connection = open_writer(path, busy_timeout, cache_size_kib)?;
        migrations::run(&mut connection)?;
        let writer = start_writer(connection)?;
        Ok(Self {
            path: path.to_path_buf(),
            busy_timeout,
            cache_size_kib,
            writer: Mutex::new(Some(writer)),
            readers: Mutex::new(Vec::new()),
        })
    }

    /// Absolute database path.
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    /// Runs a write closure on the dedicated writer thread.
    ///
    /// Closures must not call back into [`Database::write`], which would
    /// deadlock on the writer slot.
    pub(crate) fn write<T, F>(&self, operation: F) -> StoreResult<T>
    where
        T: Send + 'static,
        F: FnOnce(&mut Connection) -> StoreResult<T> + Send + 'static,
    {
        let guard = self.writer.lock().map_err(|_| StoreError::DatabaseWorkerStopped)?;
        let handle = guard.as_ref().ok_or(StoreError::DatabaseWorkerStopped)?;
        let (reply_tx, reply_rx) = mpsc::sync_channel(1);
        handle
            .tx
            .send(Box::new(move |connection| {
                let _ = reply_tx.send(operation(connection));
            }))
            .map_err(|_| StoreError::DatabaseWorkerStopped)?;
        drop(guard);
        reply_rx.recv().map_err(|_| StoreError::DatabaseWorkerStopped)?
    }

    /// Writes a consistent snapshot of the database to `destination`.
    ///
    /// `VACUUM INTO` runs on the writer connection, so it is serialized with
    /// all other writes and never attempted on a `query_only` reader.
    pub(crate) fn snapshot_to(&self, destination: &Path) -> StoreResult<()> {
        let destination = destination.to_string_lossy().into_owned();
        self.write(move |connection| {
            connection.execute("VACUUM INTO ?1", [destination])?;
            Ok(())
        })
    }

    /// Borrows a read connection from the pool for the duration of `operation`.
    pub(crate) fn read<T, F>(&self, operation: F) -> StoreResult<T>
    where
        F: FnOnce(&Connection) -> StoreResult<T>,
    {
        let mut readers = self.readers.lock().map_err(|_| StoreError::DatabaseWorkerStopped)?;
        let connection = match readers.pop() {
            Some(connection) => connection,
            None => open_reader(&self.path, self.busy_timeout, self.cache_size_kib)?,
        };
        let result = operation(&connection);
        readers.push(connection);
        result
    }

    /// Flushes the WAL and stops accepting work.
    fn close(&mut self) {
        let handle = self
            .writer
            .lock()
            .ok()
            .and_then(|mut guard: MutexGuard<'_, Option<WriterHandle>>| guard.take());
        if let Some(handle) = handle {
            let WriterHandle { tx, join } = handle;
            drop(tx);
            let _ = join.join();
        }
        if let Ok(mut readers) = self.readers.lock() {
            readers.clear();
        }
    }

    /// Replaces the database file with a validated staging copy.
    ///
    /// Current connections are closed first, the old database is renamed
    /// aside, and the new file is opened and migrated. On failure the old
    /// database is restored.
    pub(crate) fn replace_with(&mut self, staged: &Path) -> StoreResult<()> {
        if !staged.is_file() {
            return Err(StoreError::CorruptArchive("staged database is missing".to_owned()));
        }

        let _ = self.write(|connection| {
            connection.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
            Ok(())
        });
        self.close();

        let database_path = self.path.clone();
        let backup_path = append_suffix(&database_path, &format!(".pre-restore-{}", unique_suffix()));
        let backup_created = database_path.is_file();

        if backup_created {
            std::fs::rename(&database_path, &backup_path)?;
        }
        remove_sidecars(&database_path)?;

        if let Err(error) = std::fs::rename(staged, &database_path) {
            if backup_created {
                let _ = std::fs::rename(&backup_path, &database_path);
                let _ = self.reopen();
            }
            return Err(StoreError::Io(error));
        }

        match self.reopen() {
            Ok(()) => {
                if backup_created {
                    let _ = std::fs::remove_file(&backup_path);
                }
                Ok(())
            }
            Err(error) => {
                let _ = std::fs::remove_file(&database_path);
                if backup_created {
                    let _ = std::fs::rename(&backup_path, &database_path);
                }
                let _ = self.reopen();
                Err(error)
            }
        }
    }

    fn reopen(&mut self) -> StoreResult<()> {
        let mut connection = open_writer(&self.path, self.busy_timeout, self.cache_size_kib)?;
        migrations::run(&mut connection)?;
        let writer = start_writer(connection)?;
        if let Ok(mut guard) = self.writer.lock() {
            *guard = Some(writer);
        }
        Ok(())
    }
}

fn start_writer(mut connection: Connection) -> StoreResult<WriterHandle> {
    let (tx, rx) = mpsc::channel::<WriteJob>();
    let join = thread::Builder::new()
        .name("nexus-db-writer".to_owned())
        .spawn(move || {
            while let Ok(job) = rx.recv() {
                job(&mut connection);
            }
        })?;
    Ok(WriterHandle { tx, join })
}

fn open_writer(path: &Path, busy_timeout: Duration, cache_size_kib: i64) -> StoreResult<Connection> {
    let connection = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    configure_connection(&connection, busy_timeout, cache_size_kib)?;
    connection.pragma_update(None, "journal_mode", "WAL")?;
    connection.pragma_update(None, "synchronous", "FULL")?;
    connection.pragma_update(None, "wal_autocheckpoint", 1_000_i64)?;
    connection.pragma_update(None, "temp_store", "MEMORY")?;
    Ok(connection)
}

fn open_reader(path: &Path, busy_timeout: Duration, cache_size_kib: i64) -> StoreResult<Connection> {
    let connection =
        Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX)?;
    configure_connection(&connection, busy_timeout, cache_size_kib)?;
    connection.pragma_update(None, "query_only", true)?;
    Ok(connection)
}

fn configure_connection(connection: &Connection, busy_timeout: Duration, cache_size_kib: i64) -> StoreResult<()> {
    connection.busy_timeout(busy_timeout)?;
    connection.pragma_update(None, "foreign_keys", true)?;
    connection.pragma_update(None, "cache_size", -cache_size_kib)?;
    Ok(())
}

fn append_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut value = path.as_os_str().to_os_string();
    value.push(suffix);
    PathBuf::from(value)
}

fn remove_sidecars(path: &Path) -> StoreResult<()> {
    for suffix in ["-wal", "-shm"] {
        let sidecar = append_suffix(path, suffix);
        if sidecar.exists() {
            std::fs::remove_file(sidecar)?;
        }
    }
    Ok(())
}
