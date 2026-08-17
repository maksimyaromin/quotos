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

    pub fn forget(&self, account_id: &str) {
        let mut sessions = self.sessions.lock().expect("sign-in registry poisoned");
        sessions.remove(account_id);
    }
}

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
