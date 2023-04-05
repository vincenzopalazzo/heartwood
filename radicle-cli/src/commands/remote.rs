//! Remote Command implementation
#[path = "remote/add.rs"]
pub mod add;
#[path = "remote/list.rs"]
pub mod list;
#[path = "remote/rm.rs"]
pub mod rm;

use std::ffi::OsString;
use std::str::FromStr;

use anyhow::anyhow;

use radicle::git::Url;

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
    Rm,
    #[default]
    List,
}

#[derive(Debug)]
pub enum Operation {
    Add { url: Url },
    Rm { alias: String },
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
        let mut url: Option<Url> = None;
        let mut alias: Option<String> = None;
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
                    "r" | "rm" => op = Some(OperationName::Rm),
                    unknown => anyhow::bail!("unknown operation '{}'", unknown),
                },
                Value(val) => {
                    if op == Some(OperationName::Add) && url.is_none() {
                        let val = string(&val);
                        let id = Url::from_str(&val)?;
                        url = Some(id);
                    } else if op == Some(OperationName::Rm) && alias.is_none() {
                        let val = string(&val);
                        alias = Some(val);
                    }
                }
                _ => return Err(anyhow::anyhow!(arg.unexpected())),
            }
        }

        let op = match op.unwrap_or_default() {
            OperationName::Add => Operation::Add {
                url: url.ok_or(anyhow!("url required"))?,
            },
            OperationName::List => Operation::List,
            OperationName::Rm => Operation::Rm {
                alias: alias.ok_or(anyhow!(
                    "alias required, try to lookup for it by running `rad remote`"
                ))?,
            },
        };

        Ok((Options { op, verbose }, vec![]))
    }
}

pub fn run(options: Options, _: impl Context) -> anyhow::Result<()> {
    let (working, _) = radicle::rad::cwd()
        .map_err(|_| anyhow!("this command must be run in the context of a project"))?;

    match options.op {
        Operation::Add { url: ref did } => self::add::run(&working, did)?,
        Operation::Rm { ref alias } => self::rm::run(&working, alias)?,
        Operation::List => self::list::run(&working, &options)?,
    };
    Ok(())
}
