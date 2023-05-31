//! CI Workflow implementation!
//!
//! Welcome in the workflow implementation of the radicle CI
//! this is the entry point of any radicle CI configuration
//! and we can build the CI in any way that we want.
//!
//! With a declarative way (in any language) if you are a crazy person,
//! or with a simple yaml configuration like the Github Action if you
//! are a person that want automate all the boring task that you need
//! to do manually as mantainer.
//!
//! So, the workflow concept in the radicle CI is a way to understand
//! from a work directory what kind of pipeline (we discuss the pipeline later)
//! we need to run.
//!
//!
//! Author: Vincenzo Palazzo <vincenzopalazzo@member.fsf.org>
use std::vec::Vec;

use tokio::fs::File;
use tokio::io::AsyncReadExt;

use crate::ci::pipeline::Pipeline;

#[derive(Debug)]
pub struct Error {}

#[derive(Debug)]
pub struct Workflow {
    pub working_dir: String,
    pub pipelines: Vec<Pipeline>,
}

impl Workflow {
    pub async fn new(working_dir: String, exec_path: String) -> std::io::Result<Self> {
        let pipelines = Self::load_pipelines(working_dir.clone(), exec_path).await?;
        Ok(Self {
            working_dir,
            pipelines,
        })
    }

    pub async fn run(&mut self) -> std::io::Result<()> {
        for pipeline in &mut self.pipelines {
            pipeline.run().await?;
        }
        Ok(())
    }

    pub async fn load_pipelines(
        workdir: String,
        exec_path: String,
    ) -> std::io::Result<Vec<Pipeline>> {
        let mut pipelines = vec![];
        // FIXME: load just the file, but in the future we should
        // load all the file inside the .radicle-ci/
        let mut pipeline_file = File::open(workdir).await?;
        let mut conf_str = String::new();
        pipeline_file.read_to_string(&mut conf_str).await?;
        let mut pipeline = serde_yaml::from_str::<Pipeline>(&conf_str).unwrap();
        pipeline.exec_path = exec_path;
        pipelines.push(pipeline);
        Ok(pipelines)
    }
}
