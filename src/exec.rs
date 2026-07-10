use std::io::Read;
use std::process::{Command, Stdio};
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
