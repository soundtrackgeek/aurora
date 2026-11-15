//! Aurora's AI assistant: settings, CLI detection, and background runs.
//!
//! The assistant never talks to a model directly. It starts the user's own
//! Claude Code or OpenAI Codex CLI in non-interactive mode, gives it only
//! Aurora's read-only MCP server (`aurora --mcp`), and turns the CLI's JSON
//! event stream into `assistant://event` messages for the WebView. The CLI's
//! own sign-in pays for the requests, so Aurora handles no AI credentials.
//!
//! Each CLI is locked down to Aurora's tools: Claude Code gets no built-in
//! tools, only Aurora's MCP server, and no user settings or hooks; Codex
//! ignores the user's config, runs in its read-only sandbox, never asks for
//! approval, and has its shell, apps, plugins, and sub-agents turned off.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    fs,
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tauri::{AppHandle, Emitter};

const SETTINGS_FILE: &str = "aurora-assistant.json";
const EVENT: &str = "assistant://event";
const MAX_PROMPT_CHARS: usize = 8_000;
const MAX_CONTEXT_CHARS: usize = 2_000;
const RUN_TIMEOUT: Duration = Duration::from_secs(10 * 60);
const STDERR_TAIL_BYTES: usize = 4_000;

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Backend {
    #[default]
    ClaudeCode,
    Codex,
}

impl Backend {
    fn label(self) -> &'static str {
        match self {
            Self::ClaudeCode => "Claude Code",
            Self::Codex => "OpenAI Codex",
        }
    }
}

/// Fast, inexpensive models suit library questions: the tools do the work.
pub(crate) const DEFAULT_CLAUDE_MODEL: &str = "claude-haiku-5-5";
pub(crate) const DEFAULT_CODEX_MODEL: &str = "gpt-6-luna";

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct AssistantSettings {
    /// Lets MCP clients and the Assistant read the library. Off by default.
    pub(crate) library_access: bool,
    pub(crate) backend: Backend,
    /// A user-chosen CLI executable; empty means detect it.
    pub(crate) claude_path: String,
    pub(crate) codex_path: String,
    /// The model each CLI runs in Aurora; empty means the CLI's own default.
    pub(crate) claude_model: String,
    pub(crate) codex_model: String,
}

impl Default for AssistantSettings {
    fn default() -> Self {
        Self {
            library_access: false,
            backend: Backend::default(),
            claude_path: String::new(),
            codex_path: String::new(),
            claude_model: DEFAULT_CLAUDE_MODEL.to_owned(),
            codex_model: DEFAULT_CODEX_MODEL.to_owned(),
        }
    }
}

pub(crate) fn read_settings(directory: &Path) -> AssistantSettings {
    fs::read_to_string(directory.join(SETTINGS_FILE))
        .ok()
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default()
}

/// Read on every MCP tool call, so turning access off takes effect at once.
pub(crate) fn library_access_enabled(directory: &Path) -> bool {
    read_settings(directory).library_access
}

fn write_settings(directory: &Path, settings: &AssistantSettings) -> Result<(), String> {
    fs::create_dir_all(directory)
        .map_err(|error| format!("Could not create Aurora's settings folder: {error}"))?;
    let json = serde_json::to_string_pretty(settings)
        .map_err(|error| format!("Could not encode the assistant settings: {error}"))?;
    let path = directory.join(SETTINGS_FILE);
    let staged = path.with_extension("json.tmp");
    fs::write(&staged, json)
        .and_then(|()| fs::rename(&staged, &path))
        .map_err(|error| format!("Could not save the assistant settings: {error}"))
}

// ---------------------------------------------------------------------------
// Status and settings commands
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CliStatus {
    /// The executable Aurora will run, if any was found.
    path: Option<String>,
    /// "settings" when the user chose it, "detected" when Aurora found it.
    source: Option<&'static str>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AssistantStatus {
    settings: AssistantSettings,
    claude: CliStatus,
    codex: CliStatus,
    /// The command that starts Aurora's MCP server, for `claude mcp add` etc.
    mcp_command: Vec<String>,
}

pub(crate) fn status(directory: &Path) -> AssistantStatus {
    let settings = read_settings(directory);
    let claude = cli_status(&settings.claude_path, detect_claude);
    let codex = cli_status(&settings.codex_path, detect_codex);
    AssistantStatus {
        settings,
        claude,
        codex,
        mcp_command: mcp_command(),
    }
}

fn cli_status(configured: &str, detect: fn() -> Option<PathBuf>) -> CliStatus {
    let configured = configured.trim();
    if !configured.is_empty() {
        return CliStatus {
            path: Some(configured.to_owned()),
            source: Some("settings"),
        };
    }
    match detect() {
        Some(path) => CliStatus {
            path: Some(path.to_string_lossy().into_owned()),
            source: Some("detected"),
        },
        None => CliStatus {
            path: None,
            source: None,
        },
    }
}

fn mcp_command() -> Vec<String> {
    let executable = std::env::current_exe()
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_else(|_| "aurora".to_owned());
    vec![executable, "--mcp".to_owned()]
}

pub(crate) fn update_settings(
    directory: &Path,
    mut settings: AssistantSettings,
) -> Result<AssistantStatus, String> {
    settings.claude_path = settings.claude_path.trim().to_owned();
    settings.codex_path = settings.codex_path.trim().to_owned();
    settings.claude_model = settings.claude_model.trim().to_owned();
    settings.codex_model = settings.codex_model.trim().to_owned();
    for (label, model) in [
        ("Claude Code", &settings.claude_model),
        ("Codex", &settings.codex_model),
    ] {
        if !model.is_empty() && !valid_model_id(model) {
            return Err(format!(
                "{label} model: use a model id such as {}.",
                if label == "Codex" {
                    DEFAULT_CODEX_MODEL
                } else {
                    DEFAULT_CLAUDE_MODEL
                }
            ));
        }
    }
    for (label, path) in [
        ("Claude Code", &settings.claude_path),
        ("Codex", &settings.codex_path),
    ] {
        if !path.is_empty() {
            validate_executable(Path::new(path), label)?;
        }
    }
    write_settings(directory, &settings)?;
    Ok(status(directory))
}

fn validate_executable(path: &Path, label: &str) -> Result<(), String> {
    if !path.is_absolute() || !path.is_file() {
        return Err(format!(
            "{label}: choose the full path of an existing executable file."
        ));
    }
    if cfg!(windows) {
        let extension = path
            .extension()
            .map(|value| value.to_string_lossy().to_ascii_lowercase());
        if extension.as_deref() != Some("exe") {
            return Err(format!(
                "{label}: choose an .exe file. npm's .cmd and .ps1 wrappers need Node on Aurora's PATH; Codex's codex.exe is inside the npm package's vendor folder."
            ));
        }
    }
    Ok(())
}

/// Runs `<cli> --version` and returns its first line.
pub(crate) fn test_backend(directory: &Path, backend: Backend) -> Result<String, String> {
    let executable = resolve_executable(directory, backend)?;
    let mut command = Command::new(&executable);
    command
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    hide_console(&mut command);
    let mut child = command
        .spawn()
        .map_err(|error| format!("Could not start {}: {error}", backend.label()))?;
    let started = Instant::now();
    loop {
        if child
            .try_wait()
            .map_err(|error| error.to_string())?
            .is_some()
        {
            break;
        }
        if started.elapsed() > Duration::from_secs(20) {
            let _ = child.kill();
            return Err(format!(
                "{} did not answer within 20 seconds.",
                backend.label()
            ));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let output = child
        .wait_with_output()
        .map_err(|error| error.to_string())?;
    let text = String::from_utf8_lossy(&output.stdout);
    let line = text.lines().next().unwrap_or_default().trim().to_owned();
    if !output.status.success() || line.is_empty() {
        return Err(format!(
            "{} did not report a version. {}",
            backend.label(),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(format!("{} {line}", backend.label()))
}

fn resolve_executable(directory: &Path, backend: Backend) -> Result<PathBuf, String> {
    let settings = read_settings(directory);
    let status = match backend {
        Backend::ClaudeCode => cli_status(&settings.claude_path, detect_claude),
        Backend::Codex => cli_status(&settings.codex_path, detect_codex),
    };
    status.path.map(PathBuf::from).ok_or_else(|| {
        format!(
            "Aurora could not find {}. Install it, or choose its executable in Settings → AI assistant.",
            backend.label()
        )
    })
}

// ---------------------------------------------------------------------------
// CLI detection
// ---------------------------------------------------------------------------

fn home() -> Option<PathBuf> {
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from)
}

fn on_path(names: &[&str]) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .flat_map(|directory| names.iter().map(move |name| directory.join(name)))
        .find(|candidate| candidate.is_file())
}

fn detect_claude() -> Option<PathBuf> {
    let name = if cfg!(windows) {
        "claude.exe"
    } else {
        "claude"
    };
    on_path(&[name]).or_else(|| {
        let mut candidates = Vec::new();
        if let Some(home) = home() {
            candidates.push(home.join(".local").join("bin").join(name));
            candidates.push(home.join(".claude").join("local").join(name));
        }
        if !cfg!(windows) {
            candidates.push(PathBuf::from("/opt/homebrew/bin/claude"));
            candidates.push(PathBuf::from("/usr/local/bin/claude"));
        }
        candidates.into_iter().find(|candidate| candidate.is_file())
    })
}

/// npm installs Codex as a Node wrapper, and Node often is not on a GUI app's
/// PATH (for example when it comes from fnm), so Aurora looks for the native
/// codex.exe that the npm package ships in its vendor folder.
fn detect_codex() -> Option<PathBuf> {
    if !cfg!(windows) {
        let mut candidates = vec![
            PathBuf::from("/opt/homebrew/bin/codex"),
            PathBuf::from("/usr/local/bin/codex"),
        ];
        if let Some(home) = home() {
            candidates.push(home.join(".local").join("bin").join("codex"));
        }
        return on_path(&["codex"])
            .or_else(|| candidates.into_iter().find(|candidate| candidate.is_file()));
    }
    if let Some(found) = on_path(&["codex.exe"]) {
        return Some(found);
    }
    let mut package_roots = Vec::new();
    if let Some(app_data) = std::env::var_os("APPDATA").map(PathBuf::from) {
        package_roots.push(app_data.join("npm").join("node_modules"));
        // fnm keeps one global node_modules per installed Node version.
        if let Ok(versions) = fs::read_dir(app_data.join("fnm").join("node-versions")) {
            for version in versions.flatten() {
                package_roots.push(version.path().join("installation").join("node_modules"));
            }
        }
    }
    for root in package_roots {
        for (package, triple) in [
            ("codex-win32-x64", "x86_64-pc-windows-msvc"),
            ("codex-win32-arm64", "aarch64-pc-windows-msvc"),
        ] {
            let vendor = root
                .join("@openai")
                .join("codex")
                .join("node_modules")
                .join("@openai")
                .join(package)
                .join("vendor")
                .join(triple);
            for candidate in [
                vendor.join("bin").join("codex.exe"),
                vendor.join("codex").join("codex.exe"),
            ] {
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
    }
    None
}

#[cfg(windows)]
fn hide_console(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn hide_console(_command: &mut Command) {}

// ---------------------------------------------------------------------------
// Runs
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct SendRequest {
    message: String,
    backend: Backend,
    /// The CLI session to continue; empty for a new conversation.
    session_id: Option<String>,
    /// What the user has open in Aurora, so "these albums" makes sense.
    context: Option<String>,
    /// The user's local date, `YYYY-MM-DD`.
    today: String,
    /// The user's UTC offset in minutes, for listening reports.
    utc_offset_minutes: i32,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub(crate) enum AssistantEvent {
    Session {
        session_id: String,
    },
    /// Starts a new paragraph of the answer, optionally with its first text.
    TextBlock {
        text: String,
    },
    /// Appends to the current paragraph.
    TextDelta {
        text: String,
    },
    Tool {
        id: String,
        name: String,
        status: ToolStatus,
    },
    Done {
        session_id: Option<String>,
        cost_usd: Option<f64>,
    },
    Error {
        message: String,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ToolStatus {
    Running,
    Done,
    Failed,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct EventEnvelope {
    run_id: String,
    #[serde(flatten)]
    event: AssistantEvent,
}

/// Running CLI processes, so a run can be stopped.
#[derive(Default)]
pub(crate) struct AssistantRuns {
    children: Mutex<HashMap<String, Arc<Mutex<Child>>>>,
    cancelled: Mutex<std::collections::HashSet<String>>,
}

impl AssistantRuns {
    pub(crate) fn cancel(&self, run_id: &str) {
        let child = self
            .children
            .lock()
            .ok()
            .and_then(|children| children.get(run_id).cloned());
        if let Some(child) = child {
            if let Ok(mut cancelled) = self.cancelled.lock() {
                cancelled.insert(run_id.to_owned());
            }
            if let Ok(mut child) = child.lock() {
                let _ = child.kill();
            }
        }
    }

    fn insert(&self, run_id: &str, child: Arc<Mutex<Child>>) {
        if let Ok(mut children) = self.children.lock() {
            children.insert(run_id.to_owned(), child);
        }
    }

    /// Forgets a finished run and reports whether the user stopped it.
    fn finish(&self, run_id: &str) -> bool {
        if let Ok(mut children) = self.children.lock() {
            children.remove(run_id);
        }
        self.cancelled
            .lock()
            .map(|mut cancelled| cancelled.remove(run_id))
            .unwrap_or(false)
    }
}

fn system_prompt(request: &SendRequest) -> String {
    let mut prompt = format!(
        "You are the assistant inside Aurora, a desktop app for exploring the user's own music library \
         (about a million MP3s catalogued by the companion app Music Library). Answer with facts from the \
         aurora tools, never from memory about what the library might contain. General music knowledge is \
         fine for context, but say when something is not in the library.\n\
         Today is {today}. The user's UTC offset is {offset} minutes: pass utcOffsetMinutes={offset} to \
         listening_report and resolve dates such as \"last month\" from today.\n\
         Your tools are read-only. If the user asks you to change tags, ratings, files, or playlists, say \
         that the assistant cannot make changes yet, and where it helps, give an Aurora search query they \
         can paste into the Songs or Albums search box (field terms joined with AND).\n\
         Be concise. Use short Markdown lists or tables. Name albums as Artist – Album (Year).",
        today = request.today,
        offset = request.utc_offset_minutes,
    );
    if let Some(context) = request
        .context
        .as_deref()
        .map(str::trim)
        .filter(|context| !context.is_empty())
    {
        let context: String = context.chars().take(MAX_CONTEXT_CHARS).collect();
        prompt.push_str("\nWhat the user has open in Aurora right now: ");
        prompt.push_str(&context);
    }
    prompt
}

/// Starts a run in the background and returns its id. Events follow on
/// `assistant://event`, ending with exactly one `done` or `error`.
pub(crate) fn send(
    app: AppHandle,
    runs: Arc<AssistantRuns>,
    directory: &Path,
    request: SendRequest,
) -> Result<String, String> {
    let settings = read_settings(directory);
    if !settings.library_access {
        return Err(
            "Turn on \"Let AI assistants read my library\" in Settings → AI assistant first."
                .to_owned(),
        );
    }
    let message = request.message.trim();
    if message.is_empty() {
        return Err("Type a question first.".to_owned());
    }
    if message.chars().count() > MAX_PROMPT_CHARS {
        return Err(format!(
            "Keep the question under {MAX_PROMPT_CHARS} characters."
        ));
    }
    if let Some(session) = request.session_id.as_deref()
        && !valid_session_id(session)
    {
        return Err("The conversation id is invalid. Start a new conversation.".to_owned());
    }
    let executable = resolve_executable(directory, request.backend)?;
    let workspace = directory.join("assistant").join("workspace");
    fs::create_dir_all(&workspace)
        .map_err(|error| format!("Could not prepare the assistant's folder: {error}"))?;
    let mcp = mcp_command();
    let (mut command, stdin_text) = match request.backend {
        Backend::ClaudeCode => {
            let config = directory.join("assistant").join("aurora-mcp.json");
            let json = json!({
                "mcpServers": { "aurora": { "type": "stdio", "command": mcp[0], "args": [&mcp[1]] } }
            });
            fs::write(&config, json.to_string())
                .map_err(|error| format!("Could not write the MCP configuration: {error}"))?;
            (
                claude_command(&executable, &config, &settings.claude_model, &request),
                message.to_owned(),
            )
        }
        Backend::Codex => {
            let text = if request.session_id.is_some() {
                message.to_owned()
            } else {
                format!(
                    "<aurora_instructions>\n{}\n</aurora_instructions>\n\n{message}",
                    system_prompt(&request)
                )
            };
            (
                codex_command(
                    &executable,
                    &workspace,
                    &mcp,
                    &settings.codex_model,
                    &request,
                ),
                text,
            )
        }
    };
    command
        .current_dir(&workspace)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    hide_console(&mut command);
    let mut child = command
        .spawn()
        .map_err(|error| format!("Could not start {}: {error}", request.backend.label()))?;
    let stdout = child
        .stdout
        .take()
        .ok_or("The assistant's output is unavailable.")?;
    let stderr = child.stderr.take();
    if let Some(mut stdin) = child.stdin.take() {
        // Written off-thread so a CLI that fills its output pipe before
        // reading the prompt cannot block. Dropping stdin sends EOF. A closed
        // pipe means the CLI exited early; its error arrives on stderr.
        std::thread::spawn(move || {
            let _ = stdin.write_all(stdin_text.as_bytes());
        });
    }
    let run_id = uuid::Uuid::new_v4().to_string();
    let child = Arc::new(Mutex::new(child));
    runs.insert(&run_id, child.clone());
    let backend = request.backend;
    let thread_run_id = run_id.clone();
    std::thread::Builder::new()
        .name("aurora-assistant-run".to_owned())
        .spawn(move || {
            let emit = |event: AssistantEvent| {
                let _ = app.emit(
                    EVENT,
                    EventEnvelope {
                        run_id: thread_run_id.clone(),
                        event,
                    },
                );
            };
            let failure = run_to_completion(backend, stdout, stderr, &child, &emit);
            let cancelled = runs.finish(&thread_run_id);
            if let Some(message) = failure {
                emit(AssistantEvent::Error {
                    message: if cancelled {
                        "Stopped.".to_owned()
                    } else {
                        message
                    },
                });
            }
        })
        .map_err(|error| format!("Could not start the assistant worker: {error}"))?;
    Ok(run_id)
}

/// Model ids are passed as CLI arguments, so they must not look like flags.
fn valid_model_id(value: &str) -> bool {
    value.len() <= 100
        && value.starts_with(|character: char| character.is_ascii_alphanumeric())
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '.' | '_' | '[' | ']')
        })
}

/// Session ids are passed as CLI arguments, so they must not look like flags.
fn valid_session_id(value: &str) -> bool {
    value.len() <= 128
        && value.starts_with(|character: char| character.is_ascii_alphanumeric())
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || character == '-')
}

fn claude_command(executable: &Path, config: &Path, model: &str, request: &SendRequest) -> Command {
    let mut command = Command::new(executable);
    command.args([
        "-p",
        "--output-format",
        "stream-json",
        "--verbose",
        "--include-partial-messages",
        "--mcp-config",
    ]);
    command.arg(config);
    command.args([
        "--strict-mcp-config",
        // No built-in tools: no files, shell, or web.
        "--tools",
        "",
        "--allowedTools",
        "mcp__aurora",
        // Anything not allowed is refused instead of waiting for a prompt.
        "--permission-mode",
        "dontAsk",
        // No user or project settings, hooks, or plugins.
        "--setting-sources",
        "",
        "--system-prompt",
    ]);
    command.arg(system_prompt(request));
    if !model.is_empty() {
        command.args(["--model", model]);
    }
    if let Some(session) = &request.session_id {
        command.args(["--resume", session]);
    }
    command
}

fn codex_command(
    executable: &Path,
    workspace: &Path,
    mcp: &[String],
    model: &str,
    request: &SendRequest,
) -> Command {
    let mut command = Command::new(executable);
    command.args([
        "exec",
        "--json",
        "--skip-git-repo-check",
        "--ignore-user-config",
        "--sandbox",
        "read-only",
        "-C",
    ]);
    command.arg(workspace);
    let mcp_command = format!("mcp_servers.aurora.command={}", toml_string(&mcp[0]));
    let mcp_args = format!("mcp_servers.aurora.args=[{}]", toml_string(&mcp[1]));
    for override_value in [
        mcp_command.as_str(),
        mcp_args.as_str(),
        "approval_policy=\"never\"",
        "web_search=\"disabled\"",
        "features.shell_tool=false",
        "features.unified_exec=false",
        "features.apps=false",
        "features.plugins=false",
        "features.multi_agent=false",
        "features.multi_agent_v2=false",
        "features.view_image=false",
    ] {
        command.args(["-c", override_value]);
    }
    if !model.is_empty() {
        command.args(["-m", model]);
    }
    if let Some(session) = &request.session_id {
        command.args(["resume", session]);
    }
    command.arg("-");
    command
}

fn toml_string(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"\"".to_owned())
}

/// Streams one run's events. Returns an error message when the CLI ended
/// without a final `done` or `error` event.
fn run_to_completion(
    backend: Backend,
    stdout: impl Read,
    stderr: Option<impl Read + Send + 'static>,
    child: &Arc<Mutex<Child>>,
    emit: &dyn Fn(AssistantEvent),
) -> Option<String> {
    let stderr_tail = Arc::new(Mutex::new(Vec::<u8>::new()));
    let stderr_reader = stderr.map(|mut stderr| {
        let tail = stderr_tail.clone();
        std::thread::spawn(move || {
            let mut buffer = [0_u8; 4096];
            while let Ok(read) = stderr.read(&mut buffer) {
                if read == 0 {
                    break;
                }
                if let Ok(mut tail) = tail.lock() {
                    tail.extend_from_slice(&buffer[..read]);
                    let excess = tail.len().saturating_sub(STDERR_TAIL_BYTES);
                    tail.drain(..excess);
                }
            }
        })
    });
    let watchdog_child = child.clone();
    let finished = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let watchdog_finished = finished.clone();
    std::thread::spawn(move || {
        let started = Instant::now();
        while started.elapsed() < RUN_TIMEOUT {
            if watchdog_finished.load(std::sync::atomic::Ordering::SeqCst) {
                return;
            }
            std::thread::sleep(Duration::from_millis(500));
        }
        if let Ok(mut child) = watchdog_child.lock() {
            let _ = child.kill();
        }
    });

    let mut parser = StreamParser::new(backend);
    for line in BufReader::new(stdout).lines() {
        let Ok(line) = line else {
            break;
        };
        for event in parser.parse_line(&line) {
            emit(event);
        }
        if parser.finished {
            break;
        }
    }
    finished.store(true, std::sync::atomic::Ordering::SeqCst);
    let exit = child.lock().ok().and_then(|mut child| child.wait().ok());
    if let Some(reader) = stderr_reader {
        let _ = reader.join();
    }
    if parser.finished {
        return None;
    }
    let tail = stderr_tail
        .lock()
        .map(|tail| String::from_utf8_lossy(&tail).into_owned())
        .unwrap_or_default();
    let detail = tail
        .lines()
        .rev()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or_default()
        .to_owned();
    let summary = match exit {
        Some(status) if status.success() => "finished without an answer".to_owned(),
        Some(status) => format!("stopped ({status})"),
        None => "stopped".to_owned(),
    };
    Some(
        format!("{} {summary}. {detail}", backend.label())
            .trim()
            .to_owned(),
    )
}

/// Turns one CLI's JSON lines into Aurora events. Unknown events are ignored,
/// because neither CLI promises a stable stream format.
struct StreamParser {
    backend: Backend,
    session_id: Option<String>,
    /// Claude: whether the current message streamed text as deltas.
    streamed_text: bool,
    /// Claude: tool_use ids to tool names.
    tool_names: HashMap<String, String>,
    finished: bool,
}

impl StreamParser {
    fn new(backend: Backend) -> Self {
        Self {
            backend,
            session_id: None,
            streamed_text: false,
            tool_names: HashMap::new(),
            finished: false,
        }
    }

    fn parse_line(&mut self, line: &str) -> Vec<AssistantEvent> {
        let Ok(event) = serde_json::from_str::<Value>(line.trim()) else {
            return Vec::new();
        };
        match self.backend {
            Backend::ClaudeCode => self.parse_claude(&event),
            Backend::Codex => self.parse_codex(&event),
        }
    }

    fn parse_claude(&mut self, event: &Value) -> Vec<AssistantEvent> {
        let text =
            |value: &Value, key: &str| value.get(key).and_then(Value::as_str).map(str::to_owned);
        match event.get("type").and_then(Value::as_str) {
            Some("system") if event.get("subtype").and_then(Value::as_str) == Some("init") => {
                let mut events = Vec::new();
                if let Some(session) = text(event, "session_id") {
                    self.session_id = Some(session.clone());
                    events.push(AssistantEvent::Session {
                        session_id: session,
                    });
                }
                let aurora_connected = event
                    .get("mcp_servers")
                    .and_then(Value::as_array)
                    .is_some_and(|servers| {
                        servers.iter().any(|server| {
                            server.get("name").and_then(Value::as_str) == Some("aurora")
                                && server.get("status").and_then(Value::as_str) == Some("connected")
                        })
                    });
                if !aurora_connected {
                    events.push(AssistantEvent::TextBlock {
                        text: "⚠ Aurora's library tools did not connect, so this answer cannot use your library.".to_owned(),
                    });
                }
                events
            }
            Some("stream_event") => {
                let inner = event.get("event").unwrap_or(&Value::Null);
                match inner.get("type").and_then(Value::as_str) {
                    Some("message_start") => {
                        self.streamed_text = false;
                        Vec::new()
                    }
                    Some("content_block_start") => {
                        let block = inner.get("content_block").unwrap_or(&Value::Null);
                        match block.get("type").and_then(Value::as_str) {
                            Some("text") => {
                                self.streamed_text = true;
                                vec![AssistantEvent::TextBlock {
                                    text: text(block, "text").unwrap_or_default(),
                                }]
                            }
                            Some("tool_use") => {
                                let id = text(block, "id").unwrap_or_default();
                                let name = tool_label(&text(block, "name").unwrap_or_default());
                                self.tool_names.insert(id.clone(), name.clone());
                                vec![AssistantEvent::Tool {
                                    id,
                                    name,
                                    status: ToolStatus::Running,
                                }]
                            }
                            _ => Vec::new(),
                        }
                    }
                    Some("content_block_delta") => {
                        let delta = inner.get("delta").unwrap_or(&Value::Null);
                        match (
                            delta.get("type").and_then(Value::as_str),
                            text(delta, "text"),
                        ) {
                            (Some("text_delta"), Some(text)) if !text.is_empty() => {
                                vec![AssistantEvent::TextDelta { text }]
                            }
                            _ => Vec::new(),
                        }
                    }
                    _ => Vec::new(),
                }
            }
            // Complete messages: used only when the CLI did not stream them.
            Some("assistant") => {
                let mut events = Vec::new();
                for block in content_blocks(event) {
                    match block.get("type").and_then(Value::as_str) {
                        Some("text") if !self.streamed_text => {
                            if let Some(text) = text(block, "text") {
                                events.push(AssistantEvent::TextBlock { text });
                            }
                        }
                        Some("tool_use") => {
                            let id = text(block, "id").unwrap_or_default();
                            if !self.tool_names.contains_key(&id) {
                                let name = tool_label(&text(block, "name").unwrap_or_default());
                                self.tool_names.insert(id.clone(), name.clone());
                                events.push(AssistantEvent::Tool {
                                    id,
                                    name,
                                    status: ToolStatus::Running,
                                });
                            }
                        }
                        _ => {}
                    }
                }
                events
            }
            Some("user") => content_blocks(event)
                .filter(|block| block.get("type").and_then(Value::as_str) == Some("tool_result"))
                .map(|block| {
                    let id = text(block, "tool_use_id").unwrap_or_default();
                    let failed = block.get("is_error").and_then(Value::as_bool) == Some(true);
                    AssistantEvent::Tool {
                        name: self.tool_names.get(&id).cloned().unwrap_or_default(),
                        id,
                        status: if failed {
                            ToolStatus::Failed
                        } else {
                            ToolStatus::Done
                        },
                    }
                })
                .collect(),
            Some("result") => {
                self.finished = true;
                let session_id = text(event, "session_id").or_else(|| self.session_id.clone());
                if event.get("is_error").and_then(Value::as_bool) == Some(true)
                    || event.get("subtype").and_then(Value::as_str) != Some("success")
                {
                    let detail = text(event, "result")
                        .or_else(|| {
                            event
                                .get("errors")
                                .and_then(Value::as_array)
                                .and_then(|errors| errors.first())
                                .and_then(Value::as_str)
                                .map(str::to_owned)
                        })
                        .unwrap_or_else(|| "Claude Code reported an error.".to_owned());
                    return vec![AssistantEvent::Error { message: detail }];
                }
                vec![AssistantEvent::Done {
                    session_id,
                    cost_usd: event.get("total_cost_usd").and_then(Value::as_f64),
                }]
            }
            _ => Vec::new(),
        }
    }

    fn parse_codex(&mut self, event: &Value) -> Vec<AssistantEvent> {
        let text =
            |value: &Value, key: &str| value.get(key).and_then(Value::as_str).map(str::to_owned);
        match event.get("type").and_then(Value::as_str) {
            Some("thread.started") => match text(event, "thread_id") {
                Some(session) => {
                    self.session_id = Some(session.clone());
                    vec![AssistantEvent::Session {
                        session_id: session,
                    }]
                }
                None => Vec::new(),
            },
            Some(kind @ ("item.started" | "item.completed")) => {
                let item = event.get("item").unwrap_or(&Value::Null);
                let completed = kind == "item.completed";
                match item.get("type").and_then(Value::as_str) {
                    Some("agent_message") if completed => match text(item, "text") {
                        Some(text) if !text.trim().is_empty() => {
                            vec![AssistantEvent::TextBlock { text }]
                        }
                        _ => Vec::new(),
                    },
                    Some("mcp_tool_call") => {
                        let failed = item.get("error").is_some_and(|error| !error.is_null())
                            || item.get("status").and_then(Value::as_str) == Some("failed");
                        vec![AssistantEvent::Tool {
                            id: text(item, "id").unwrap_or_default(),
                            name: text(item, "tool").unwrap_or_default(),
                            status: match (completed, failed) {
                                (false, _) => ToolStatus::Running,
                                (true, false) => ToolStatus::Done,
                                (true, true) => ToolStatus::Failed,
                            },
                        }]
                    }
                    // `error` items are configuration warnings, not failures.
                    _ => Vec::new(),
                }
            }
            Some("turn.completed") => {
                self.finished = true;
                vec![AssistantEvent::Done {
                    session_id: self.session_id.clone(),
                    cost_usd: None,
                }]
            }
            Some("turn.failed") => {
                self.finished = true;
                let message = event
                    .get("error")
                    .and_then(|error| text(error, "message"))
                    .unwrap_or_else(|| "Codex could not finish the answer.".to_owned());
                vec![AssistantEvent::Error { message }]
            }
            Some("error") => {
                self.finished = true;
                vec![AssistantEvent::Error {
                    message: text(event, "message")
                        .unwrap_or_else(|| "Codex reported an error.".to_owned()),
                }]
            }
            _ => Vec::new(),
        }
    }
}

fn content_blocks(event: &Value) -> impl Iterator<Item = &Value> {
    event
        .get("message")
        .and_then(|message| message.get("content"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
}

/// `mcp__aurora__search_albums` → `search_albums`.
fn tool_label(name: &str) -> String {
    name.strip_prefix("mcp__aurora__")
        .unwrap_or(name)
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(backend: Backend, lines: &[&str]) -> (Vec<AssistantEvent>, bool) {
        let mut parser = StreamParser::new(backend);
        let events = lines
            .iter()
            .flat_map(|line| parser.parse_line(line))
            .collect();
        (events, parser.finished)
    }

    #[test]
    fn claude_stream_becomes_session_tools_text_and_done() {
        let (events, finished) = parse(
            Backend::ClaudeCode,
            &[
                r#"{"type":"system","subtype":"init","session_id":"s-1","mcp_servers":[{"name":"aurora","status":"connected"}]}"#,
                r#"{"type":"stream_event","event":{"type":"message_start"}}"#,
                r#"{"type":"stream_event","event":{"type":"content_block_start","content_block":{"type":"tool_use","id":"t1","name":"mcp__aurora__list_genres","input":{}}}}"#,
                r#"{"type":"assistant","message":{"content":[{"type":"tool_use","id":"t1","name":"mcp__aurora__list_genres","input":{}}]}}"#,
                r#"{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"t1","content":[]}]}}"#,
                r#"{"type":"stream_event","event":{"type":"message_start"}}"#,
                r#"{"type":"stream_event","event":{"type":"content_block_start","content_block":{"type":"text","text":""}}}"#,
                r#"{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"text_delta","text":"TV leads."}}}"#,
                r#"{"type":"assistant","message":{"content":[{"type":"text","text":"TV leads."}]}}"#,
                r#"{"type":"some_future_event","x":1}"#,
                r#"{"type":"result","subtype":"success","is_error":false,"session_id":"s-1","total_cost_usd":0.02}"#,
            ],
        );
        assert!(finished);
        assert_eq!(
            events,
            vec![
                AssistantEvent::Session {
                    session_id: "s-1".into()
                },
                AssistantEvent::Tool {
                    id: "t1".into(),
                    name: "list_genres".into(),
                    status: ToolStatus::Running
                },
                AssistantEvent::Tool {
                    id: "t1".into(),
                    name: "list_genres".into(),
                    status: ToolStatus::Done
                },
                AssistantEvent::TextBlock {
                    text: String::new()
                },
                AssistantEvent::TextDelta {
                    text: "TV leads.".into()
                },
                AssistantEvent::Done {
                    session_id: Some("s-1".into()),
                    cost_usd: Some(0.02)
                },
            ]
        );
    }

    #[test]
    fn claude_text_without_deltas_is_still_shown_and_errors_end_the_run() {
        let (events, finished) = parse(
            Backend::ClaudeCode,
            &[
                r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Hello"}]}}"#,
                r#"{"type":"result","subtype":"error_during_execution","is_error":true,"errors":["Not logged in"]}"#,
            ],
        );
        assert!(finished);
        assert_eq!(
            events,
            vec![
                AssistantEvent::TextBlock {
                    text: "Hello".into()
                },
                AssistantEvent::Error {
                    message: "Not logged in".into()
                },
            ]
        );
    }

    #[test]
    fn codex_stream_ignores_config_warnings() {
        let (events, finished) = parse(
            Backend::Codex,
            &[
                r#"{"type":"thread.started","thread_id":"th-1"}"#,
                r#"{"type":"item.completed","item":{"id":"item_0","type":"error","message":"deprecated feature"}}"#,
                r#"{"type":"item.started","item":{"id":"item_1","type":"mcp_tool_call","server":"aurora","tool":"library_overview","status":"in_progress","error":null}}"#,
                r#"{"type":"item.completed","item":{"id":"item_1","type":"mcp_tool_call","server":"aurora","tool":"library_overview","status":"completed","error":null}}"#,
                r#"{"type":"item.completed","item":{"id":"item_2","type":"agent_message","text":"You have 5,921 loved tracks."}}"#,
                r#"{"type":"turn.completed","usage":{}}"#,
            ],
        );
        assert!(finished);
        assert_eq!(
            events,
            vec![
                AssistantEvent::Session {
                    session_id: "th-1".into()
                },
                AssistantEvent::Tool {
                    id: "item_1".into(),
                    name: "library_overview".into(),
                    status: ToolStatus::Running
                },
                AssistantEvent::Tool {
                    id: "item_1".into(),
                    name: "library_overview".into(),
                    status: ToolStatus::Done
                },
                AssistantEvent::TextBlock {
                    text: "You have 5,921 loved tracks.".into()
                },
                AssistantEvent::Done {
                    session_id: Some("th-1".into()),
                    cost_usd: None
                },
            ]
        );
    }

    #[test]
    fn settings_default_to_no_library_access() {
        let directory = tempfile::tempdir().unwrap();
        assert!(!library_access_enabled(directory.path()));
        let mut settings = read_settings(directory.path());
        settings.library_access = true;
        write_settings(directory.path(), &settings).unwrap();
        assert!(library_access_enabled(directory.path()));
    }

    #[test]
    fn models_default_to_haiku_and_luna_and_must_not_look_like_flags() {
        let directory = tempfile::tempdir().unwrap();
        // A settings file from 0.62.0 has no model fields.
        fs::write(
            directory.path().join(SETTINGS_FILE),
            r#"{"libraryAccess":true}"#,
        )
        .unwrap();
        let settings = read_settings(directory.path());
        assert_eq!(settings.claude_model, DEFAULT_CLAUDE_MODEL);
        assert_eq!(settings.codex_model, DEFAULT_CODEX_MODEL);
        assert!(valid_model_id("claude-sonnet-5-5[1m]"));
        assert!(valid_model_id("gpt-6.1-sol"));
        assert!(!valid_model_id("--dangerously-skip-permissions"));
        assert!(!valid_model_id("haiku; rm"));
        let rejected = AssistantSettings {
            codex_model: "-c x".to_owned(),
            ..AssistantSettings::default()
        };
        assert!(update_settings(directory.path(), rejected).is_err());
    }

    #[test]
    fn session_ids_are_restricted() {
        assert!(valid_session_id("9220ae13-101a-4a2e-8271-9be5489186fc"));
        assert!(!valid_session_id("--dangerously-skip-permissions"));
        assert!(!valid_session_id("a b"));
    }
}
