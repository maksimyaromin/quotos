//! Drives Claude Code's own sign-in for one account. See "Sign-in
//! recovery" in claude-provider.md for the full argument: what Quotos does
//! and does not touch, and why completion is detected by the process
//! exiting rather than trusted from its exit status.
//!
//! `claude setup-token` opens a browser at the authorization URL itself,
//! then waits for a pasted code on stdin, confirmed by reading `claude
//! setup-token --help` and by observing its real behavior. So Quotos's job
//! is only to start that process pointed at the right account's
//! `CLAUDE_CONFIG_DIR`, relay a pasted code back into its stdin, and notice
//! when it is done. It never parses the URL out and never opens a browser
//! itself.
//!
//! The CLI renders an interactive, cursor-positioning prompt using ANSI
//! cursor movement rather than plain line output, so it is spawned
//! attached to a real pty through `portable-pty` rather than plain pipes.
//! A plain pipe risks the CLI detecting a non-tty stdin and refusing or
//! silently changing behavior. Quotos never reads or displays that output.
//! It only needs the pty alive long enough for the CLI to behave as it
//! does in a real terminal.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::Mutex;

use portable_pty::{ChildKiller, CommandBuilder, PtySize, native_pty_system};
use serde::Serialize;
use tauri::{AppHandle, Emitter};

struct Session {
    killer: Mutex<Box<dyn ChildKiller + Send + Sync>>,
    writer: Mutex<Box<dyn Write + Send>>,
}

pub struct SignInRegistry {
    sessions: Mutex<HashMap<String, Session>>,
}

#[derive(Serialize, Clone)]
struct SignInFinished {
    account_id: String,
    success: bool,
}

impl SignInRegistry {
    pub fn new() -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
        }
    }

    /// The row's action should already be disabled while a sign-in is in
    /// progress, but the check against `sessions` below is the actual
    /// guard against starting a second one.
    pub fn start(
        &self,
        app: AppHandle,
        account_id: String,
        config_dir: String,
    ) -> Result<(), String> {
        {
            let sessions = self.sessions.lock().expect("sign-in registry poisoned");
            if sessions.contains_key(&account_id) {
                return Err("a sign-in is already in progress for this account".to_string());
            }
        }

        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows: 24,
                cols: 80,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| e.to_string())?;

        // A Finder-launched .app inherits no PATH, so spawning "claude" by
        // name would fail to find anything, and forcing CLAUDE_CONFIG_DIR
        // for the default account would point Claude Code at a config it
        // treats as signed out, writing a credential Quotos never reads.
        let invocation = crate::providers::claude::cli_invocation(Path::new(&config_dir)).ok_or_else(|| {
            "Quotos couldn't find the Claude Code command on this Mac. Open Claude Code once, then try again."
                .to_string()
        })?;

        let mut cmd = CommandBuilder::new(invocation.program.as_os_str());
        cmd.arg("setup-token");
        match &invocation.config_dir_env {
            Some(dir) => cmd.env("CLAUDE_CONFIG_DIR", dir),
            None => cmd.env_remove("CLAUDE_CONFIG_DIR"),
        }
        cmd.env("PATH", &invocation.path_env);

        let child = pair.slave.spawn_command(cmd).map_err(|e| e.to_string())?;
        // This copy of the slave must close so the pty can signal EOF once
        // the child itself exits. Otherwise the reader thread below never
        // sees end-of-stream.
        drop(pair.slave);

        let killer = child.clone_killer();
        let reader = pair.master.try_clone_reader().map_err(|e| e.to_string())?;
        let writer = pair.master.take_writer().map_err(|e| e.to_string())?;

        spawn_output_drain(reader);
        spawn_wait_and_notify(app.clone(), account_id.clone(), child, pair.master);

        let mut sessions = self.sessions.lock().expect("sign-in registry poisoned");
        sessions.insert(
            account_id,
            Session {
                killer: Mutex::new(killer),
                writer: Mutex::new(writer),
            },
        );
        Ok(())
    }

    pub fn submit_code(&self, account_id: &str, code: &str) -> Result<(), String> {
        let sessions = self.sessions.lock().expect("sign-in registry poisoned");
        let session = sessions
            .get(account_id)
            .ok_or_else(|| "no sign-in in progress for this account".to_string())?;
        let mut writer = session
            .writer
            .lock()
            .expect("sign-in writer mutex poisoned");
        writer
            .write_all(code.trim().as_bytes())
            .map_err(|e| e.to_string())?;
        writer.write_all(b"\n").map_err(|e| e.to_string())?;
        writer.flush().map_err(|e| e.to_string())
    }

    /// Called either as the panel's own cancel action or as cleanup when
    /// the row is removed mid-flow.
    pub fn cancel(&self, account_id: &str) {
        let mut sessions = self.sessions.lock().expect("sign-in registry poisoned");
        if let Some(session) = sessions.remove(account_id) {
            let _ = session
                .killer
                .lock()
                .expect("sign-in killer mutex poisoned")
                .kill();
        }
    }

    /// Called after the frontend receives `sign-in-finished`, so a retry
    /// starts clean.
    pub fn forget(&self, account_id: &str) {
        let mut sessions = self.sessions.lock().expect("sign-in registry poisoned");
        sessions.remove(account_id);
    }
}

/// Drains the pty continuously so the child never blocks writing to a full
/// buffer.
fn spawn_output_drain(mut reader: Box<dyn Read + Send>) {
    std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(_) => {}
            }
        }
    });
}

/// This thread owns `child` for the rest of the session, and by extension
/// `master`, since dropping the master before the child exits can tear
/// down the pty out from under it. `killer`, cloned by the caller before
/// this call, is the independent handle `cancel` uses, so cancel never
/// contends with this thread's blocking wait.
fn spawn_wait_and_notify(
    app: AppHandle,
    account_id: String,
    mut child: Box<dyn portable_pty::Child + Send + Sync>,
    master: Box<dyn portable_pty::MasterPty + Send>,
) {
    std::thread::spawn(move || {
        let status = child.wait();
        let success = matches!(status, Ok(s) if s.success());
        let _ = app.emit(
            "sign-in-finished",
            SignInFinished {
                account_id,
                success,
            },
        );
        drop(master);
    });
}
