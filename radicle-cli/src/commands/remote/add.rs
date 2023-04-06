use radicle::{git::Url, Profile, node::TRACKING_DB_FILE, prelude::{Did, Id}};
use radicle_crypto::PublicKey;
use radicle::node::tracking::store::Config;

use crate::{git, terminal as term};

/// core command to run the `rad remote add ...` subcommand.
pub fn run(repository: &git::Repository, profile: &Profile, did: &Did, id: Id) -> anyhow::Result<()> {
    let pubkey = PublicKey(did.0);
    let Some(alias) = lookup_for_alias(profile, &pubkey)? else {
        anyhow::bail!("an alias need to be specified");
    };
    if lookup_for_remote(repository, &alias)? {
        anyhow::bail!("remote with did `{did}` already present");
    }
    let url = Url::from(id).with_namespace(pubkey);
    add_new_remote(repository, &alias, &url)?;

    term::println("Done", "Remote added with success");
    Ok(())
}

/// from a node pubkey try to get the alias of the node
fn lookup_for_alias(profile: &Profile, pubkey: &PublicKey) -> anyhow::Result<Option<String>> {
    let path = profile.home.node().join(TRACKING_DB_FILE);
    let storage = Config::reader(path)?;
    let Some(node) = storage.node_policy(pubkey)? else {
        return Ok(None);
    };
    Ok(node.alias)
}

fn lookup_for_remote(repository: &git::Repository, alias: &str) -> anyhow::Result<bool> {
    let found = git::rad_has_remote(repository, alias)?;
    Ok(found)
}

fn add_new_remote(repository: &git::Repository, alias: &str, url: &Url) -> anyhow::Result<()> {
    let remote = repository.remote(alias, &url.to_string())?;
    println!("added {:?} with url {:?}", remote.name(), remote.url());
    Ok(())
}
