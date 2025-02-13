//! Components to manage a WTG session and query/chat with the last command's output.

use std::os::fd::AsFd;
use std::path::{Path, PathBuf};

use libc::{kill, SIGWINCH};
use nix::pty::{forkpty, ForkptyResult, Winsize};
use nix::sys::termios::{cfmakeraw, tcgetattr, tcsetattr, LocalFlags, SetArg, Termios};
use nix::sys::wait::waitpid;
use nix::unistd::{execvp, Pid};
use signal_hook::iterator::Signals;
use std::env;
use std::ffi::CString;
use std::fs::{File, OpenOptions};
use std::io::{self, BufReader, Read, Write};
use std::os::fd::{AsRawFd, FromRawFd, RawFd};
use std::sync::{mpsc, Arc, Mutex};

use crate::cli::{Model, NEW_COMMAND_MSG};
use crate::errors::WtgError;
use crate::openai::query_chatgpt;

/// Convert the original terminal to raw mode so characters are sent immediately to the pty
/// So the pty can process ANSI escape sequences. Also disable default echo of user input.
/// Restore after session completes.
// Similar to `script`: https://github.com/freebsd/freebsd-src/blob/main/usr.bin/script/script.c#L252-L257
struct RawModeGuard<F: AsFd> {
    fd: F,
    orig_termios: Termios,
}

impl<F: AsFd> RawModeGuard<F> {
    fn new(fd: F) -> nix::Result<Self> {
        let orig_termios = tcgetattr(fd.as_fd())?;
        Ok(RawModeGuard { fd, orig_termios })
    }

    fn enable_raw_mode(&self) -> nix::Result<()> {
        let mut raw = self.orig_termios.clone();
        cfmakeraw(&mut raw);
        raw.local_flags.remove(LocalFlags::ECHO);
        tcsetattr(self.fd.as_fd(), SetArg::TCSANOW, &raw)
    }
}

impl<F: AsFd> Drop for RawModeGuard<F> {
    fn drop(&mut self) {
        // Restore the original terminal settings.
        let _ = tcsetattr(self.fd.as_fd(), SetArg::TCSANOW, &self.orig_termios);
    }
}

/// Get the parent terminal window size
fn get_parent_winsize() -> Winsize {
    use libc::{ioctl, winsize, TIOCGWINSZ};

    // stdin is connected to the parent terminal
    let fd = io::stdin().as_raw_fd();

    // mac terminal default size can be considerably larger,
    let mut ws: winsize = winsize {
        ws_row: 24,
        ws_col: 80,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };

    // SAFETY: the window size pointer is valid and the process stdin has not been closed
    if unsafe { ioctl(fd, TIOCGWINSZ, &mut ws) } == -1 {
        panic!("Failed to get window size using ioctl");
    }

    Winsize {
        ws_row: ws.ws_row,
        ws_col: ws.ws_col,
        ws_xpixel: ws.ws_xpixel,
        ws_ypixel: ws.ws_ypixel,
    }
}

/// Update the pty winsize to match the parent terminal window size
fn update_pty_winsize(master_fd: RawFd) -> Result<(), WtgError> {
    let window_size = get_parent_winsize();
    // SAFETY: the window size pointer is valid and the master fd has not been closed
    let ret = unsafe { libc::ioctl(master_fd, libc::TIOCSWINSZ, &window_size) };
    if ret == -1 {
        Err(WtgError::NixError(nix::Error::last()))
    } else {
        Ok(())
    }
}

/// Listen for `SIGWINCH` signal to trigger pty window size updates
fn listen_pty_resize(child_pid_for_resize: Pid, master_fd: RawFd) -> Result<(), WtgError> {
    std::thread::spawn(move || {
        let mut signals =
            Signals::new([SIGWINCH]).expect("Unable to register SIGWINCH signal handler");
        for _ in signals.forever() {
            if let Err(e) = update_pty_winsize(master_fd) {
                eprintln!("Failed to update pty window size: {:?}", e);
            }
            // Also send SIGWINCH to the child so it can adjust its display.
            // SAFETY: the child pid is valid (taken from `forkpty`)
            let _ = unsafe { kill(child_pid_for_resize.into(), SIGWINCH) };
        }
    });
    Ok(())
}

/// Start a WTG session
pub fn run_session(logfile: &str) -> Result<(), WtgError> {
    let path = PathBuf::from(logfile);
    let log = OpenOptions::new()
        .append(true)
        .create(true)
        .open(path.clone())
        .expect("Failed to open log file");
    initialize_env_vars(path)?;

    println!("Starting wtg session. Type 'exit' to quit.");
    // inherit parent window size, can be resized dynamically
    let window_size = get_parent_winsize();
    // forks a child and parent for the pty
    // SAFETY: the child only calls async signal safe functions (per the `forkpty` requirements)
    let fork_result = unsafe { forkpty(Some(&window_size), None).expect("forkpty failed") };

    match fork_result {
        ForkptyResult::Parent { child, master } => {
            // called here, sets the parent's STDIN to raw mode (i.e. the original terminal input), the child is still in cooked mode
            let stdin = std::io::stdin();
            let guard =
                RawModeGuard::new(stdin).expect("Failed to get terminal attributes for raw mode");
            guard.enable_raw_mode().expect("Failed to enable raw mode");
            let master_fd = master.as_raw_fd();
            // SAFETY: the fd must be owned and open. The master fd is an OwnedFd and has not been closed.
            let master_file = unsafe { File::from_raw_fd(master_fd) };
            let mut master_reader = master_file
                .try_clone()
                .expect("Failed to clone master file");
            let mut master_writer = master_file;

            // forward resizes to the pty via master fd
            listen_pty_resize(child, master_fd).expect("Failed to listen for pty resize");

            let (enter_tx, enter_rx) = mpsc::channel::<()>();
            let (truncated_tx, truncated_rx) = mpsc::channel::<()>();

            // take user input and write to the master pty
            std::thread::spawn(move || {
                // in raw mode, every character is sent immediately to the pty stdin
                // in canonical mode, the user input is buffered until a newline is entered
                let stdin = io::stdin();
                let mut input = stdin.lock();
                let mut buf = [0u8; 1024];
                loop {
                    match input.read(&mut buf) {
                        Ok(0) => {
                            break;
                        }
                        Ok(n) => {
                            // log file should only have most recent command output
                            // truncate the log on enter, indicating a new command has started
                            if buf[..n].iter().any(|&b| b == b'\n' || b == b'\r') {
                                let _ = enter_tx.send(());
                                let _ = truncated_rx.recv();
                            }
                            if master_writer.write_all(&buf[..n]).is_err() {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
            });

            let log = Arc::new(Mutex::new(log));
            {
                let log = Arc::clone(&log);
                std::thread::spawn(move || {
                    // Clears the log file on Enter keypress
                    while let Ok(()) = enter_rx.recv() {
                        let mut log = log.lock().unwrap();
                        // write message to log indicating a new command has started
                        log.write_all(NEW_COMMAND_MSG.as_bytes()).unwrap();
                        // sends "ack" that log file has been cleared
                        let _ = truncated_tx.send(());
                    }
                });
            }

            let mut buf = [0u8; 1024];
            loop {
                let n = master_reader
                    .read(&mut buf)
                    .expect("Error reading from PTY");
                if n == 0 {
                    break;
                }
                {
                    let stdout = io::stdout();
                    // acquire lock inside loop so it is periodically released
                    // otherwise, any use of `println!` for debugging would block because the `stdout` lock is always held
                    let mut out = stdout.lock();
                    out.write_all(&buf[..n]).expect("Failed to write to stdout");
                    out.flush().unwrap();
                }
                {
                    let mut log = log.lock().unwrap();
                    log.write_all(&buf[..n]).expect("Failed to write to log");
                    log.flush().unwrap();
                }
            }
            waitpid(child, None).expect("Failed to wait on child");
        }
        ForkptyResult::Child => {
            // the child starts a new tty and is still in cooked mode
            let shell = env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
            let shell_c = CString::new(shell).expect("CString failed");
            let args = [shell_c.clone()];
            execvp(&shell_c, &args).expect("execvp failed");
        }
    }
    Ok(())
}

/// Initialize the default environment variables for the WTG session
fn initialize_env_vars<P: AsRef<Path>>(path: P) -> Result<(), WtgError> {
    env::set_var("WTG_LOG", path.as_ref().canonicalize()?);
    Ok(())
}

/// Get the string contents of the log file
fn get_log_content(logfile: String) -> Result<String, WtgError> {
    let file = File::open(&logfile).map_err(|_| WtgError::LogFileOpenError { logfile })?;
    let mut reader = BufReader::new(file);
    let mut log_vec = Vec::new();
    reader
        .read_to_end(&mut log_vec)
        .expect("Failed to read log file");
    Ok(String::from_utf8_lossy(&log_vec).to_string())
}

/// Extract the output of the last command from the log file
fn extract_context_from_log(logfile: &str) -> Result<String, WtgError> {
    let log_content = get_log_content(logfile.to_string())?;
    // Only take the contents of the log file roughly between the second to last `NEW_COMMAND_MSG` and
    // the last `NEW_COMMAND_MSG`, since the logs following the last correspond to the current
    // `wtg query` command. Also takes the entire line of the second to last `NEW_COMMAND_MSG`
    let last_idx = log_content
        .rfind(NEW_COMMAND_MSG)
        .ok_or(WtgError::NoCommandRun {
            logfile: logfile.to_string(),
        })?;
    let last_line_start = log_content[..last_idx]
        .rfind('\n')
        .map(|i| i + 1)
        .unwrap_or(0);
    let second_to_last_idx =
        log_content[..last_idx]
            .rfind(NEW_COMMAND_MSG)
            .ok_or(WtgError::NoCommandRun {
                logfile: logfile.to_string(),
            })?;
    let second_to_last_line_start = log_content[..second_to_last_idx]
        .rfind('\n')
        .map(|i| i + 1)
        .unwrap_or(0);
    let mut context = log_content[second_to_last_line_start..last_line_start].to_string();
    // strip the `wtg` inserted `NEW_COMMAND_MSG` delimiter from the context
    context = context.replace(NEW_COMMAND_MSG, "");
    Ok(context)
}

/// Run a GPT query using the last log output as context
pub fn run_query(
    logfile: Option<String>,
    prompt: Option<String>,
    model: Option<Model>,
) -> Result<(), WtgError> {
    let stdin_fileno = io::stdin().as_raw_fd();
    let context = if !nix::unistd::isatty(stdin_fileno).unwrap_or(false) {
        let mut piped_input = String::new();
        io::stdin().read_to_string(&mut piped_input).unwrap();
        piped_input
    } else {
        let logfile = logfile.unwrap_or_else(|| env::var("WTG_LOG").expect("WTG_LOG not set"));
        extract_context_from_log(&logfile)?
    };
    let _ = query_chatgpt(&context, prompt.as_deref(), model).unwrap_or_else(|e| {
        eprintln!("Error querying ChatGPT: {}", e);
        String::new()
    });
    Ok(())
}

/// Start a chat using the last log output as context
pub fn run_chat(logfile: Option<String>, model: Option<Model>) -> Result<(), WtgError> {
    // sanity check chat is running from a tty
    let stdin_fileno = io::stdin().as_raw_fd();
    if !nix::unistd::isatty(stdin_fileno).unwrap_or(false) {
        return Err(WtgError::ChatNotTty);
    }
    let logfile = logfile.unwrap_or_else(|| env::var("WTG_LOG").expect("WTG_LOG not set"));
    let mut chat_context = extract_context_from_log(&logfile)?;
    println!("(type 'exit' ('e') or 'quit' ('q') to end chat)");
    loop {
        let prompt_text = {
            print!("user> ");
            io::stdout().flush().unwrap();

            let mut input = String::new();
            if io::stdin().read_line(&mut input).is_err() {
                eprintln!("Error reading from stdin.");
                continue;
            }
            let trimmed = input.trim().to_string();
            // accepts "exit", "e", "q", "quit" to end chat
            if trimmed.to_lowercase() == "exit"
                || trimmed.to_lowercase() == "e"
                || trimmed.to_lowercase() == "q"
                || trimmed.to_lowercase() == "quit"
            {
                break;
            }
            trimmed
        };
        let response =
            query_chatgpt(&chat_context, Some(&prompt_text), model).unwrap_or_else(|e| {
                eprintln!("Error querying ChatGPT: {}", e);
                String::new()
            });
        chat_context.push_str(&format!("\nuser: {}\nassistant: {}", prompt_text, response));
    }
    Ok(())
}
