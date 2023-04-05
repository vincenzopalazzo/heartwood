use crate::git;
use crate::terminal as term;

use super::add::lookup_for_remote;

pub fn run(repository: &git::Repository, alias: &str) -> anyhow::Result<()> {
    if !lookup_for_remote(repository, alias)? {
        anyhow::bail!("remote with alias {alias} not found!");
    }
    remote_remote(repository, alias)?;
    term::println("🗑️", term::format::italic(format!("Remote {alias} removed")));
    Ok(())
}

fn remote_remote(repository: &git::Repository, alias: &str) -> anyhow::Result<()> {
    repository.remote_delete(alias)?;
    Ok(())
}
