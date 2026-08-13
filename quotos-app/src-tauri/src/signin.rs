//! R2-6: drives Claude Code's own sign-in for one account, without Quotos
//! ever touching the Keychain or handling a credential itself — the
//! captain's own decision, verbatim: "Quotos drives Claude Code's own
//! login. Claude Code writes the credential. Quotos never touches the
//! Keychain and never handles a credential."
//!
//! `claude setup-token` already does exactly what the captain asked for —
//! confirmed by reading `claude setup-token --help` and by one careful,
//! throwaway-`CLAUDE_CONFIG_DIR` observation of its real behavior: it opens
//! a browser at the authorization URL *itself*, then waits for a pasted
//! code on stdin. So Quotos's job is only to start that process pointed at
//! the right account's `CLAUDE_CONFIG_DIR`, relay a pasted code back into
//! its stdin, and notice when it's done — never to parse the URL out or
//! open a browser itself.
//!
//! That one observation also showed the CLI rendering an interactive,
//! cursor-positioning prompt (ANSI cursor movement, not plain line output),
//! so it's spawned attached to a real pty via `portable-pty` rather than
//! plain pipes — a plain pipe risks the CLI detecting a non-tty stdin and
//! refusing or silently changing behavior. Quotos never reads or displays
//! that output; it only needs the pty alive long enough for the CLI to
//! behave as it does in a real terminal.
//!
//! **What is verified and what isn't** (see also AGENTS.md and the round's
//! status report): confirmed — the CLI accepts `CLAUDE_CONFIG_DIR`, prints
//! an authorization URL, attempts to open a browser, and then prints a
//! "paste code here" prompt. Not verified — what happens after a real code
//! is pasted (whether the process exits 0 and Claude Code has written a
//! working credential), because completing that would have meant finishing
//! a real login, which is explicitly the captain's own final check to make,
//! not this agent's.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::Mutex;

use portable_pty::{native_pty_system, ChildKiller, CommandBuilder, PtySize};
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
        Self { sessions: Mutex::new(HashMap::new()) }
    }

    /// Starts `claude setup-token` for one account. Fails fast if a session
    /// for that account is already running rather than starting a second
    /// one — the row's action should be disabled while in progress, but
    /// this is the actual guard.
    pub fn start(&self, app: AppHandle, account_id: String, config_dir: String) -> Result<(), String> {
        {
            let sessions = self.sessions.lock().expect("sign-in registry poisoned");
            if sessions.contains_key(&account_id) {
                return Err("a sign-in is already in progress for this account".to_string());
            }
        }

        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize { rows: 24, cols: 80, pixel_width: 0, pixel_height: 0 })
            .map_err(|e| e.to_string())?;

        let mut cmd = CommandBuilder::new("claude");
        cmd.arg("setup-token");
        cmd.env("CLAUDE_CONFIG_DIR", &config_dir);

        let mut child = pair.slave.spawn_command(cmd).map_err(|e| e.to_string())?;
        // Our copy of the slave must close so the pty can signal EOF once
        // the child itself exits — otherwise the reader thread below never
        // sees end-of-stream.
        drop(pair.slave);

        let killer = child.clone_killer();
        let mut reader = pair.master.try_clone_reader().map_err(|e| e.to_string())?;
        let writer = pair.master.take_writer().map_err(|e| e.to_string())?;

        // Drain output continuously so the child never blocks writing to a
        // full pty buffer. Quotos doesn't parse or display any of it — the
        // CLI already opens the browser and prints the URL on its own; this
        // thread's only job is to keep the pipe flowing.
        std::thread::spawn(move || {
            let mut buf = [0u8; 4096];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {}
                }
            }
        });

        // Owns `child` (and, by extension, `pair.master` — dropping the
        // master before the child exits can tear down the pty out from
        // under it) for the rest of the session; `killer`, cloned above, is
        // the independent handle `cancel` uses so it never has to contend
        // with this thread's blocking `.wait()`.
        let master = pair.master;
        let wait_app = app.clone();
        let wait_account_id = account_id.clone();
        std::thread::spawn(move || {
            let status = child.wait();
            let success = matches!(status, Ok(s) if s.success());
            let _ = wait_app.emit("sign-in-finished", SignInFinished { account_id: wait_account_id, success });
            drop(master);
        });

        let mut sessions = self.sessions.lock().expect("sign-in registry poisoned");
        sessions.insert(account_id, Session { killer: Mutex::new(killer), writer: Mutex::new(writer) });
        Ok(())
    }

    /// Relays a pasted authorization code into the waiting process's stdin,
    /// exactly as if it had been typed into a real terminal.
    pub fn submit_code(&self, account_id: &str, code: &str) -> Result<(), String> {
        let sessions = self.sessions.lock().expect("sign-in registry poisoned");
        let session = sessions.get(account_id).ok_or_else(|| "no sign-in in progress for this account".to_string())?;
        let mut writer = session.writer.lock().expect("sign-in writer mutex poisoned");
        writer.write_all(code.trim().as_bytes()).map_err(|e| e.to_string())?;
        writer.write_all(b"\n").map_err(|e| e.to_string())?;
        writer.flush().map_err(|e| e.to_string())
    }

    /// Kills the in-progress process for `account_id`, if any — the panel's
    /// own cancel action, or cleanup if the row is removed mid-flow.
    pub fn cancel(&self, account_id: &str) {
        let mut sessions = self.sessions.lock().expect("sign-in registry poisoned");
        if let Some(session) = sessions.remove(account_id) {
            let _ = session.killer.lock().expect("sign-in killer mutex poisoned").kill();
        }
    }

    /// Drops bookkeeping once a session has finished (called after the
    /// frontend receives `sign-in-finished`), so a retry starts clean.
    pub fn forget(&self, account_id: &str) {
        let mut sessions = self.sessions.lock().expect("sign-in registry poisoned");
        sessions.remove(account_id);
    }
}
