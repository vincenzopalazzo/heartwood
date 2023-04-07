use radicle::node::tracking::store::Config;
use radicle::{
    git::Url,
    node::TRACKING_DB_FILE,
    prelude::{Did, Id},
    Profile,
};
use radicle_crypto::PublicKey;

use crate::{git, terminal as term};

/// core command to run the `rad remote add ...` subcommand.
pub fn run(
    repository: &git::Repository,
    profile: &Profile,
    did: &Did,
    alias: &Option<String>,
    id: Id,
) -> anyhow::Result<()> {
    let pubkey = did.as_key();
    let Some(alias) = lookup_for_alias(profile, pubkey, alias)? else {
            anyhow::bail!("an alias need to be specified");
    };

    if git::rad_has_remote(repository, &alias)? {
        anyhow::bail!("remote with did `{did}` already present");
    }
    let url = Url::from(id).with_namespace(*pubkey);
    let (name, url) = add_new_remote(repository, &alias, &url)?;

    term::println(
        term::format::badge_primary("🚀"),
        term::format::italic(format!("Remote {name} added with {url}")),
    );
    Ok(())
}

/// from a node pubkey try to get the alias of the node
fn lookup_for_alias(
    profile: &Profile,
    pubkey: &PublicKey,
    alias: &Option<String>,
) -> anyhow::Result<Option<String>> {
    if let Some(alias) = alias {
        return Ok(Some(alias.to_owned()));
    }
    let path = profile.home.node().join(TRACKING_DB_FILE);
    let storage = Config::reader(path)?;
    let Some(node) = storage.node_policy(pubkey)? else {
        return Ok(None);
    };
    Ok(node.alias)
}

fn add_new_remote(
    repository: &git::Repository,
    alias: &str,
    url: &Url,
) -> anyhow::Result<(String, String)> {
    let remote = repository.remote(alias, &url.to_string())?;
    Ok((
        remote.name().unwrap_or_default().to_owned(),
        remote.url().unwrap().to_owned(),
    ))
}
