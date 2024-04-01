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
}
