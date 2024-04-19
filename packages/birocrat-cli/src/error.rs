use std::path::PathBuf;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error(transparent)]
    DialogueError(#[from] dialoguer::Error),
    #[error(transparent)]
    FormError(#[from] birocrat::error::Error),
    #[error("failed to read driver script for form")]
    ReadScriptFailed {
        #[source]
        source: std::io::Error,
    },
    #[error("failed to read script from stdin (did you mean to provide a path to the cli?)")]
    ReadScriptFromStdinFailed {
        #[source]
        source: std::io::Error,
    },
    #[error("failed to write form output to '{target:?}'")]
    WriteOutputFailed {
        #[source]
        source: std::io::Error,
        target: PathBuf,
    },
}
