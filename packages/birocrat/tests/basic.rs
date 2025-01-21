use std::collections::HashMap;

use birocrat::*;
use mlua::Lua;
use serde_json::json;

static BASIC_SCRIPT: &str = include_str!("basic.lua");

#[test]
fn should_work() {
    let mut params = HashMap::new();
    params.insert("id", 37);
    let vm = Lua::new();
    let mut form = Form::new(BASIC_SCRIPT, params, &vm).unwrap();

    let question = form.first_question();
    assert_eq!(
        question,
        &Question::Simple {
            prompt: "What is your name, user 37?".to_string(),
            default: None,
        }
    );
    let poll = form
        .progress_with_answer(0, Answer::Text("Alice".to_string()))
        .unwrap();
    assert_eq!(
        poll,
        FormPoll::Question {
            question: &Question::Simple {
                prompt: "How old are you, Alice?".to_string(),
                default: Some("30".to_string()),
            },
            answer: None
        }
    );
    // Provide an incorrect answer
    let poll = form
        .progress_with_answer(1, Answer::Text("twenty-five".to_string()))
        .unwrap();
    assert_eq!(
        poll,
        FormPoll::Error("Please enter a valid number.".to_string())
    );
    // Provide a correct answer
    let poll = form
        .progress_with_answer(1, Answer::Text("25".to_string()))
        .unwrap();
    assert_eq!(
        poll,
        FormPoll::Question {
            question: &Question::Select {
                prompt: "What is your favourite type of cuisine?".to_string(),
                default: None,
                options: vec!["Indian", "Korean", "Japanese", "Chinese", "Italian"]
                    .into_iter()
                    .map(|s| s.to_string())
                    .collect(),
                multiple: false
            },
            answer: None
        }
    );
    // Get the previous question and answer
    assert_eq!(
        form.get_question(1),
        Some((
            &Question::Simple {
                prompt: "How old are you, Alice?".to_string(),
                default: Some("30".to_string()),
            },
            Some(&Answer::Text("25".to_string()))
        ))
    );

    // Invalid options
    assert!(form
        .progress_with_answer(2, Answer::Text("Test".to_string()))
        .is_err());
    assert!(form
        .progress_with_answer(
            2,
            Answer::Options(vec!["Indian".to_string(), "Korean".to_string()])
        )
        .is_err());
    assert!(form
        .progress_with_answer(2, Answer::Options(vec!["American".to_string()]))
        .is_err());

    // If we answer with `Italian`, we should be done
    let poll = form
        .progress_with_answer(2, Answer::Options(vec!["Italian".to_string()]))
        .unwrap();
    assert_eq!(poll, FormPoll::Done);

    // But we can go back and answer with something else to get another question
    let poll = form
        .progress_with_answer(2, Answer::Options(vec!["Indian".to_string()]))
        .unwrap();
    assert_eq!(
        poll,
        FormPoll::Question {
            question: &Question::Select {
                prompt: "What levels of spice can you tolerate?".to_string(),
                default: None,
                options: vec!["Mild", "Medium", "Hot", "Very Hot", "Extreme Hot"]
                    .into_iter()
                    .map(|s| s.to_string())
                    .collect(),
                multiple: true,
            },
            answer: None,
        }
    );

    // Now that there's another question, we can't complete the form
    let res = form.into_done();
    assert!(res.is_err());
    form = res.unwrap_err();

    // Answering this with one or more options will work, but not with invalid options
    assert!(form
        .progress_with_answer(
            3,
            Answer::Options(vec!["Mild".to_string(), "Not that hot".to_string()])
        )
        .is_err());
    assert_eq!(
        form.progress_with_answer(3, Answer::Options(vec!["Mild".to_string()]))
            .unwrap(),
        FormPoll::Done
    );
    assert_eq!(
        form.progress_with_answer(
            3,
            Answer::Options(vec!["Mild".to_string(), "Medium".to_string()])
        )
        .unwrap(),
        FormPoll::Done
    );

    // And now we can get the form's final details
    let res = form.into_done().unwrap();
    assert_eq!(
        res,
        json!({
            "name": "Alice",
            "age": 25,
            "favourite_cuisine": "Indian",
            "spice_levels": ["Mild", "Medium"]
        })
    );
}
