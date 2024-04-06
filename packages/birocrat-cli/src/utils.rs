use crate::error::Error;
use dialoguer::{Editor, Input, MultiSelect, Select};

/// Reads a single-line input from the terminal using `dialoguer`.
pub fn read_simple(prompt: &str, default: Option<String>) -> Result<String, Error> {
    let input = if let Some(default) = default {
        Input::<String>::new().with_prompt(prompt).default(default)
    } else {
        Input::<String>::new().with_prompt(prompt)
    }
    .interact()?;

    Ok(input)
}

/// Reads a multi-line input from the terminal using `dialoguer`.
///
/// This takes a prompt, which will be provided as a comment, along with some starter text for the
/// user to actually edit. This is performed through the system's text editor.
pub fn read_multiple(prompt: &str, starter: &str) -> Result<String, Error> {
    let prompt = prompt.replace("\n", "\n# ");
    let edit_str = format!("#{prompt}\n\n{starter}");

    let input = Editor::new().edit(&edit_str)?;
    // If the user didn't provide any input (i.e. file not saved in editor), return an empty string
    let input = input.unwrap_or_else(|| String::new());

    // Strip off the leading commented lines
    let real_input = input
        .lines()
        .skip_while(|l| l.starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n");
    let real_input = real_input.trim().to_string();

    Ok(real_input)
}

/// Gives the user an option between several values and allows them to select one, returning it.
///
/// This returns `&String` rather than `&str` for compatibility with [`select_multiple`].
pub fn select_one<'o>(prompt: &str, options: &'o Vec<String>) -> Result<&'o String, Error> {
    let selection = Select::new()
        .with_prompt(prompt)
        .items(&options)
        .interact()?;

    Ok(&options[selection])
}

/// Gives the user options between several values, allowing them to select multiple, and returning
/// it.
pub fn select_multiple<'o>(
    prompt: &str,
    options: &'o Vec<String>,
) -> Result<Vec<&'o String>, Error> {
    let selections = MultiSelect::new()
        .with_prompt(prompt)
        .items(&options)
        .interact()?;

    Ok(selections.into_iter().map(|i| &options[i]).collect())
}
