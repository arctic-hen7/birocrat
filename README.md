# Birocrat

Birocrat is a universal engine for running complex forms. What does that mean? Well, imagine a simple form that asks you a set number of questions in a set order. This can be specified by a basic config file. But now imagine a more complex form, where the questions asked depend on the answers given: say, we only ask what levels of spice you can tolerate if you say your favourite cuisine is Indian, not if you say Italian. These kinds of forms can get arbitrarily complex, so Birocrat uses Lua scripts, executed through a blazingly-fast Rust environment, to run them, letting you create highly complex forms and execute them however you like! Additionally, it's built in an *engine-pattern*, so, despite being an application in its own right, the `birocrat` crate is generic over any interface: you get the first question, and then call `.progress_with_answer()` to give an answer and go on to the next question. For quality of life, Birocrat lets you re-answer old questions, and recompute the questions that will be asked thereafter (as they might change), suggesting the same answers as before for any that remain the same.

This repo houses the `birocrat` crate, together with a terminal interface for any Birocrat form under `birocrat-cli`, which takes a Lua file as its only argument.

## Why does this exist?

There are plenty of utilities for building forms of all kinds, but few that allow forms to be arbitrarily complex or implement arbitrary validation logic. On top of that, most are written with one interface in mind: usually the browser, but sometimes the terminal, so Birocrat bridges those disparate needs. It was originally created for my own personal productivity systems, where I have an *inbox* that I capture things into throughout the day. I then want to *refile* those things to other places (a to-do list, list of events, etc.), each of which has its own form, which might be quite complex. The critical thing is validation logic for timestamps actually, which none of the existing systems I found would let me do. Birocrat wasn't too much work, so I made it generic to fit as many use-cases as possible! If you find it handy, give it a star!

## What's in a name?

Lots of forms, so bureaucrat, which takes thought to spell, and you don't want a terminal program to take thought to spell, ergo *Birocrat*, with biro for pen, as in forms!

## Writing a script

Birocrat uses a *driver script* to operate a form, and this script is responsible for, at a fundamental level, **generating the next question given the previous answer**. The script should be stateless, providing its internal state as an output (so it can be restarted from any previous input to allow the user to change their answers), and is operated through a single `Main` function. For now, Birocrat only supports Lua scripts, though in future this may be extended to support other languages like Python and JavaScript for ease of use.

A Lua script for a Birocrat form should consist of a function with the signature `fn Main(state: State | nil, answer: Answer | nil) -> { "question" | "error" | "done", Question | Error | Done, state: State }`, where the following pseudo-types are defined:

- `State`: whatever the internal state of the Lua script is (this will be held by Birocrat and sent back; e.g. the state generated with question 1 will be provided back when question 1 is answered, in order to generate question 2); if this is `nil` the script is being instantiated for the first time
- `Answer`: the answer to the last question (whatever the last question is should be recorded internally in `state`, remember to support going back to old state, `Main` should be a pure function!); if this is `nil`, `state` will also be `nil` and the script is being instantiated for the first time
  - `type`: the type of the answer, which will be either `text` or `options`, depending on the kind of question asked
  - `text`: (only provided if `type = "text"`) the text of the user's answer
  - `selected`: (only provided if `type = "options"`) the options selected by the user; if the question only allowed a single selection, this will be an array with a single element, otherwise there will be as many as the user selected
- `Question`: used if there is another question to ask after the one we've just answered
  - `id`: a unique identifier for this question; typically there will be a finite number of questions the script can ask and the order in which they are asked (if at all) will depend on the users' answers; each question should have its own unique ID used every time it's asked (this allows Birocrat to cache answers to questions, see below)
  - `type`: one of `simple` (single-line text input), `multiline` (multi-line text input), or `select` (selection from given options)
  - `text`: the actual prompt of the question
  - `options`: (only if `type = "select"`) the options from which the user may choose
  - `multiple`: (only if `type = "select"`) whether or not the user can choose multiple options (default: `false`)
- `Error`: a string error message for when something has gone wrong; if this is returned the script will not be progressed again from this state, rather the user will be prompted to re-answer the last question (given the error message from the script to aide them); this is typically used for input validation (e.g. email address checking)
- `Done`: an arbitrary object that can be serialized to JSON; this indicates the form is complete and there are no more questions to ask; the provided object represents the user's responses and can be sent back for processing

As mentioned above, it is critical that `Main` is a *pure* function, meaning that, given the same state and answer, it must always return the same response. For example, storing state in a local variable that is modified each time is a bad idea, as this would make it very hard to revert to a previous state if the user wants to change their answer to an earlier question. You should let Birocrat handle such cases, as it will remember the states your script produces and give you back the right one at the right time to produce the right next question. Any information about where you are in a question tree should be stored in that `state` variable.

## Answer caching

Birocrat automatically caches a user's answers for convenience, primarily for when they change their answers. As a Birocrat form may produce different questions depending on each answer, we have to assume when an answer is changed that all the questions the user subsequently answered are invalid, so we discard them. However, if there would have been no change to some of those questions, this is very inconvenient, so we remember the answers to all the questions they've answered so far so we can suggest them if those questions appear again. This also gives systems using Birocrat a simple system for remembering answers to display them again. As such, it is important questions have unique identifiers, and that the same question asked in different places has the same identifier! Any two different questions which share the same identifier will be treated identically by Birocrat, which will lead to problems beyond caching! Note that an ID can be as simple as a number, and this is the typical pattern.

## License

See [`LICENSE`](LICENSE).
