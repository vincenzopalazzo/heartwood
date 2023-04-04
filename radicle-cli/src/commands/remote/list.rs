use super::Options;
use crate::git;

#[inline]
fn format_direction(d: &git::Direction) -> &str {
    match d {
        git::Direction::Fetch => "fetch",
        git::Direction::Push => "push",
    }
}

/// core command to run the `rad remote list` or just `rad remote` subcommand.
pub fn run(repo: &git::Repository, options: &Options) -> anyhow::Result<()> {
    let remotes = git::rad_remotes(repo)?;
    for r in remotes {
        if options.verbose {
            for spec in r.refspecs() {
                println!(
                    "{}\t{} ({})",
                    r.name().unwrap(),
                    r.url().unwrap(),
                    format_direction(&spec.direction()),
                );
            }
        } else {
            println!("{}", r.name().unwrap());
        }
    }
    Ok(())
}
