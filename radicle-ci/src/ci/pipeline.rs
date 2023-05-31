//! Pipeline Implementation
//!
//! Welcome to the Pipeline implementation. The pipeline
//! concept is simply a sequence of dependent or independent
//! actions that will run in a Runner (we will describe the runner later).
//!
//! So, the Pipeline is the place where we define the CI code that
//! the user wants to check inside the CI.
//!
//! Author: Vincenzo Palazzo <vincenzopalazzo@member.fsf.org>
use std::collections::HashSet;
use std::fmt::Formatter;
use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// sh macro is the macro that allow to run a
/// script as a sequence of commands.
#[macro_export]
macro_rules! sh {
    ($root: expr, $script:expr, $verbose:expr) => {
        use tokio::process::Command;

        let script = $script.trim();
        let cmds = script.split("\n"); // Check if the script contains `\`
        for cmd in cmds {
            let cmd_tok: Vec<&str> = cmd
                .split(" ")
                .map(|tok| tok.trim())
                .filter(|tok| !tok.is_empty())
                .collect();
            let command = cmd_tok.first().unwrap().to_string();
            let mut cmd = Command::new(command);
            cmd.args(&cmd_tok[1..cmd_tok.len()]);
            cmd.current_dir($root);
            if $verbose {
                let _ = cmd
                    .spawn()
                    .expect("Unable to run the command")
                    .wait()
                    .await?;
            } else {
                let _ = cmd.output().await?;
            }
        }
    };

    ($root:expr, $script:expr) => {
        sh!($root, $script, false)
    };
}

// FIXME: move in a separate file
#[async_trait]
pub trait Runner {
    async fn run(&self, action: &Action) -> std::io::Result<()>;
}

// FIXME: move in a separate file
#[derive(Serialize, Deserialize)]
pub struct Action {
    pub on: HashSet<String>,
    pub run: String,
    #[serde(skip_serializing, skip_deserializing)]
    pub root_path: String,
    pub verbose: bool,
}

#[derive(Serialize, Deserialize)]
pub struct Pipeline {
    #[serde(skip_serializing, skip_deserializing)]
    pub exec_path: String,
    pub image: String,
    /// Pipeline runner that will have
    /// the implementation to run the
    /// Pipeline in the correct way.
    ///
    /// For example, the runner can be a Docker Runner,
    /// a Native Runner, or any other kind.
   #[serde(skip_serializing, skip_deserializing)]
    pub runner: Option<Arc<dyn Runner>>,
    /// DAG of Actions that implement the
    /// kind of action that the user wants to
    /// run inside the runner.
    pub actions: Vec<Action>,
}

impl std::fmt::Debug for Pipeline {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "pipeline")
    }
}

impl Pipeline {
    pub async fn run(&mut self) -> std::io::Result<()> {
        self.runner = Some(Arc::new(NativeRunner::new()));
        for action in &mut self.actions {
            action.root_path = self.exec_path.clone();
            self.runner.clone().unwrap().run(action).await?;
        }
        Ok(())
    }
}

// FIXME: move in a separate file.
pub struct NativeRunner {}

impl NativeRunner {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl Runner for NativeRunner {
    async fn run(&self, action: &Action) -> std::io::Result<()> {
        sh!(action.root_path.clone(), action.run.trim(), action.verbose);
        Ok(())
    }
}
