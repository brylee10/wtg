# WTG (What The GPT) ❓
Chat with your program logs!

A command line program that allows you to pass the terminal output of the last command run to a GPT as context for a one time question or an extended chat. Supports Unix like OSes.

Why `wtg`? Relevant, expresses questioning, and typeable with one hand (in fact, all the subcommands are)! 

<p align="center">
    <img src="https://i.imgur.com/h2t2gR2.gif" alt="WTG Demo" width="800"/>
</p>

# Installation
Ensure Rust is installed on your machine and then install `wtg` as a local binary crate
```shell
cargo install wtg
```

# Usage
`wtg` supports queries (one time questions) and chats (multiple follow ups). Both can be done inside or outside a `wtg` session. Logs are only recorded inside `wtg` sessions.

## Starting a `wtg` session
Start a `wtg` session in a terminal.  
```shell
wtg s /path/to/log/file
```
Under the hood, this starts a new pseudo terminal where `wtg` appends commands' `stdout` and `stderr` for a log file (closely mirroring `script` in UNIX). Additionally, `wtg` adds delimiters in the log to indicate the start and end of different commands' output. This sets the `WTG_LOG` env variable to the provided log file.

To ask a model about the last run's output
```shell
wtg q
```
If no file name is provided, this implicitly queries the file specified by the `WTG_LOG` environment variable.

Start an extended chat about the last run's output
```shell
wtg c
```
This has similar logfile semantics as `wtg q`.

End a `wtg` session (a nested shell session) with the typical `exit` command.

## Running commands outside of a session
Not all commands need to be run in a `wtg` session. You can run a query against any explicitly specified log file
```shell
wtg q -l /path/to/log/file
``` 
If no log file is provided, this falls back to using the file specified in `WTG_LOG`.

`wtg c` for chat can be used in a similar way.

`wtg q` can also take `stdout/stderr` from another command via pipe.
```shell
some_program 2>&1 | wtg q
``` 
Chats cannot 

## Additional CLI Options
Queries (`wtg q`) are run with a default prompt. This prompt can be customized per run
```shell
wtg q -p "A custom prompt"
```

The model queries and prompts can also be specified
```shell
wtg c -m "o3-mini"
```

## Environment Variables
Environment variables are used so users can customize default behavior of `wtg` commands while reducing typing of redundant CLI args.

- `WTG_OPENAI_KEY`: Required. The OpenAI API key to use for any queries or chats.
- `WTG_LOG`: Optional for queries and chats. Specifies the absolute (recommended) or relative log file to use for queries and chats. If not specified, `logfile` arg must be provided.
- `WTG_LLM`: Optional. The model to use for the session (default: gpt-4o, also valid: gpt-4o-mini, o3-mini)
- `WTG_PROMPT`: Optional. The default prompt to use for `query` if none is provided by the user.

`wtg` queries and chats use the below environment variables. For equivalent options, the fallback order is (1) the parameter CLI argument (if applicable), (2) the environment variable, (3) the hard coded default (if applicable).

For example, the prompt used in `query` (not applicable to `chat`, since all prompts are user provided) the model will be (1) the `-p` parameter if provided, (2) the `WTG_PROMPT` env var if set, (3) the default prompt `DEFAULT_QUERY`.

Similarly, the log file used for contexts in queries and chats will be (1) the `-l` parameter if provided, (2) the `WTG_LOG` variable if set, (3) N/A as the log file does not have a default. 

Lastly, the model used is selected by, (1) the `-m` if provided, (2) the `WTG_LLM` env var if set, (3) the default `DEFAULT_LLM`. 

These environment variables can be added to `~/.bashrc`,  `~/.zshrc` or similar shell configuration files.

## Notes
If using `wtl` with `tmux`, it's more convenient to start the `tmux` session first and then start `wtl`. If done in the reverse order, `tmux` may clear the `WTG_LOG` env var (which the `wtl` session sets). You would need to reinitialize this variable or pass the logfile to the `q` and `c` subcommands.