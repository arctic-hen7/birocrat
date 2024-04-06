use std::{collections::HashMap, io::Read};

use crate::cli::Args;
use birocrat::{Answer, Form, FormPoll, Question};
use clap::Parser;
use error::Error;
use fmterr::fmterr;
use mlua::Lua;

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
    let args = Args::parse();
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

    // Parse the parameters
    let params: HashMap<String, String> = args
        .params
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
        .collect();

    let vm = Lua::new();
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
                    Question::Simple(prompt) => {
                        let input = utils::read_simple(prompt)?;
                        poll =
                            form.progress_with_answer(question_idx as usize, Answer::Text(input))?;
                    }
                    Question::Multiline(prompt) => {
                        let input = utils::read_multiple(prompt, "")?;
                        poll =
                            form.progress_with_answer(question_idx as usize, Answer::Text(input))?;
                    }
                    Question::Select {
                        prompt,
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

    // Stdout will be the output of the string for sending it elsewhere
    println!("{output_str}");

    Ok(())
}
