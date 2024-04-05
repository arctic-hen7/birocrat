use thiserror::Error;

/// Errors that can occur while operating a form.
#[derive(Error, Debug)]
pub enum Error {
    #[error("failed to load lua script to drive form operation")]
    ScriptLoadFailed {
        #[source]
        source: mlua::Error,
    },
    #[error("could not find main function in driver script")]
    NoMainFunction {
        #[source]
        source: mlua::Error,
    },
    #[error("failed to run driver function")]
    RunDriverFailed {
        #[source]
        source: mlua::Error,
    },

    #[error("received invalid return value from driver script (expected array with status string and data)")]
    InvalidResult,
    #[error("found invalid state from driver function (expected `question`, `error`, or `done`)")]
    InvalidState { value: String },
    #[error("failed to serialize intermediate driver script state")]
    SerializeStateFailed {
        #[source]
        source: serde_json::Error,
    },
    #[error("failed to serialize answers from completed driver script")]
    SerializeAnswersFailed {
        #[source]
        source: serde_json::Error,
    },
    #[error("expected string error message as second value when status from script was 'error'")]
    NonStringErrorMessage,
    #[error("failed to parse question data from driver script as a table")]
    NonTableQuestion,
    #[error("found no, or failed to parse, question id in question data from script")]
    NoIdInQuestionData {
        #[source]
        source: mlua::Error,
    },
    #[error("found no, or failed to parse, question type in question data from script")]
    NoTypeInQuestionData {
        #[source]
        source: mlua::Error,
    },
    #[error("found no, or failed to parse, question body in question data from script")]
    NoBodyInQuestionData {
        #[source]
        source: mlua::Error,
    },
    #[error("received invalid question type from driver script: '{ty}'")]
    InvalidQuestionType { ty: String },
    #[error("found invalid non-boolean value for property `multiple` in select-type question")]
    InvalidMultipleProperty,
    #[error(
        "found no, or failed to parse, answer options in select-type question data from script"
    )]
    NoOptionsInQuestionData {
        #[source]
        source: mlua::Error,
    },
    #[error("first poll of driver script failed with no input: '{script_err}'")]
    FirstPollFailed { script_err: String },
    #[error("first poll of driver script completed form without asking a question")]
    FirstPollDone,
    #[error("failed to allocate space in lua vm for table to hold answer")]
    AllocateAnswerTableFailed {
        #[source]
        source: mlua::Error,
    },
    #[error("invalid answer type for question (expected '{expected}')")]
    InvalidAnswerType { expected: &'static str },
    #[error("failed to serialize form parameters to lua table")]
    SerializeFormParamsFailed {
        #[source]
        source: mlua::Error,
    },
}
