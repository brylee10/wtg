use clap::Parser;
use wtg::{
    cli::{Args, Commands},
    session::{run_chat, run_query, run_session},
};

fn main() {
    let args = Args::parse();
    let res = match args.command {
        Commands::Start { logfile } => run_session(&logfile),
        Commands::Query {
            logfile,
            prompt,
            model,
        } => run_query(logfile, prompt, model),
        Commands::Chat { logfile, model } => run_chat(logfile, model),
    };
    res.unwrap_or_else(|e| {
        eprintln!("{}", e);
        std::process::exit(1);
    });
}
