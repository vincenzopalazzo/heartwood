//! Remote Command implementation
#[path = "remote/add.rs"]
pub mod add;
#[path = "remote/list.rs"]
pub mod list;

use std::ffi::OsString;
use std::str::FromStr;

use anyhow::anyhow;

use radicle::git::Url;
use radicle::prelude::Id;
use radicle::storage::ReadStorage;

use crate::terminal::args::{string, Error};
use crate::terminal::{Args, Context, Help};

pub const HELP: Help = Help {
    name: "remote",
    description: "Manage set of tracked repositories",
    version: env!("CARGO_PKG_VERSION"),
    usage: r#"
Usage
    rad remote
    rad remote list
    rad remote add <name> <id>
Options
        --help                 Print help
"#,
};

#[derive(Debug, Default, PartialEq, Eq)]
pub enum OperationName {
    Add,
    #[default]
    List,
}

#[derive(Debug)]
pub enum Operation {
    Add { url: Url },
    List,
}

#[derive(Debug)]
pub struct Options {
    pub op: Operation,
    pub verbose: bool,
}

impl Args for Options {
    fn from_args(args: Vec<OsString>) -> anyhow::Result<(Self, Vec<OsString>)> {
        use lexopt::prelude::*;

        let mut parser = lexopt::Parser::from_args(args);
        let mut op: Option<OperationName> = None;
        let mut did: Option<Url> = None;
        let mut verbose = false;

        while let Some(arg) = parser.next()? {
            match arg {
                Long("help") => {
                    return Err(Error::Help.into());
                }
                Long("verbose") | Short('v') => {
                    verbose = true;
                }
                Value(val) if op.is_none() => match val.to_string_lossy().as_ref() {
                    "a" | "add" => op = Some(OperationName::Add),
                    "l" | "list" => op = Some(OperationName::List),
                    unknown => anyhow::bail!("unknown operation '{}'", unknown),
                },
                Value(val) => {
                    if op == Some(OperationName::Add) && did.is_none() {
                        let val = string(&val);
                        let id = Url::from_str(&val)?;
                        did = Some(id);
                    }
                }
                _ => return Err(anyhow::anyhow!(arg.unexpected())),
            }
        }

        let op = match op.unwrap_or_default() {
            OperationName::Add => Operation::Add {
                url: did.ok_or(anyhow!("did required, try to run `rad remote add <did>`"))?,
            },
            OperationName::List => Operation::List,
        };

        Ok((Options { op, verbose }, vec![]))
    }
}

pub fn run(options: Options, ctx: impl Context) -> anyhow::Result<()> {
    let (working, id) = radicle::rad::cwd()
        .map_err(|_| anyhow!("this command must be run in the context of a project"))?;
    let profile = ctx.profile()?;
    let repository = profile.storage.repository(id)?;

    match options.op {
        Operation::Add { url: ref did } => self::add::run(&repository, &profile, did)?,
        Operation::List => self::list::run(&working, &options)?,
    };
    Ok(())
}
