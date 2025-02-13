use std::{
    fmt::{self, Display},
    str::FromStr,
};

use clap::{command, Parser, Subcommand};

/// The default model to use for queries and chats.
pub const DEFAULT_LLM: Model = Model::Gpt4o;
/// The default prompt to use for queries.
pub const DEFAULT_QUERY: &str =
    "Here is the program output. If there was an error, concisely explain how it can be fixed. 
If there was no error, concisely summarize the output.";
/// A delimiter to indicate the end of a command, but prior to the output.
// Ideally, there would be a more robust way to detect the end of a command
// using stdin and detecting the end of a running subprocess in a shell.
pub const NEW_COMMAND_MSG: &str = "<<<wtg:cmd-end>>>";

/// Various models supported by WTG
#[derive(Debug, Clone, Copy)]
pub enum Model {
    Gpt4o,
    Gpt4oMini,
    O3Mini,
}

impl Model {
    pub fn all_models() -> Vec<String> {
        [Model::Gpt4o, Model::Gpt4oMini, Model::O3Mini]
            .iter()
            .map(|m| m.to_string())
            .collect()
    }
}

impl FromStr for Model {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "gpt-4o" => Ok(Model::Gpt4o),
            "gpt4o" => Ok(Model::Gpt4o),
            "gpt-4o-mini" => Ok(Model::Gpt4oMini),
            "gpt4o-mini" => Ok(Model::Gpt4oMini),
            "o3-mini" => Ok(Model::O3Mini),
            "o3mini" => Ok(Model::O3Mini),
            _ => Err(format!(
                "Invalid model: {}. Choose from: gpt-4o, gpt-4o-mini, o3-mini.",
                s
            )),
        }
    }
}

impl Display for Model {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Model::Gpt4o => write!(f, "gpt-4o"),
            Model::Gpt4oMini => write!(f, "gpt-4o-mini"),
            Model::O3Mini => write!(f, "o3-mini"),
        }
    }
}

/// CLI for `wtg`
#[derive(Debug, Parser)]
#[command(author, version, about)]
pub struct Args {
    #[command(subcommand)]
    pub command: Commands,
}

/// WTG subcommands
#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Starts a new WTG session. Output of the most recent command is
    /// logged to the file specified. This log file is also set as
    /// `WTG_LOG` env var.
    #[command(alias = "s")]
    Start { logfile: String },
    /// Queries GPT using the log file as context. Log file taken from
    /// CLI arg or `WTG_LOG` env var.
    #[command(alias = "q")]
    Query {
        #[arg(short, long)]
        logfile: Option<String>,
        #[arg(short, long)]
        prompt: Option<String>,
        #[arg(short, long)]
        model: Option<Model>,
    },
    /// Start a chat session with the last command's output and all
    /// subsequent chat messages as context.
    #[command(alias = "c")]
    Chat {
        #[arg(short, long)]
        logfile: Option<String>,
        #[arg(short, long)]
        model: Option<Model>,
    },
}
