mod features;

pub mod address;
pub mod config;
pub mod events;
pub mod routing;
pub mod tracking;

use std::collections::{BTreeSet, HashMap, HashSet};
use std::io::{BufRead, BufReader};
use std::ops::Deref;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::{fmt, io, net, thread, time};

use amplify::WrapperMut;
use cyphernet::addr::{HostName, NetAddr};
use localtime::LocalTime;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json as json;

use crate::crypto::PublicKey;
use crate::identity::Id;
use crate::node::address::Store as _;
use crate::node::routing::Store as _;
use crate::storage::RefUpdate;

pub use config::Config;
pub use cyphernet::addr::PeerAddr;
pub use events::{Event, Events};
pub use features::Features;

/// Default name for control socket file.
pub const DEFAULT_SOCKET_NAME: &str = "control.sock";
/// Default radicle protocol port.
pub const DEFAULT_PORT: u16 = 8776;
/// Default timeout when waiting for the node to respond with data.
pub const DEFAULT_TIMEOUT: time::Duration = time::Duration::from_secs(9);
/// Maximum length in bytes of a node alias.
pub const MAX_ALIAS_LENGTH: usize = 32;
/// Filename of routing table database under the node directory.
pub const ROUTING_DB_FILE: &str = "routing.db";
/// Filename of address database under the node directory.
pub const ADDRESS_DB_FILE: &str = "addresses.db";
/// Filename of tracking table database under the node directory.
pub const TRACKING_DB_FILE: &str = "tracking.db";
/// Filename of last node announcement, when running in debug mode.
#[cfg(debug_assertions)]
pub const NODE_ANNOUNCEMENT_FILE: &str = "announcement.wire.debug";
/// Filename of last node announcement.
#[cfg(not(debug_assertions))]
pub const NODE_ANNOUNCEMENT_FILE: &str = "announcement.wire";

/// Milliseconds since epoch.
pub type Timestamp = u64;

#[derive(Debug, Copy, Clone, Default, PartialEq, Eq)]
pub enum PingState {
    #[default]
    /// The peer has not been sent a ping.
    None,
    /// A ping has been sent and is waiting on the peer's response.
    AwaitingResponse(u16),
    /// The peer was successfully pinged.
    Ok,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(clippy::large_enum_variant)]
pub enum State {
    /// Initial state for outgoing connections.
    Initial,
    /// Connection attempted successfully.
    Attempted { addr: Address },
    /// Initial state after handshake protocol hand-off.
    Connected {
        /// Remote address.
        addr: Address,
        /// Connected since this time.
        since: LocalTime,
        /// Ping state.
        #[serde(skip)]
        ping: PingState,
        /// Ongoing fetches.
        fetching: HashSet<Id>,
    },
    /// When a peer is disconnected.
    Disconnected {
        /// Since when has this peer been disconnected.
        since: LocalTime,
        /// When to retry the connection.
        retry_at: LocalTime,
    },
}

impl fmt::Display for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Initial => {
                write!(f, "initial")
            }
            Self::Attempted { .. } => {
                write!(f, "attempted")
            }
            Self::Connected { .. } => {
                write!(f, "connected")
            }
            Self::Disconnected { .. } => {
                write!(f, "disconnected")
            }
        }
    }
}

/// Node alias.
#[derive(Debug, PartialEq, Eq, Clone, serde::Serialize, serde::Deserialize)]
pub struct Alias(String);

impl Alias {
    /// Create a new alias from a string. Panics if the string is not a valid alias.
    pub fn new(alias: impl ToString) -> Self {
        let alias = alias.to_string();

        match Self::from_str(&alias) {
            Ok(a) => a,
            Err(e) => panic!("Alias::new: {e}"),
        }
    }
}

impl From<Alias> for String {
    fn from(value: Alias) -> Self {
        value.0
    }
}

impl From<&NodeId> for Alias {
    fn from(nid: &NodeId) -> Self {
        Alias(nid.to_string())
    }
}

impl fmt::Display for Alias {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl Deref for Alias {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<str> for Alias {
    fn as_ref(&self) -> &str {
        self.0.as_str()
    }
}

impl From<&Alias> for [u8; 32] {
    fn from(input: &Alias) -> [u8; 32] {
        let mut alias = [0u8; 32];

        alias[..input.len()].copy_from_slice(input.as_bytes());
        alias
    }
}

#[derive(thiserror::Error, Debug)]
pub enum AliasError {
    #[error("alias cannot be empty")]
    Empty,
    #[error("alias cannot be greater than {MAX_ALIAS_LENGTH} bytes")]
    MaxBytesExceeded,
    #[error("alias cannot contain whitespace or control characters")]
    InvalidCharacter,
}

impl FromStr for Alias {
    type Err = AliasError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.is_empty() {
            return Err(AliasError::Empty);
        }
        if s.chars().any(|c| c.is_control() || c.is_whitespace()) {
            return Err(AliasError::InvalidCharacter);
        }
        if s.len() > MAX_ALIAS_LENGTH {
            return Err(AliasError::MaxBytesExceeded);
        }
        Ok(Self(s.to_owned()))
    }
}

/// Result of a command, on the node control socket.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status")]
pub enum CommandResult {
    /// Response on node socket indicating that a command was carried out successfully.
    #[serde(rename = "ok")]
    Okay {
        /// Whether the command had any effect.
        #[serde(default, skip_serializing_if = "crate::serde_ext::is_default")]
        updated: bool,
    },
    /// Response on node socket indicating that an error occured.
    Error {
        /// The reason for the error.
        reason: String,
    },
}

impl CommandResult {
    /// Create an "updated" response.
    pub fn updated() -> Self {
        Self::Okay { updated: true }
    }

    /// Create an "ok" response.
    pub fn ok() -> Self {
        Self::Okay { updated: false }
    }

    /// Create an error result.
    pub fn error(err: impl std::error::Error) -> Self {
        Self::Error {
            reason: err.to_string(),
        }
    }

    /// Write this command result to a stream, including a terminating LF character.
    pub fn to_writer(&self, mut w: impl io::Write) -> io::Result<()> {
        json::to_writer(&mut w, self).map_err(|_| io::ErrorKind::InvalidInput)?;
        w.write_all(b"\n")
    }
}

impl From<CommandResult> for Result<bool, Error> {
    fn from(value: CommandResult) -> Self {
        match value {
            CommandResult::Okay { updated } => Ok(updated),
            CommandResult::Error { reason } => Err(Error::Node(reason)),
        }
    }
}

/// Peer public protocol address.
#[derive(Wrapper, WrapperMut, Clone, Eq, PartialEq, Debug, From, Serialize, Deserialize)]
#[wrapper(Deref, Display, FromStr)]
#[wrapper_mut(DerefMut)]
pub struct Address(#[serde(with = "crate::serde_ext::string")] NetAddr<HostName>);

impl cyphernet::addr::Host for Address {
    fn requires_proxy(&self) -> bool {
        self.0.requires_proxy()
    }
}

impl cyphernet::addr::Addr for Address {
    fn port(&self) -> u16 {
        self.0.port()
    }
}

impl From<net::SocketAddr> for Address {
    fn from(addr: net::SocketAddr) -> Self {
        Address(NetAddr {
            host: HostName::Ip(addr.ip()),
            port: addr.port(),
        })
    }
}

/// Command name.
#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CommandName {
    /// Announce repository references for given repository to peers.
    AnnounceRefs,
    /// Announce local repositories to peers.
    AnnounceInventory,
    /// Sync local inventory with node.
    SyncInventory,
    /// Connect to node with the given address.
    Connect,
    /// Lookup seeds for the given repository in the routing table.
    Seeds,
    /// Get the current peer sessions.
    Sessions,
    /// Fetch the given repository from the network.
    Fetch,
    /// Track the given repository.
    TrackRepo,
    /// Untrack the given repository.
    UntrackRepo,
    /// Track the given node.
    TrackNode,
    /// Untrack the given node.
    UntrackNode,
    /// Get the node's status.
    Status,
    /// Get the node's NID.
    NodeId,
    /// Shutdown the node.
    Shutdown,
    /// Subscribe to events.
    Subscribe,
}

impl fmt::Display for CommandName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // SAFETY: The enum can always be converted to a value.
        #[allow(clippy::unwrap_used)]
        let val = json::to_value(self).unwrap();
        // SAFETY: The value is always a string.
        #[allow(clippy::unwrap_used)]
        let s = val.as_str().unwrap();

        write!(f, "{s}")
    }
}

/// Commands sent to the node via the control socket.
#[derive(Debug, Serialize, Deserialize)]
pub struct Command {
    /// Command name.
    #[serde(rename = "cmd")]
    pub name: CommandName,
    /// Command arguments.
    #[serde(rename = "args")]
    pub args: Vec<String>,
}

impl Command {
    /// Shutdown command.
    pub const SHUTDOWN: Self = Self {
        name: CommandName::Shutdown,
        args: vec![],
    };

    /// Create a new command.
    pub fn new<T: ToString>(name: CommandName, args: impl IntoIterator<Item = T>) -> Self {
        Self {
            name,
            args: args.into_iter().map(|a| a.to_string()).collect(),
        }
    }

    /// Write this command to a stream, including a terminating LF character.
    pub fn to_writer(&self, mut w: impl io::Write) -> io::Result<()> {
        json::to_writer(&mut w, self).map_err(|_| io::ErrorKind::InvalidInput)?;
        w.write_all(b"\n")
    }
}

/// An established network connection with a peer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub nid: NodeId,
    pub state: State,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
#[serde(tag = "state", content = "id")]
pub enum Seed {
    Disconnected(NodeId),
    Connected(NodeId),
}

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Seeds(BTreeSet<Seed>);

impl Seeds {
    pub fn insert(&mut self, seed: Seed) {
        self.0.insert(seed);
    }

    pub fn connected(&self) -> impl Iterator<Item = &NodeId> {
        self.0.iter().filter_map(|s| match s {
            Seed::Connected(node) => Some(node),
            Seed::Disconnected(_) => None,
        })
    }

    pub fn disconnected(&self) -> impl Iterator<Item = &NodeId> {
        self.0.iter().filter_map(|s| match s {
            Seed::Disconnected(node) => Some(node),
            Seed::Connected(_) => None,
        })
    }

    pub fn has_connections(&self) -> bool {
        self.0.iter().any(|s| match s {
            Seed::Connected(_) => true,
            Seed::Disconnected(_) => false,
        })
    }

    pub fn is_connected(&self, node: &NodeId) -> bool {
        self.0.contains(&Seed::Connected(*node))
    }

    pub fn is_disconnected(&self, node: &NodeId) -> bool {
        self.0.contains(&Seed::Disconnected(*node))
    }
}

/// Announcement result returned by [`Node::announce`].
pub struct AnnounceResult {
    /// Nodes that timed out.
    pub timeout: Vec<NodeId>,
    /// Nodes that synced.
    pub synced: Vec<NodeId>,
}

/// A sync event, emitted by [`Node::announce`].
pub enum AnnounceEvent {
    /// Refs were synced with the given node.
    RefsSynced { remote: NodeId },
    /// Refs were announced to all given nodes.
    Announced,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "kebab-case")]
pub enum FetchResult {
    Success {
        updated: Vec<RefUpdate>,
        namespaces: HashSet<NodeId>,
    },
    // TODO: Create enum for reason.
    Failed {
        reason: String,
    },
}

impl FetchResult {
    pub fn is_success(&self) -> bool {
        matches!(self, FetchResult::Success { .. })
    }

    pub fn success(self) -> Option<(Vec<RefUpdate>, HashSet<NodeId>)> {
        match self {
            Self::Success {
                updated,
                namespaces,
            } => Some((updated, namespaces)),
            _ => None,
        }
    }
}

impl<S: ToString> From<Result<(Vec<RefUpdate>, HashSet<NodeId>), S>> for FetchResult {
    fn from(value: Result<(Vec<RefUpdate>, HashSet<NodeId>), S>) -> Self {
        match value {
            Ok((updated, namespaces)) => Self::Success {
                updated,
                namespaces,
            },
            Err(err) => Self::Failed {
                reason: err.to_string(),
            },
        }
    }
}

/// Holds multiple fetch results.
#[derive(Debug, Default)]
pub struct FetchResults(Vec<(NodeId, FetchResult)>);

impl FetchResults {
    /// Push a fetch result.
    pub fn push(&mut self, nid: NodeId, result: FetchResult) {
        self.0.push((nid, result));
    }

    /// Iterate over all fetch results.
    pub fn iter(&self) -> impl Iterator<Item = (&NodeId, &FetchResult)> {
        self.0.iter().map(|(nid, r)| (nid, r))
    }

    /// Iterate over successful fetches.
    pub fn success(&self) -> impl Iterator<Item = (&NodeId, &[RefUpdate], HashSet<NodeId>)> {
        self.0.iter().filter_map(|(nid, r)| {
            if let FetchResult::Success {
                updated,
                namespaces,
            } = r
            {
                Some((nid, updated.as_slice(), namespaces.clone()))
            } else {
                None
            }
        })
    }

    /// Iterate over failed fetches.
    pub fn failed(&self) -> impl Iterator<Item = (&NodeId, &str)> {
        self.0.iter().filter_map(|(nid, r)| {
            if let FetchResult::Failed { reason } = r {
                Some((nid, reason.as_str()))
            } else {
                None
            }
        })
    }
}

impl From<Vec<(NodeId, FetchResult)>> for FetchResults {
    fn from(value: Vec<(NodeId, FetchResult)>) -> Self {
        Self(value)
    }
}

impl Deref for FetchResults {
    type Target = [(NodeId, FetchResult)];

    fn deref(&self) -> &Self::Target {
        self.0.as_slice()
    }
}

impl IntoIterator for FetchResults {
    type Item = (NodeId, FetchResult);
    type IntoIter = std::vec::IntoIter<(NodeId, FetchResult)>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

/// Error returned by [`Handle`] functions.
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("failed to connect to node: {0}")]
    Connect(#[from] io::Error),
    #[error("failed to call node: {0}")]
    Call(#[from] CallError),
    #[error("{0}")]
    Routing(#[from] routing::Error),
    #[error("{0}")]
    Address(#[from] address::Error),
    #[error("node: {0}")]
    Node(String),
    #[error("received empty response for `{cmd}` command")]
    EmptyResponse { cmd: CommandName },
}

impl Error {
    /// Check if the error is due to the not being able to connect to the local node.
    pub fn is_connection_err(&self) -> bool {
        matches!(self, Self::Connect(_))
    }
}

/// Error returned by [`Node::call`] iterator.
#[derive(thiserror::Error, Debug)]
pub enum CallError {
    #[error("i/o: {0}")]
    Io(#[from] io::Error),
    #[error("received invalid json in response for `{cmd}` command: '{response}': {error}")]
    InvalidJson {
        cmd: CommandName,
        response: String,
        error: json::Error,
    },
}

/// A handle to send commands to the node or request information.
pub trait Handle: Clone + Sync + Send {
    /// The peer sessions type.
    type Sessions;
    /// The error returned by all methods.
    type Error: std::error::Error + Send + Sync + 'static;

    /// Get the local Node ID.
    fn nid(&self) -> Result<NodeId, Self::Error>;
    /// Check if the node is running. to a peer.
    fn is_running(&self) -> bool;
    /// Connect to a peer.
    fn connect(&mut self, node: NodeId, addr: Address) -> Result<(), Self::Error>;
    /// Lookup the seeds of a given repository in the routing table.
    fn seeds(&mut self, id: Id) -> Result<Seeds, Self::Error>;
    /// Fetch a repository from the network.
    fn fetch(&mut self, id: Id, from: NodeId) -> Result<FetchResult, Self::Error>;
    /// Start tracking the given project. Doesn't do anything if the project is already
    /// tracked.
    fn track_repo(&mut self, id: Id, scope: tracking::Scope) -> Result<bool, Self::Error>;
    /// Start tracking the given node.
    fn track_node(&mut self, id: NodeId, alias: Option<Alias>) -> Result<bool, Self::Error>;
    /// Untrack the given project and delete it from storage.
    fn untrack_repo(&mut self, id: Id) -> Result<bool, Self::Error>;
    /// Untrack the given node.
    fn untrack_node(&mut self, id: NodeId) -> Result<bool, Self::Error>;
    /// Notify the service that a project has been updated, and announce local refs.
    fn announce_refs(&mut self, id: Id) -> Result<(), Self::Error>;
    /// Announce local inventory.
    fn announce_inventory(&mut self) -> Result<(), Self::Error>;
    /// Notify the service that our inventory was updated.
    fn sync_inventory(&mut self) -> Result<bool, Self::Error>;
    /// Ask the service to shutdown.
    fn shutdown(self) -> Result<(), Self::Error>;
    /// Query the peer session state.
    fn sessions(&self) -> Result<Self::Sessions, Self::Error>;
    /// Subscribe to node events.
    fn subscribe(
        &self,
        timeout: time::Duration,
    ) -> Result<Box<dyn Iterator<Item = Result<Event, io::Error>>>, Self::Error>;
}

/// Public node & device identifier.
pub type NodeId = PublicKey;

pub enum RefAnnouncement {
    Store,
    Forwarded,
}

/// Node controller.
#[derive(Debug, Clone)]
pub struct Node {
    socket: PathBuf,
}

impl Node {
    /// Connect to the node, via the socket at the given path.
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            socket: path.as_ref().to_path_buf(),
        }
    }

    /// Call a command on the node.
    pub fn call<A: ToString, T: DeserializeOwned>(
        &self,
        name: CommandName,
        args: impl IntoIterator<Item = A>,
        timeout: time::Duration,
    ) -> Result<impl Iterator<Item = Result<T, CallError>>, io::Error> {
        let stream = UnixStream::connect(&self.socket)?;
        Command::new(name, args).to_writer(&stream)?;

        stream.set_read_timeout(Some(timeout))?;

        Ok(BufReader::new(stream).lines().map(move |l| {
            let l = l?;
            let v = json::from_str(&l).map_err(|e| CallError::InvalidJson {
                cmd: name,
                response: l,
                error: e,
            })?;

            Ok(v)
        }))
    }

    /// Announce refs of the given `rid` to the given seeds.
    /// Waits for the seeds to acknowledge the refs or times out if no acknowledgments are received
    /// within the given time.
    pub fn announce(
        &mut self,
        rid: Id,
        seeds: impl IntoIterator<Item = NodeId>,
        timeout: time::Duration,
        mut callback: impl FnMut(AnnounceEvent),
    ) -> Result<AnnounceResult, Error> {
        let events = self.subscribe(timeout)?;
        let mut seeds = seeds.into_iter().collect::<BTreeSet<_>>();

        self.announce_refs(rid)?;

        callback(AnnounceEvent::Announced);

        let mut synced = Vec::new();
        let mut timeout: Vec<NodeId> = Vec::new();

        for e in events {
            match e {
                Ok(Event::RefsSynced { remote, rid: rid_ }) if rid == rid_ => {
                    seeds.remove(&remote);
                    synced.push(remote);

                    callback(AnnounceEvent::RefsSynced { remote });
                }
                Ok(_) => {}

                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    timeout.extend(seeds.iter());
                    break;
                }
                Err(e) => return Err(e.into()),
            }
            if seeds.is_empty() {
                break;
            }
        }
        Ok(AnnounceResult { timeout, synced })
    }

    /// Try to Announce refs of the given `rid` if the node is running,
    /// otherwise store the minimal information to re-announce when the node
    /// will start.
    pub fn try_announce_refs(&mut self, rid: Id, path: &Path) -> Result<RefAnnouncement, Error> {
        if self.is_running() {
            self.announce_refs(rid)?;
            return Ok(RefAnnouncement::Forwarded);
        }
        let address_db = path.join(ADDRESS_DB_FILE);
        let routing_db = routing::Table::open(path.join(ROUTING_DB_FILE))?;
        let seeds = routing_db.get(&rid)?;
        let addresses = address::Book::open(address_db)?;
        // FIXME: use SQL transaction!
        for seed in seeds.into_iter() {
            addresses.insert_ref_announcement(&seed, &rid, LocalTime::now().as_millis(), None)?;
        }
        Ok(RefAnnouncement::Store)
    }
}

// TODO(finto): repo_policies, node_policies, and routing should all
// attempt to return iterators instead of allocating vecs.
impl Handle for Node {
    type Sessions = Vec<Session>;
    type Error = Error;

    fn nid(&self) -> Result<NodeId, Error> {
        self.call::<&str, NodeId>(CommandName::NodeId, [], DEFAULT_TIMEOUT)?
            .next()
            .ok_or(Error::EmptyResponse {
                cmd: CommandName::NodeId,
            })?
            .map_err(Error::from)
    }

    fn is_running(&self) -> bool {
        let Ok(mut lines) = self.call::<&str, CommandResult>(CommandName::Status, [], DEFAULT_TIMEOUT) else {
            return false;
        };
        let Some(Ok(result)) = lines.next() else {
            return false;
        };
        matches!(result, CommandResult::Okay { .. })
    }

    fn connect(&mut self, nid: NodeId, addr: Address) -> Result<(), Error> {
        self.call::<_, CommandResult>(
            CommandName::Connect,
            [nid.to_human(), addr.to_string()],
            DEFAULT_TIMEOUT,
        )?
        .next()
        .ok_or(Error::EmptyResponse {
            cmd: CommandName::Connect,
        })??;

        Ok(())
    }

    fn seeds(&mut self, id: Id) -> Result<Seeds, Error> {
        let seeds: Seeds = self
            .call(CommandName::Seeds, [id.urn()], DEFAULT_TIMEOUT)?
            .next()
            .ok_or(Error::EmptyResponse {
                cmd: CommandName::Seeds,
            })??;

        Ok(seeds)
    }

    fn fetch(&mut self, id: Id, from: NodeId) -> Result<FetchResult, Error> {
        let result = self
            .call(
                CommandName::Fetch,
                [id.urn(), from.to_human()],
                DEFAULT_TIMEOUT,
            )?
            .next()
            .ok_or(Error::EmptyResponse {
                cmd: CommandName::Fetch,
            })??;

        Ok(result)
    }

    fn track_node(&mut self, id: NodeId, alias: Option<Alias>) -> Result<bool, Error> {
        let id = id.to_human();
        let args = if let Some(alias) = alias.as_deref() {
            vec![id.as_str(), alias]
        } else {
            vec![id.as_str()]
        };

        let mut line = self.call(CommandName::TrackNode, args, DEFAULT_TIMEOUT)?;
        let response: CommandResult = line.next().ok_or(Error::EmptyResponse {
            cmd: CommandName::TrackNode,
        })??;

        response.into()
    }

    fn track_repo(&mut self, id: Id, scope: tracking::Scope) -> Result<bool, Error> {
        let mut line = self.call(
            CommandName::TrackRepo,
            [id.urn(), scope.to_string()],
            DEFAULT_TIMEOUT,
        )?;
        let response: CommandResult = line.next().ok_or(Error::EmptyResponse {
            cmd: CommandName::TrackRepo,
        })??;

        response.into()
    }

    fn untrack_node(&mut self, id: NodeId) -> Result<bool, Error> {
        let mut line = self.call(CommandName::UntrackNode, [id], DEFAULT_TIMEOUT)?;
        let response: CommandResult = line.next().ok_or(Error::EmptyResponse {
            cmd: CommandName::UntrackNode,
        })??;

        response.into()
    }

    fn untrack_repo(&mut self, id: Id) -> Result<bool, Error> {
        let mut line = self.call(CommandName::UntrackRepo, [id.urn()], DEFAULT_TIMEOUT)?;
        let response: CommandResult = line.next().ok_or(Error::EmptyResponse {
            cmd: CommandName::UntrackRepo,
        })??;

        response.into()
    }

    fn announce_refs(&mut self, id: Id) -> Result<(), Error> {
        for line in
            self.call::<_, CommandResult>(CommandName::AnnounceRefs, [id.urn()], DEFAULT_TIMEOUT)?
        {
            line?;
        }
        Ok(())
    }

    fn announce_inventory(&mut self) -> Result<(), Error> {
        for line in
            self.call::<&str, CommandResult>(CommandName::AnnounceInventory, [], DEFAULT_TIMEOUT)?
        {
            line?;
        }
        Ok(())
    }

    fn sync_inventory(&mut self) -> Result<bool, Error> {
        let mut line = self.call::<&str, _>(CommandName::SyncInventory, [], DEFAULT_TIMEOUT)?;
        let response: CommandResult = line.next().ok_or(Error::EmptyResponse {
            cmd: CommandName::SyncInventory,
        })??;

        response.into()
    }

    fn subscribe(
        &self,
        timeout: time::Duration,
    ) -> Result<Box<dyn Iterator<Item = Result<Event, io::Error>>>, Error> {
        let events = self.call::<&str, _>(CommandName::Subscribe, [], timeout)?;

        Ok(Box::new(events.map(|e| {
            e.map_err(|err| match err {
                CallError::Io(e) => e,
                CallError::InvalidJson { .. } => {
                    io::Error::new(io::ErrorKind::InvalidInput, err.to_string())
                }
            })
        })))
    }

    fn sessions(&self) -> Result<Self::Sessions, Error> {
        let sessions = self
            .call::<&str, Vec<Session>>(CommandName::Sessions, [], DEFAULT_TIMEOUT)?
            .next()
            .ok_or(Error::EmptyResponse {
                cmd: CommandName::Sessions,
            })??;

        Ok(sessions)
    }

    fn shutdown(self) -> Result<(), Error> {
        for line in self.call::<&str, CommandResult>(CommandName::Shutdown, [], DEFAULT_TIMEOUT)? {
            line?;
        }
        // Wait until the shutdown has completed.
        while self.is_running() {
            thread::sleep(time::Duration::from_secs(1));
        }
        Ok(())
    }
}

/// A trait for different sources which can potentially return an alias.
pub trait AliasStore {
    /// Returns alias of a `NodeId`.
    fn alias(&self, nid: &NodeId) -> Option<Alias>;
}

impl<T: AliasStore + ?Sized> AliasStore for &T {
    fn alias(&self, nid: &NodeId) -> Option<Alias> {
        (*self).alias(nid)
    }
}

impl<T: AliasStore + ?Sized> AliasStore for Box<T> {
    fn alias(&self, nid: &NodeId) -> Option<Alias> {
        self.deref().alias(nid)
    }
}

impl AliasStore for HashMap<NodeId, Alias> {
    fn alias(&self, nid: &NodeId) -> Option<Alias> {
        self.get(nid).map(ToOwned::to_owned)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_command_name_display() {
        assert_eq!(CommandName::TrackNode.to_string(), "track-node");
    }

    #[test]
    fn test_alias() {
        assert!(Alias::from_str("cloudhead").is_ok());
        assert!(Alias::from_str("cloud-head").is_ok());
        assert!(Alias::from_str("cl0ud.h3ad$__").is_ok());
        assert!(Alias::from_str("©loudhèâd").is_ok());

        assert!(Alias::from_str("").is_err());
        assert!(Alias::from_str(" ").is_err());
        assert!(Alias::from_str("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").is_err());
        assert!(Alias::from_str("cloud\0head").is_err());
        assert!(Alias::from_str("cloud head").is_err());
        assert!(Alias::from_str("cloudhead\n").is_err());
    }
}
