//! # What The GPT (`wtg`)
//! Chat with your program logs!
//!
//! A command line program that allows you to pass the terminal output of the last command run to a GPT as context for a one time question or an extended chat. Supports Unix like OSes.
//!
//! Why `wtg`? Relevant, expresses questioning, and typeable with one hand (in fact, all the subcommands are)!
//!
//! Additional documentation can be found in the [README.md](https://github.com/brylee10/wtg/blob/main/README.md).
//!
//! ## Usage
//! These are the library crate documentation for `wtg`. For usage of the binary install the local binary crate (`cargo install wtg`) and see
//! ```shell
//! $ wtg --help
//! ```
//!
//! ## Environment Variables:
//! - `WTG_OPENAI_KEY`: Required. The OpenAI API key to use for any queries or chats.
//! - `WTG_LOG`: Optional for queries and chats. Specifies the absolute (recommended) or relative log file to use for queries and chats. If not specified, `logfile` arg must be provided.
//! - `WTG_LLM`: Optional. The model to use for the session (default: gpt-4o, also valid: gpt-4o-mini, o3-mini)
//! - `WTG_PROMPT`: Optional. The default prompt to use for `query` if none is provided by the user.
//!
//! ## Notes:
//! - The WTG session uses a heuristic to determine new commands.
//!   A "new command indicator" is added to the log file on each new command as indicated by a new line.
//! - Similar to `script`, the log file is not automatically cleaned up for visibility after a session.
//!   Users should manually delete the log when the session is complete
//!   and the log is not needed
//!
pub mod cli;
pub mod errors;
pub mod openai;
pub mod session;
