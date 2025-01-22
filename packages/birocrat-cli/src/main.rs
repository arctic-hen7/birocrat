use std::{fs, io::Read};

use crate::cli::Cli;
use birocrat::{Answer, Form, FormPoll, Question};
use clap::Parser;
use error::Error;
use fmterr::fmterr;
use mlua::Lua;
use serde_json::Value;

mod cli;
mod error;
mod utils;

fn main() {
    match core() {
        Ok(()) => {}
        Err(err) => {
            eprintln!("{}", fmterr(&err));
            std::process::exit(1);
        }
    }
}

fn core() -> Result<(), Error> {
    let args = Cli::parse();
    // We'll take the script from stdin if the user gave `-`, else treat it as a path
    let script = match args.script.as_str() {
        "-" => {
            let mut buffer = String::new();
            std::io::stdin()
                .read_to_string(&mut buffer)
                .map_err(|err| Error::ReadScriptFromStdinFailed { source: err })?;
            buffer
        }
        _ => std::fs::read_to_string(&args.script)
            .map_err(|err| Error::ReadScriptFailed { source: err })?,
    };
    let vm = Lua::new();

    // Parse the parameters (we either have a vec of pairs or a JSON file)
    let params: serde_json::Value = match (args.params.params, args.params.json_params) {
        (Some(params), None) => Value::Object(
            params
                .into_iter()
                .map(|p| p.splitn(2, '=').map(|s| s.to_string()).collect())
                .map(|mut parts: Vec<String>| {
                    (
                        parts.remove(0),
                        if parts.is_empty() {
                            String::new()
                        } else {
                            parts.remove(0)
                        },
                    )
                })
                .map(|(k, v)| (k, Value::String(v)))
                .collect::<serde_json::Map<_, _>>(),
        ),
        (None, Some(json_params)) => {
            let json_params =
                fs::read_to_string(&json_params).map_err(|err| Error::ReadJsonParamsFailed {
                    source: err,
                    target: json_params,
                })?;
            serde_json::from_str(&json_params).map_err(|err| Error::ParseJsonParamsFailed {
                source: err,
                target: json_params,
            })?
        }
        (None, None) => Value::Object(serde_json::Map::new()),
        _ => unreachable!(),
    };

    let mut form = Form::new(&script, params, &vm)?;

    // Format the first question inside a `FormPoll` for consistency of handling logic
    let mut poll = FormPoll::Question {
        question: form.first_question(),
        answer: None,
    };
    // This will be immediately incremented, as we know the first poll is a question. Generally, it
    // will only be incremented if we move on to another question, which allows us to re-ask
    // questions comfortably otherwise.
    let mut question_idx: isize = -1;
    let mut reasking = false;
    loop {
        match poll {
            // NOTE: No answer suggestions in this implementation because we can't go back to
            // previous questions (and reasks from errors won't have cached answers, because those
            // answers failed).
            FormPoll::Question { question, .. } => {
                if !reasking {
                    question_idx += 1;
                } else {
                    reasking = false;
                }

                match question {
                    Question::Simple { prompt, default } => {
                        let input = utils::read_simple(prompt, default.clone())?;
                        poll =
                            form.progress_with_answer(question_idx as usize, Answer::Text(input))?;
                    }
                    Question::Multiline { prompt, default } => {
                        let input = utils::read_multiple(
                            prompt,
                            &default.as_ref().unwrap_or(&String::new()),
                        )?;
                        poll =
                            form.progress_with_answer(question_idx as usize, Answer::Text(input))?;
                    }
                    Question::Select {
                        prompt,
                        // TODO: Add support for default option
                        default: _,
                        options,
                        multiple,
                    } => {
                        let selection = if *multiple {
                            utils::select_multiple(prompt, options)?
                        } else {
                            vec![utils::select_one(prompt, options)?]
                        };
                        let selection = selection.into_iter().map(|s| s.to_string()).collect();

                        poll = form.progress_with_answer(
                            question_idx as usize,
                            Answer::Options(selection),
                        )?;
                    }
                }
            }
            FormPoll::Error(err) => {
                // We have an error in the question with index `question_idx`, so we should display
                // this error message and then return to it
                // TODO: Better printing
                eprintln!("Error: {}", err);

                // We know an error just occurred, so the form still has the old question as the
                // next one to ask
                let (question, answer) = form.next_question().unwrap();
                poll = FormPoll::Question { question, answer };
                reasking = true;
            }
            FormPoll::Done => break,
        }
    }

    // The above loop can only finish on `FormPoll::Done`, so this is guaranteed to work
    let output = form.into_done().unwrap();
    // This is already a `Value`, so serializing it can't fail
    let output_str = serde_json::to_string(&output).unwrap();

    if let Some(output) = args.output {
        fs::write(&output, output_str).map_err(|err| Error::WriteOutputFailed {
            source: err,
            target: output.clone(),
        })?;
        eprintln!("Form output written to {output:?}.")
    } else {
        println!("{output_str}");
    }

    Ok(())
}
