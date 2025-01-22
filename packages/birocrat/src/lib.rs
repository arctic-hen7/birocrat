pub mod error;

use crate::error::Error;
use mlua::{Function, Lua, LuaSerdeExt, Table, Value as LuaValue};
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;

/// A form created and operated by Birocrat. This follows the engine pattern, whereby this may be
/// used to "drive" an interface of any type.
#[derive(Debug)]
pub struct Form<'l> {
    /// Answers to questions that have been presented at some stage. These are useless unless the
    /// user goes back to change their answer to a previous question, in which case all later
    /// question/answer states will be clobbered. As all questions have unique IDs, if the same
    /// question is later asked, we can put up the same answer to the refiling program for
    /// convenience, without having to manage multiple conflicting states of what the script might
    /// have looked like in the past before the clobbering.
    cached_answers: HashMap<String, Answer>,
    /// The Lua virtual machine which stores the script driving this form. This is held by
    /// reference and must be provided externally.
    lua_vm: &'l Lua,
    /// The main function in the Lua script that drives the form creation and operation.
    driver_function: Function<'l>,
    /// The state of the script at every stage, along with the question is was asking and the
    /// internal ID of that question. This allows us to return to a previous state of the script
    /// to, say, submit a different answer to a question previously asked.
    ///
    /// The indices in this array are used to index questions in the order which they were asked,
    /// while the `String`s stored in each element are script-provided unique question identifiers.
    /// These should not be confused!
    ///
    /// Note that driver script states are stored as serialized values because otherwise Lua will
    /// be a little too efficient and override the values from under our noses when we call the
    /// driver script again! (I.e. they will all point to the same value in the VM.)
    script_states: Vec<(String, Question, Value)>,
    /// The state of the script in the next case. For all the states in `script_states`, there are
    /// corresponding answers in `cached_answers`, while this state is the question which has not
    /// yet been answered. Alternately, it might be a completion state. By populating this for the
    /// next question whenever we're given the answer to another question, we can recreate the
    /// state list when a previous answer is changed and also determine if an error would occur and
    /// propagate that immediately (in which case the old `next_state` would be kept).
    next_state: (ScriptState, Value),
    /// A series of parameters to be passed to the form script. This allows handling everything
    /// from user identifiers to default values. It is *not* modifiable.
    ///
    /// These are stored as a reference to a serialized object in the Lua VM.
    parameters: LuaValue<'l>,
}
impl<'l> Form<'l> {
    /// Creates a new form from the given Lua script. All this does is loads the script.
    pub fn new<P: Serialize>(script: &str, parameters: P, lua_vm: &'l Lua) -> Result<Self, Error> {
        // Register the parameters in the Lua VM
        let parameters = lua_vm
            .to_value(&parameters)
            .map_err(|err| Error::SerializeFormParamsFailed { source: err })?;

        Self::new_with_lua_params(script, parameters, lua_vm)
    }
    /// Same as [`Self::new`], but this takes parameters allocated within the Lua VM. In some
    /// cases, this can be more flexible if serialization can be skipped, or if a heterogeneous
    /// collection is desired. Most users should use [`Self::new`] though, which takes a regular
    /// [`HashMap`].
    pub fn new_with_lua_params(
        script: &str,
        parameters: LuaValue<'l>,
        lua_vm: &'l Lua,
    ) -> Result<Self, Error> {
        lua_vm
            .load(script)
            .exec()
            .map_err(|err| Error::ScriptLoadFailed { source: err })?;
        let driver_function: Function = lua_vm
            .globals()
            .get("Main")
            .map_err(|err| Error::NoMainFunction { source: err })?;

        // Get the first state (manually, because we don't have a `self` yet and because we need to
        // pass `nil` values, which should otherwise be impossible)
        let first_state = Self::call_driver_fn(lua_vm, &driver_function, parameters.clone(), None)?
            .map_err(|err| Error::FirstPollFailed {
                script_err: err.to_string(),
            })?;

        if let ScriptState::Asking { .. } = first_state.0 {
            Ok(Self {
                cached_answers: HashMap::new(),
                lua_vm,
                driver_function,
                script_states: Vec::new(),
                next_state: first_state,
                parameters,
            })
        } else {
            // This isn't a form...
            Err(Error::FirstPollDone)
        }
    }
    /// Gets the first question in the form. This should be called directly after [`Self::new`].
    ///
    /// # Panics
    ///
    /// This will panic if it's called when any other questions have been asked or any answers
    /// provided.
    pub fn first_question(&self) -> &Question {
        if !self.script_states.is_empty() || !self.cached_answers.is_empty() {
            panic!("attempted to get first question when form has already been progressed")
        }

        match &self.next_state.0 {
            ScriptState::Asking { question, .. } => question,
            _ => unreachable!(),
        }
    }

    /// Gets the next question in the form. This is typically used to re-ask the last question
    /// after an error occurs. This will also return a cached answer for this question, if one
    /// exists.
    ///
    /// If there is no next question (i.e. the form is done), this will return `None`.
    pub fn next_question(&self) -> Option<(&Question, Option<&Answer>)> {
        match &self.next_state.0 {
            ScriptState::Asking { question, id } => {
                let answer = self.cached_answers.get(id);
                Some((question, answer))
            }
            _ => None,
        }
    }

    /// Gets the question at the given index. This will return a cached answer as well if the user
    /// has answered this question before. This should be used exclusively for getting past
    /// questions for whatever reason, and providing an index greater than the number of questions
    /// that has been asked will simply return `None`.
    ///
    /// This will never poll the driver script.
    // NOTE: The `idx` here is completely different from the internal question IDs!
    pub fn get_question(&mut self, idx: usize) -> Option<(&Question, Option<&Answer>)> {
        let (id, question, _inner) = self.script_states.get(idx)?;
        // See if there's a cached answer for this question (by its ID)
        let answer = self.cached_answers.get(id);
        Some((question, answer))
    }
    /// Progresses the form by providing an answer for the question with the given index. If this
    /// is the latest question, which has not yet been answered, this will poll the Lua script for
    /// the next question. However, if this provides an answer to a previous question (different
    /// or not!), all subsequent questions the script has generated will be removed from the
    /// internal state (e.g. if you have 10 questions, re-answer index 5, and then request index 7,
    /// you'll get `None`), and the script will be polled for the new next question after the one
    /// answered with this function. This may be the same question (in which case the answer will
    /// be cached), but it may be completely different!
    ///
    /// If the script returns an error (i.e. [`FormPoll::Error`]), no changes will be made to the
    /// internal state of the form (i.e. no clobbering, no answer caching).
    ///
    /// Attempting to answer an out-of-range ID when the form has already been completed will
    /// short-circuit to return the script's completed object.
    ///
    /// This will return a hard `Err(_)` if the answer is of an incorrect type relative to the
    /// question (e.g. multiple options when only one was allowed, options when text was required).
    pub fn progress_with_answer(
        &mut self,
        question_idx: usize,
        answer: Answer,
    ) -> Result<FormPoll<'_>, Error> {
        // Get the script-internal state at whatever point in the question history we're at
        let (question_id, question, inner_state, should_clobber) = if let Some((
            question_id,
            question,
            inner_state,
        )) =
            self.script_states.get(question_idx)
        {
            (question_id, question, inner_state, true)
        } else {
            match &self.next_state {
                // There's a question, we can use its details
                (ScriptState::Asking { id, question }, inner_state) => {
                    (id, question, inner_state, false)
                }
                // If we're already done, short-circuit
                (ScriptState::Done(_), _) => return Ok(FormPoll::Done),
            }
        };

        // Check the answer
        match question {
            Question::Simple { .. } | Question::Multiline { .. } => {
                if !matches!(answer, Answer::Text(_)) {
                    return Err(Error::InvalidAnswerType {
                        expected: "text for simple/multiline question",
                    });
                }
            }
            Question::Select {
                options, multiple, ..
            } => {
                if let Answer::Options(ref selected) = answer {
                    if !*multiple && selected.len() > 1 {
                        return Err(Error::InvalidAnswerType {
                            expected: "single option for non-multiple select question",
                        });
                    }
                    if !selected.iter().all(|s| options.contains(s)) {
                        return Err(Error::InvalidAnswerType {
                            expected: "all options to be valid",
                        });
                    }
                } else {
                    return Err(Error::InvalidAnswerType {
                        expected: "options for select question",
                    });
                }
            }
        }

        // Poll the driver script for a new state (if we get an error from this, we won't clobber)
        let next_state = self.get_script_state(inner_state, &answer)?;
        match next_state {
            Ok((new_state, new_inner_state)) => {
                // This answer worked, cache it
                self.cached_answers.insert(question_id.clone(), answer);

                if should_clobber {
                    // We're changing an answer, so we should get rid of additional questions (they
                    // might have changed). Keep the question we're answering though (`.truncate()`
                    // works by length).
                    self.script_states.truncate(question_idx + 1);
                    // We can also clobber `next_state`
                    self.next_state = (new_state, new_inner_state);
                } else {
                    // We've answered the question in `next_state` (which we confirmed above is a
                    // question), put it into `script_states`
                    let old_next_state =
                        std::mem::replace(&mut self.next_state, (new_state, new_inner_state));
                    match old_next_state {
                        (ScriptState::Asking { id, question }, old_inner_state) => {
                            self.script_states.push((id, question, old_inner_state))
                        }
                        _ => unreachable!(),
                    };
                }

                // Regardless of the above, we have the right thing in `next_state` now
                match &self.next_state.0 {
                    ScriptState::Asking { question, id } => Ok(FormPoll::Question {
                        question,
                        answer: self.cached_answers.get(id),
                    }),
                    ScriptState::Done(_) => Ok(FormPoll::Done),
                }
            }
            // We have an error from the script, which indicates this answer is invalid. We won't
            // clobber subsequent states if this was an old question or change anything else at all
            // about the form, we'll let the user decide what to do.
            Err(script_err) => Ok(FormPoll::Error(script_err)),
        }
    }
    /// If the form has been completed, returns the final object the driver script returned,
    /// serialized for convenience as JSON.
    pub fn into_done(self) -> Result<serde_json::Value, Self> {
        match self.next_state {
            (ScriptState::Done(obj), _) => Ok(obj),
            _ => Err(self),
        }
    }

    /// Polls the Lua script with the given state and answer, returning the next state of the
    /// script. This method does not modify the internal `next_state` or any other properties.
    ///
    /// This returns a nested `Result` because the execution may succeed but the script itself may
    /// return a string error message.
    fn get_script_state(
        &self,
        inner_state: &Value,
        answer: &Answer,
    ) -> Result<Result<(ScriptState, Value), String>, Error> {
        Self::call_driver_fn(
            self.lua_vm,
            &self.driver_function,
            // Cheap clone of a Lua reference
            self.parameters.clone(),
            // PERF: Way of avoiding this clone?
            Some((inner_state.clone(), answer)),
        )
    }

    /// Calls the raw driver function with the given optional state and answer (if one is provided,
    /// both must be). This is used internally, and only directly when getting the first state,
    /// when `None` must be provided. For all subsequent calls, [`Self::get_script_state`] should
    /// be used.
    fn call_driver_fn(
        lua_vm: &'l Lua,
        driver_function: &Function<'l>,
        parameters: LuaValue<'l>,
        inner_state_and_answer: Option<(Value, &Answer)>,
    ) -> Result<Result<(ScriptState, Value), String>, Error> {
        // Convert the answer provided into a Lua table, or, if nothing was provided, call with
        // nils
        let (inner_state, answer) = if let Some((inner_state, answer)) = inner_state_and_answer {
            (
                lua_vm.to_value(&inner_state).unwrap(),
                LuaValue::Table(
                    answer
                        .to_lua(lua_vm)
                        .map_err(|err| Error::AllocateAnswerTableFailed { source: err })?,
                ),
            )
        } else {
            (LuaValue::Nil, LuaValue::Nil)
        };

        let ret_table: Table = driver_function
            .call((inner_state, answer, parameters))
            .map_err(|err| Error::RunDriverFailed { source: err })?;
        let state: String = ret_table.get(1).map_err(|_| Error::InvalidResult)?;
        let props: LuaValue = ret_table.get(2).map_err(|_| Error::InvalidResult)?;
        let inner_state: LuaValue = ret_table.get(3).map_err(|_| Error::InvalidResult)?;
        // Serialize the inner state as an intermediate value
        let inner_state = serde_json::to_value(inner_state)
            .map_err(|err| Error::SerializeStateFailed { source: err })?;

        // We get the raw script state as a double-result, one is handled above and the other is
        // for script errors, but if that didn't occur we should implant the internal state too
        let script_state = ScriptState::from_lua(&state, props)?;
        // NOTE: If we have a done state, `inner_state` will be null.
        Ok(script_state.map(|state| (state, inner_state)))
    }
}

/// The possible results when polling the form. This is returned when a question is answered.
#[derive(PartialEq, Eq, Debug)]
pub enum FormPoll<'a> {
    /// There is a new question to ask.
    Question {
        /// The question.
        question: &'a Question,
        /// Any answer the user previously provided for this question.
        answer: Option<&'a Answer>,
    },
    /// There was an error from the script. This is probably to do with processing the given answer
    /// to the question before the one being requested now, but it could also be to do with
    /// generating the next question.
    Error(String),
    /// The form is complete, and an object is available to be processed. [`Form::into_done`]
    /// should be used to extract the return object from the driver script.
    Done,
}

/// The state of the Lua script, which we will cache at every stage. Providing the state and the
/// answer to the next question will progress the state, and storing it at every point allows going
/// back and changing the answer to any question.
///
/// This should be stored in each case along with an arbitrary [`Value`] from the script, which
/// constitutes its internal state. This only represents the state we observe.
#[derive(Debug)]
enum ScriptState {
    /// The script is in a valid state, and wishes to ask the given question.
    Asking {
        /// The unique ID of the question. This *must not* be repeated for a different question, or
        /// an incorrect previously cached response will be suggested.
        id: String,
        /// The question to ask.
        question: Question,
    },
    /// All questions have been asked and answered, and the script has returned an object
    /// created from them. This object is serialized as JSON for simplicity.
    Done(serde_json::Value),
}
impl ScriptState {
    /// Creates an internal representation of the state of the script from the given Lua
    /// components. The first is a string indicator of the state variant (i.e. `question`, `error`,
    /// or `done`), and the second a series of properties for that variant.
    ///
    /// If the script returned an error, this will return `Ok(Err(err))`.
    fn from_lua(state: &str, props: LuaValue) -> Result<Result<Self, String>, Error> {
        match state {
            "question" => {
                // We have a question to ask, which will be provided as an ID, a question type, a
                // question body, and some optional parameters, all in a table
                let question_table = props.as_table().ok_or(Error::NonTableQuestion)?;
                let id: String = question_table
                    .get("id")
                    .map_err(|err| Error::NoIdInQuestionData { source: err })?;
                let question_type: String = question_table
                    .get("type")
                    .map_err(|err| Error::NoTypeInQuestionData { source: err })?;
                let question_body: String = question_table
                    .get("text")
                    .map_err(|err| Error::NoBodyInQuestionData { source: err })?;
                let suggested_answer: Option<String> =
                    question_table.get("default").unwrap_or(None);

                // The remaining options we extract are type-dependent
                let question = match question_type.as_str() {
                    "simple" => Question::Simple {
                        prompt: question_body,
                        default: suggested_answer,
                    },
                    "multiline" => Question::Multiline {
                        prompt: question_body,
                        default: suggested_answer,
                    },
                    "select" => {
                        // If `multiple` isn't present, we'll default to `false`, reasonably. That
                        // means we can't parse it when we get it though
                        let multiple = question_table
                            .get("multiple")
                            .unwrap_or(LuaValue::Boolean(false));
                        let multiple = if multiple.is_nil() {
                            false
                        } else {
                            multiple
                                .as_boolean()
                                .ok_or(Error::InvalidMultipleProperty)?
                        };

                        let options: Vec<String> = question_table
                            .get("options")
                            .map_err(|err| Error::NoOptionsInQuestionData { source: err })?;

                        // Make sure any default is one of the options
                        if let Some(default) = &suggested_answer {
                            if !options.contains(&default) {
                                return Err(Error::DefaultNotInOptions {
                                    default: default.clone(),
                                })?;
                            }
                        }

                        Question::Select {
                            prompt: question_body,
                            default: suggested_answer,
                            options,
                            multiple,
                        }
                    }
                    _ => {
                        return Err(Error::InvalidQuestionType {
                            ty: question_type.to_string(),
                        })
                    }
                };
                Ok(Ok(ScriptState::Asking { question, id }))
            }
            "error" => {
                // We have a string error message
                let error_msg = props.as_str().ok_or(Error::NonStringErrorMessage)?;
                Ok(Err(error_msg.to_string()))
            }
            "done" => {
                // We have the final result, parse it into a `serde_json` object and return
                let result = serde_json::to_value(&props)
                    .map_err(|err| Error::SerializeAnswersFailed { source: err })?;
                Ok(Ok(ScriptState::Done(result)))
            }
            _ => Err(Error::InvalidState {
                value: state.to_string(),
            }),
        }
    }
}

/// The different types of questions that can be asked. These are fairly generic, as Kylie knows
/// nothing about the contents of boxes. This allows significant flexibility, and delegates
/// complexity to box handlers.
#[derive(Debug, PartialEq, Eq)]
pub enum Question {
    /// A simple question that requires a single-line answer. This would correspond in HTML to a
    /// single `<input>`.
    Simple {
        /// The prompt for the question.
        prompt: String,
        /// A default suggested answer.
        default: Option<String>,
    },
    /// A simple question that requires a multiline answer. This would correspond in HTML to a
    /// `<textarea>`.
    Multiline {
        /// The prompt for the question.
        prompt: String,
        /// A default suggested answer.
        default: Option<String>,
    },
    /// A question where the user can select their answer from a list.
    Select {
        /// The question being asked.
        prompt: String,
        /// A default suggested answer. This is guaranteed to be one of the options.
        default: Option<String>,
        /// A list of options the user can take.
        options: Vec<String>,
        /// Whether or not the user can select multiple options. Further validation like ensuring
        /// the user has selected fewer than *n* answers is left to the box.
        multiple: bool,
    },
}

/// The user's answer to a question. This contains no information about the question it answers.
#[derive(Debug, PartialEq, Eq)]
pub enum Answer {
    /// A textual answer. This will come to either [`Question::Simple`] or [`Question::Multiline`].
    Text(String),
    /// An answer in terms of a series of given options. These are *guaranteed* to be valid with
    /// respect to the options offered in the relevant question, and will come as a response to
    /// [`Question::Select`].
    Options(Vec<String>),
}
impl Answer {
    /// Converts this answer into a Lua-friendly representation. This will produce a Lua table of
    /// the form `{ type = "text", text = "..." }` or `{ type = "options", selected = { ... } }`,
    /// depending on the type of question this is in answer to.
    ///
    /// # Errors
    ///
    /// This involves allocating a [`Table`] in the Lua VM, which may fail. Additionally, setting
    /// values in the table may fail.
    fn to_lua<'l>(&self, lua_vm: &'l Lua) -> Result<Table<'l>, mlua::Error> {
        let answer_table = lua_vm.create_table()?;

        match &self {
            Answer::Text(text) => {
                answer_table.set("type", "text")?;
                answer_table.set("text", text.as_str())?;
            }
            Answer::Options(options) => {
                answer_table.set("type", "options")?;
                answer_table.set("selected", options.clone())?;
            }
        };

        Ok(answer_table)
    }
}
