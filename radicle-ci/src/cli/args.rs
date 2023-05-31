/// CLI Args parser.
use radicle_term as term;

struct Help {
    name: &'static str,
    description: &'static str,
    version: &'static str,
    usage: &'static str,
}

const HELP: Help = Help {
    name: "radicle-ci",
    description: "A minimal and portable CI written in Rust",
    version: env!("CARGO_PKG_VERSION"),
    usage: r#"
Usage

    radicle-ci [<option> ...]

Options

    -w | --workdir    Override the default path of the config field
    -e | --exec       Specify the execution path.
    -h | --help       Print help
"#,
};

#[derive(Debug)]
pub struct RadicleCIArgs {
    pub workdir: String,
    pub exec_path: String,
}

impl RadicleCIArgs {
    pub fn parse() -> Result<Self, lexopt::Error> {
        use lexopt::prelude::*;

        let mut workdir: Option<String> = None;
        let mut exec_path: Option<String> = None;

        let mut parser = lexopt::Parser::from_env();
        while let Some(arg) = parser.next()? {
            match arg {
                Short('w') | Long("workdir") => {
                    let val: String = parser.value()?.parse()?;
                    workdir = Some(val);
                }
                Short('e') | Long("exec") => {
                    let val: String = parser.value()?.parse()?;
                    exec_path = Some(val);
                }
                Long("help") => {
                    let _ = Self::print_help();
                    std::process::exit(0);
                }
                _ => return Err(arg.unexpected()),
            }
        }

        Ok(Self {
            workdir: workdir.expect("Workdir must be specified"),
            exec_path: exec_path.expect("Execution Path must be specified"),
        })
    }

    // Print helps
    pub fn print_help() {
        println!(
            "{}",
            term::format::secondary("Common `radicle-ci` to manage the radicle-ci")
        );
        println!(
            "\n{} {}",
            term::format::bold("Usage:"),
            term::format::dim("lampod-cli <command> [--help]")
        );
        println!();

        println!(
            "\t{} {} {}",
            term::format::bold(format!("{:-12}", HELP.name)),
            term::format::dim(HELP.description),
            term::format::dim(HELP.version),
        );
        println!("{}", term::format::bold(HELP.usage));
    }
}
