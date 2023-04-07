use crate::git;
use crate::terminal as term;

pub fn run(repository: &git::Repository, alias: &str) -> anyhow::Result<()> {
    if !git::rad_has_remote(repository, alias)? {
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
