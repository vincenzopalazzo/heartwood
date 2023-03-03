// Copyright © 2022 The Radicle Link Contributors
//
// This file is part of radicle-link, distributed under the GPLv3 with Radicle
// Linking Exception. For full terms see the included LICENSE file.

use std::{convert::TryFrom as _, fmt, str::FromStr};

use git_ext::Oid;
use git_ref_format::{Component, RefString};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub mod collaboration;
pub use collaboration::{
    create, get, info, list, parse_refstr, remove, update, CollaborativeObject, Create, Update,
};

pub mod storage;
pub use storage::{Commit, Objects, Reference, Storage};

#[derive(Debug, Error)]
pub enum ParseObjectId {
    #[error(transparent)]
    Git(#[from] git2::Error),
}

/// The id of an object
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObjectId {
    // since we use the short object id 
    // the from call from the oid return a not 
    // complect id, so this make the oid private 
    // and prevent people to work with this accidentaly.
    oid: Oid,
}

impl ObjectId {
    pub fn new(oid: Oid) -> Self {
        ObjectId { oid }
    }

    pub fn to_short_obj(&self) -> ObjectId {
        let short = self.to_string();
        ObjectId::from_str(&short).unwrap()
    }
}

impl FromStr for ObjectId {
    type Err = ParseObjectId;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let oid = Oid::from_str(s)?;
        Ok(ObjectId::new(oid))
    }
}

impl From<Oid> for ObjectId {
    fn from(oid: Oid) -> Self {
        ObjectId::new(oid)
    }
}

impl From<&Oid> for ObjectId {
    fn from(oid: &Oid) -> Self {
        (*oid).into()
    }
}

impl From<git2::Oid> for ObjectId {
    fn from(oid: git2::Oid) -> Self {
        Oid::from(oid).into()
    }
}

impl From<&git2::Oid> for ObjectId {
    fn from(oid: &git2::Oid) -> Self {
        ObjectId::from(*oid)
    }
}

impl fmt::Display for ObjectId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.7}", self.oid)
    }
}

impl Serialize for ObjectId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        // FIXME: verify if the call as byte on to string is the same
        serializer.serialize_bytes(self.to_string().as_bytes())
    }
}

impl<'de> Deserialize<'de> for ObjectId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = <&[u8]>::deserialize(deserializer)?;
        let oid = Oid::try_from(raw).map_err(serde::de::Error::custom)?;
        Ok(ObjectId::new(oid))
    }
}

impl From<&ObjectId> for Component<'_> {
    fn from(id: &ObjectId) -> Self {
        let refstr = RefString::try_from(id.to_string())
            .expect("collaborative object id's are valid ref strings");
        Component::from_refstr(refstr)
            .expect("collaborative object id's are valid refname components")
    }
}
