use radicle_term::{Element, Table};

use super::Options;
use crate::git;
use crate::terminal as term;

#[inline]
fn format_direction(d: &git::Direction) -> String {
    match d {
        git::Direction::Fetch => "fetch".to_owned(),
        git::Direction::Push => "push".to_owned(),
    }
}

/// core command to run the `rad remote list` or just `rad remote` subcommand.
pub fn run(repo: &git::Repository, _: &Options) -> anyhow::Result<()> {
    let mut table = Table::default();
    let remotes = git::rad_remotes(repo)?;
    for r in remotes {
        for spec in r.refspecs() {
            let dir = spec.direction();
            let url = r.url().unwrap();
            let name = r.name().unwrap();
            table.push([
                term::format::badge_positive(format_direction(&dir)),
                term::format::highlight(name.to_owned()),
                term::format::italic(url.to_owned()),
            ]);
        }
    }
    table.print();
    Ok(())
}
