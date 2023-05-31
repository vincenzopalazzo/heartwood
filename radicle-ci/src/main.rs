//! Radicle CI implementation.
//!
//! This is a minimal CI implemented in Rust
//! with a flexible design.
//!
//! Why hasn't anyone built it? Well, I don't know, but this
//! is quite straightforward. Let's discuss the architecture
//! along with the code.
//!
//! Author: Vincenzo Palazzo <vincenzopalazzo@member.fsf.org>
use std::io;

use radicle_term as term;

mod ci;
mod cli;

#[tokio::main]
async fn main() -> io::Result<()> {
    let args = cli::RadicleCIArgs::parse();
    if args.is_err() {
        term::error(format!("{:?}", args));
    }
    let args = args.unwrap();

    let mut workflow = ci::Workflow::new(args.workdir, args.exec_path).await?;
    if let Err(err) = workflow.run().await {
        term::error(format!("{:?}", err));
    } else {
        term::success!("Workflow completed with success");
    }

    Ok(())
}
