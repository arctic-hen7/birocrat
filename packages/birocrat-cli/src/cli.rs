use clap::{Args, Parser};
use std::path::PathBuf;

/// birocrat-cli lets you run complex forms powered by Lua in your terminal!
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Cli {
    /// Path to a Lua script that drives the form (if `-`, this will read from stdin)
    pub script: String,
    /// Arbitrary parameters to go to the form
    #[command(flatten)]
    pub params: ParamsArgs,
    /// Where to put the JSON output [default: stdout]
    #[arg(short, long)]
    pub output: Option<PathBuf>,
}

#[derive(Args, Debug)]
#[group(required = false, multiple = false)]
pub struct ParamsArgs {
    /// Arbitrary parameters to go to the form (`key=value`)
    #[arg(short, long = "param")]
    pub params: Option<Vec<String>>,
    /// The path to a JSON file containing the parameters
    #[arg(short = 'j', long = "json-params")]
    pub json_params: Option<PathBuf>,
}
