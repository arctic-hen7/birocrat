use clap::Parser;

/// birocrat-cli lets you run complex forms powered by Lua in your terminal!
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// Path to a Lua script that drives the form (if `-`, this will read from stdin)
    pub script: String,
    #[arg(short, long = "param")]
    pub params: Vec<String>,
}
