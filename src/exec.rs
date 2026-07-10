use std::io::{BufRead, BufReader, Read};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use wait_timeout::ChildExt;

/// The result of running an external command under a timeout.
pub struct Captured {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
    pub timed_out: bool,
}

impl Captured {
    pub fn merged(&self) -> String {
        let mut out = self.stdout.clone();
        if !self.stderr.trim().is_empty() {
            out.push_str(&self.stderr);
        }
        out
    }
}

/// Runs a command, capturing stdout/stderr, and kills it if it exceeds `timeout`
/// so bide never hangs on an external tool. Pipes are drained in threads to avoid
/// a deadlock when the child fills a pipe buffer.
pub fn run(mut command: Command, timeout: Duration) -> Captured {
    command.stdout(Stdio::piped()).stderr(Stdio::piped());

    let Ok(mut child) = command.spawn() else {
        return failed("failed to spawn command");
    };
    let out = drain(child.stdout.take());
    let err = drain(child.stderr.take());

    match child.wait_timeout(timeout) {
        Ok(Some(status)) => Captured {
            success: status.success(),
            stdout: join(out),
            stderr: join(err),
            timed_out: false,
        },
        Ok(None) => {
            let _ = child.kill();
            let _ = child.wait();
            let mut stderr = join(err);
            stderr.push_str(&format!("\n[bide] killed: timed out after {}s", timeout.as_secs()));
            Captured {
                success: false,
                stdout: join(out),
                stderr,
                timed_out: true,
            }
        }
        Err(_) => failed("could not wait for command"),
    }
}

/// Like `run`, but reads stdout line by line and hands each line to `on_line` as
/// it arrives, so a long-running tool can show live progress. Still bounded by
/// `timeout`: a watchdog kills the child if it overruns, and the read loop then
/// ends when the pipe closes.
pub fn run_streaming(
    mut command: Command,
    timeout: Duration,
    mut on_line: impl FnMut(&str),
) -> Captured {
    command.stdout(Stdio::piped()).stderr(Stdio::piped());

    let Ok(mut child) = command.spawn() else {
        return failed("failed to spawn command");
    };
    let stdout = child.stdout.take();
    let err = drain(child.stderr.take());

    let child = Arc::new(Mutex::new(child));
    let done = Arc::new(AtomicBool::new(false));
    let watchdog = spawn_watchdog(Arc::clone(&child), Arc::clone(&done), timeout);

    let mut stdout_text = String::new();
    if let Some(stdout) = stdout {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            on_line(&line);
            stdout_text.push_str(&line);
            stdout_text.push('\n');
        }
    }
    done.store(true, Ordering::Relaxed);
    let timed_out = watchdog.join().unwrap_or(false);

    let succeeded = child
        .lock()
        .ok()
        .and_then(|mut child| child.wait().ok())
        .map(|status| status.success())
        .unwrap_or(false);
    let mut stderr = join(err);
    if timed_out {
        stderr.push_str(&format!("\n[bide] killed: timed out after {}s", timeout.as_secs()));
    }
    Captured {
        success: succeeded && !timed_out,
        stdout: stdout_text,
        stderr,
        timed_out,
    }
}

/// Kills the child if it outlives `timeout`. Returns whether it had to.
fn spawn_watchdog(
    child: Arc<Mutex<Child>>,
    done: Arc<AtomicBool>,
    timeout: Duration,
) -> JoinHandle<bool> {
    thread::spawn(move || {
        let step = Duration::from_millis(100);
        let mut waited = Duration::ZERO;
        while waited < timeout {
            if done.load(Ordering::Relaxed) {
                return false;
            }
            thread::sleep(step);
            waited += step;
        }
        if done.load(Ordering::Relaxed) {
            return false;
        }
        if let Ok(mut child) = child.lock() {
            let _ = child.kill();
        }
        true
    })
}

fn drain<R: Read + Send + 'static>(pipe: Option<R>) -> JoinHandle<String> {
    thread::spawn(move || {
        let mut buffer = String::new();
        if let Some(mut pipe) = pipe {
            let _ = pipe.read_to_string(&mut buffer);
        }
        buffer
    })
}

fn join(handle: JoinHandle<String>) -> String {
    handle.join().unwrap_or_default()
}

fn failed(message: &str) -> Captured {
    Captured {
        success: false,
        stdout: String::new(),
        stderr: message.to_string(),
        timed_out: false,
    }
}
