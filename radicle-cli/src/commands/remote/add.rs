use radicle::git::Url;
use radicle::storage::git::Repository;
use radicle::Profile;

use crate::terminal as term;

/// core command to run the `rad remote add ...` subcommand.
pub fn run(repository: &Repository, profile: &Profile, id: &Url) -> anyhow::Result<()> {
    if lookup_for_remote(repository, &id.to_string())? {
        anyhow::bail!("remote with did `{id}` already present");
    }
    add_new_remote(repository, &id.to_string())?;

    term::println("Done", "Remote added with success");
    Ok(())
}

fn lookup_for_remote(repository: &Repository, alias: &str) -> anyhow::Result<bool> {
    let git = &repository.backend;
    let remotes = git.remotes()?;
    // FIXME: I can use `find_remote`?
    let it = remotes.iter().find(|it| it.unwrap() == alias);
    Ok(it.is_some())
}

fn add_new_remote(repository: &Repository, alias: &str) -> anyhow::Result<()> {
    let git = &repository.backend;
    git.remote(alias, alias)?;
    Ok(())
}
