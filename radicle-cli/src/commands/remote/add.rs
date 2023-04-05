use radicle::git::Url;

use crate::{git, terminal as term};

/// core command to run the `rad remote add ...` subcommand.
pub fn run(repository: &git::Repository, url: &Url) -> anyhow::Result<()> {
    let alias = url.repo.canonical();
    if lookup_for_remote(repository, &alias)? {
        anyhow::bail!("remote with did `{url}` already present");
    }
    add_new_remote(repository, &alias, url)?;

    term::println("Done", "Remote added with success");
    Ok(())
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
