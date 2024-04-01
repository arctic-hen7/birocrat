use clap::Parser;
use std::path::PathBuf;

/// birocrat-cli lets you run complex forms powered by Lua in your terminal!
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// Path to a Lua script that drives the form
    pub script: PathBuf,
}
