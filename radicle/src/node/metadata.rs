use std::path::Path;
use std::{fmt, time};

use sqlite as sql;
use thiserror::Error;

use crate::prelude::{NodeId, Timestamp};

/// How long to wait for the database lock to be released before failing a read.
const DB_READ_TIMEOUT: time::Duration = time::Duration::from_secs(3);
/// How long to wait for the database lock to be released before failing a write.
const DB_WRITE_TIMEOUT: time::Duration = time::Duration::from_secs(6);

/// An error occuring in peer-to-peer networking code.
#[derive(Error, Debug)]
pub enum Error {
    /// An Internal error.
    #[error("internal error: {0}")]
    Internal(#[from] sql::Error),

    /// Internal unit overflow.
    #[error("the unit overflowed")]
    UnitOverflow,
}

/// Persistent file storage for a routing table.
pub struct Metadata {
    db: sql::Connection,
}

impl fmt::Debug for Metadata {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Table(..)")
    }
}

impl Metadata {
    const SCHEMA: &str = include_str!("metadata/schema.sql");

    /// Open a routing file store at the given path. Creates a new empty store
    /// if an existing store isn't found.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, Error> {
        let mut db = sql::Connection::open(path)?;
        db.set_busy_timeout(DB_WRITE_TIMEOUT.as_millis() as usize)?;
        db.execute(Self::SCHEMA)?;
        Ok(Self { db })
    }

    /// Same as [`Self::open`], but in read-only mode. This is useful to have multiple
    /// open databases, as no locking is required.
    pub fn reader<P: AsRef<Path>>(path: P) -> Result<Self, Error> {
        let mut db =
            sql::Connection::open_with_flags(path, sqlite::OpenFlags::new().set_read_only())?;
        db.set_busy_timeout(DB_READ_TIMEOUT.as_millis() as usize)?;
        db.execute(Self::SCHEMA)?;
        Ok(Self { db })
    }

    /// Create a new in-memory routing table.
    pub fn memory() -> Result<Self, Error> {
        let db = sql::Connection::open(":memory:")?;
        db.execute(Self::SCHEMA)?;
        Ok(Self { db })
    }
}

pub trait Store {
    fn get_last_accounce(&self, node: NodeId) -> Result<(NodeId, u64), Error>;
    fn entries(&self) -> Result<Box<dyn Iterator<Item = (NodeId, Timestamp)>>, Error>;
    fn prune(&mut self, oldest: Timestamp, limit: Option<usize>) -> Result<usize, Error>;
    fn insert(&mut self, node: NodeId, time: Timestamp) -> Result<(), Error>;
}

impl Store for Metadata {
    fn get_last_accounce(&self, node: NodeId) -> Result<(NodeId, u64), Error> {
        unimplemented!()
    }

    fn entries(&self) -> Result<Box<dyn Iterator<Item = (NodeId, Timestamp)>>, Error> {
        unimplemented!()
    }

    fn insert(&mut self, node: NodeId, time: Timestamp) -> Result<(), Error> {
        unimplemented!()
    }

    fn prune(&mut self, oldest: Timestamp, limit: Option<usize>) -> Result<usize, Error> {
        unimplemented!()
    }
}
