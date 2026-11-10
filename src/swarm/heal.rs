//! SWARM-04: bounded self-healing retry loop for failing verify commands.

use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

pub const DEFAULT_FEEDBACK_BYTES: usize = 4096;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HealPolicy {
    pub max_retries: u32,
    pub max_feedback_bytes: usize,
}

impl HealPolicy {
    pub fn new(max_retries: u32) -> Self {
        Self {
            max_retries,
            max_feedback_bytes: DEFAULT_FEEDBACK_BYTES,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HealDecision {
    /// Send this prompt to the agent and re-verify.
    Retry {
        attempt: u32,
        prompt: String,
    },
    Exhausted {
        attempts: u32,
    },
}

#[derive(Debug)]
pub struct HealLoop {
    policy: HealPolicy,
    retries_used: u32,
}

impl HealLoop {
    pub fn new(policy: HealPolicy) -> Self {
        Self {
            policy,
            retries_used: 0,
        }
    }

    pub fn retries_used(&self) -> u32 {
        self.retries_used
    }

    /// Record a failed verification and decide whether to retry.
    pub fn on_failure(&mut self, cmd: &str, exit_code: i32, output: &str) -> HealDecision {
        if self.retries_used >= self.policy.max_retries {
            return HealDecision::Exhausted {
                attempts: self.retries_used + 1,
            };
        }
        self.retries_used += 1;
        HealDecision::Retry {
            attempt: self.retries_used,
            prompt: synthesize_prompt(cmd, exit_code, output, self.retries_used, &self.policy),
        }
    }
}

/// Strip ANSI CSI/OSC escape sequences and stray control chars (keeps \n, \t).
pub fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        if c == '\x1b' {
            match it.peek() {
                Some('[') => {
                    it.next();
                    for n in it.by_ref() {
                        if ('@'..='~').contains(&n) {
                            break;
                        }
                    }
                }
                Some(']') => {
                    it.next();
                    while let Some(n) = it.next() {
                        if n == '\x07' {
                            break;
                        }
                        if n == '\x1b' {
                            it.next();
                            break;
                        }
                    }
                }
                _ => {
                    it.next();
                }
            }
        } else if c == '\n' || c == '\t' || !c.is_control() {
            out.push(c);
        }
    }
    out
}

/// Keep the last `max` bytes on a char boundary.
pub fn tail_bytes(s: &str, max: usize) -> &str {
    if s.len() <= max {
        return s;
    }
    let mut start = s.len() - max;
    while !s.is_char_boundary(start) {
        start += 1;
    }
    &s[start..]
}

pub fn synthesize_prompt(
    cmd: &str,
    exit_code: i32,
    output: &str,
    attempt: u32,
    p: &HealPolicy,
) -> String {
    let clean = strip_ansi(output);
    let tail = tail_bytes(&clean, p.max_feedback_bytes);
    let truncated = if tail.len() < clean.len() {
        " (truncated to tail)"
    } else {
        ""
    };
    format!(
        "Verification failed (attempt {attempt}/{}): `{cmd}` exited with code {exit_code}.\n\
         Output{truncated}:\n{tail}\n\
         Fix the problem so the command passes.",
        p.max_retries
    )
}

/// Split a command line into argv without invoking a shell (quotes only).
pub fn split_argv(cmd: &str) -> Result<Vec<String>, String> {
    let mut args = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    let mut has = false;
    for c in cmd.chars() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), c) => cur.push(c),
            (None, '"') | (None, '\'') => {
                quote = Some(c);
                has = true;
            }
            (None, c) if c.is_whitespace() => {
                if has || !cur.is_empty() {
                    args.push(std::mem::take(&mut cur));
                    has = false;
                }
            }
            (None, c) => cur.push(c),
        }
    }
    if quote.is_some() {
        return Err("unterminated quote".into());
    }
    if has || !cur.is_empty() {
        args.push(cur);
    }
    if args.is_empty() {
        return Err("empty command".into());
    }
    Ok(args)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifyResult {
    pub exit_code: i32,
    pub output: String,
    pub timed_out: bool,
}

/// Run a verify command (argv, no shell) in `cwd` with a timeout.
pub fn run_verify(cmd: &str, cwd: &Path, timeout: Duration) -> Result<VerifyResult, String> {
    let argv = split_argv(cmd)?;
    let mut child = Command::new(&argv[0])
        .args(&argv[1..])
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    let mut so = child.stdout.take().expect("piped");
    let mut se = child.stderr.take().expect("piped");
    let t1 = std::thread::spawn(move || {
        let mut b = Vec::new();
        let _ = so.read_to_end(&mut b);
        b
    });
    let t2 = std::thread::spawn(move || {
        let mut b = Vec::new();
        let _ = se.read_to_end(&mut b);
        b
    });
    let start = Instant::now();
    let (code, timed_out) = loop {
        match child.try_wait().map_err(|e| e.to_string())? {
            Some(st) => break (st.code().unwrap_or(-1), false),
            None if start.elapsed() >= timeout => {
                let _ = child.kill();
                let _ = child.wait();
                break (-1, true);
            }
            None => std::thread::sleep(Duration::from_millis(10)),
        }
    };
    let mut output = String::from_utf8_lossy(&t1.join().unwrap_or_default()).into_owned();
    output.push_str(&String::from_utf8_lossy(&t2.join().unwrap_or_default()));
    Ok(VerifyResult {
        exit_code: code,
        output,
        timed_out,
    })
}
