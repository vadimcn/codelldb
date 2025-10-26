#![allow(non_camel_case_types)]

mod json_map;

use std::path::PathBuf;

pub use crate::json_map::JsonMap;
use schemars::JsonSchema;
use serde_derive::*;

#[derive(Serialize, Deserialize, JsonSchema, Debug, Copy, Clone)]
#[serde(untagged)]
pub enum Either<T1, T2> {
    First(T1),
    Second(T2),
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Copy, Clone)]
#[serde(rename_all = "camelCase")]
pub enum BreakpointMode {
    Path,
    File,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Copy, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum ConsoleMode {
    /// Treat debug console input as debugger commands.  In order to evaluate an expression, prefix it with '?' (question mark).
    Commands,
    /// Treat debug console input as expressions.  In order to execute a debugger command, prefix it with '`' (backtick).
    Evaluate,
    /// (experimental) Use the debug console for warningevaluation of expressions, open a separate terminal for input of LLDB commands.
    Split,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Copy, Clone)]
#[serde(rename_all = "camelCase")]
pub enum DisplayFormat {
    Auto,
    Hex,
    Decimal,
    Binary,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Copy, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum ShowDisassembly {
    /// Only when source is not available.
    Always,
    /// Never show.
    Never,
    /// Always show, even if source is available.
    Auto,
}

/// Terminal device identifier
#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone)]
#[serde(rename_all = "camelCase", untagged)]
pub enum TerminalId {
    /// TTY device name (Unix)
    TTY(String),
    /// Process ID (Windows)
    PID(u64),
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub enum TerminalKind {
    /// Use integrated terminal in VSCode.
    Integrated,
    /// Use external terminal window.
    External,
    /// Use VScode Debug Console for stdout and stderr. Stdin will be unavailable.
    Console,
    /// Use the specified TTY device
    #[serde(untagged)]
    TerminalId(TerminalId),
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Copy, Clone)]
#[serde(rename_all = "camelCase")]
pub enum ConsoleKind {
    /// Use integrated terminal in VSCode.
    IntegratedTerminal,
    /// Use external terminal window.
    ExternalTerminal,
    /// Use VScode Debug Console for stdout and stderr. Stdin will be unavailable.
    InternalConsole,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Copy, Clone)]
#[serde(rename_all = "camelCase")]
pub enum Expressions {
    Simple,
    Python,
    Native,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone)]
#[serde(rename_all = "camelCase")]
#[schemars(deny_unknown_fields)]
pub struct ExcludeCallerRequest {
    pub thread_id: i64,
    pub frame_index: u32,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone)]
#[serde(rename_all = "camelCase")]
#[schemars(deny_unknown_fields)]
pub struct ExcludeCallerResponse {
    pub exclusion: ExcludedCaller,
    // Display name for the UI
    pub label: String,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone)]
#[serde(rename_all = "camelCase")]
#[schemars(deny_unknown_fields)]
pub struct SetExcludedCallersRequest {
    pub exclusions: Vec<ExcludedCaller>,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone)]
#[serde(rename_all = "camelCase")]
#[schemars(deny_unknown_fields)]
pub struct ExcludedCaller {
    // Breakpiont id (number) of exception id (string) for which exclusion is being created.
    pub site_id: Either<i64, String>,
    // Symbol of the caller which is being excluded.
    pub symbol: String,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone)]
#[schemars(deny_unknown_fields)]
pub struct Source {
    pub path: String,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone)]
#[serde(rename_all = "camelCase")]
#[schemars(deny_unknown_fields)]
pub struct Symbol {
    pub name: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub address: String,
    pub location: Option<(Source, u32)>,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone)]
#[serde(rename_all = "camelCase")]
#[schemars(deny_unknown_fields)]
pub struct SymbolsRequest {
    pub filter: String,
    pub max_results: u32,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone)]
#[serde(rename_all = "camelCase")]
#[schemars(deny_unknown_fields)]
pub struct SymbolsResponse {
    pub symbols: Vec<Symbol>,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
#[schemars(deny_unknown_fields)]
pub struct AdapterSettings {
    pub display_format: Option<DisplayFormat>,
    pub show_disassembly: Option<ShowDisassembly>,
    pub dereference_pointers: Option<bool>,
    pub container_summary: Option<bool>,
    pub evaluation_timeout: Option<f32>,
    pub summary_timeout: Option<f32>,
    pub suppress_missing_source_files: Option<bool>,
    pub console_mode: Option<ConsoleMode>,
    pub source_languages: Option<Vec<String>>,
    pub script_config: Option<serde_json::Map<String, serde_json::Value>>,
    pub evaluate_for_hovers: Option<bool>,
    pub command_completions: Option<bool>,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone)]
#[serde(rename_all = "camelCase")]
#[schemars(deny_unknown_fields)]
pub struct CommonLaunchFields {
    pub name: Option<String>,
    /// Source path remapping between the build machine and the local machine.  Each item is a pair of remote and local path prefixes.
    pub source_map: Option<JsonMap<Option<String>>>,
    /// The default evaluator type used for expressions
    pub expressions: Option<Expressions>,
    /// Initialization commands executed upon debugger startup.  Note that the target is not yet created at this point;
    /// if you need to perform an action related to the specific debugging target, prefer using `preRunCommands`.
    pub init_commands: Option<Vec<String>>,
    /// Commands executed just before the debuggee is launched or attached to
    pub pre_run_commands: Option<Vec<String>>,
    /// Commands executed just after the debuggee has been launched or attached to
    pub post_run_commands: Option<Vec<String>>,
    /// Commands executed just before the debuggee is terminated or disconnected from
    pub pre_terminate_commands: Option<Vec<String>>,
    /// Commands executed at the end of debugging session, after the debuggee has been terminated
    pub exit_commands: Option<Vec<String>>,
    /// A list of source languages to enable language-specific features for
    pub source_languages: Option<Vec<String>>,
    /// Enable reverse debugging (Requires reverse execution support in the debug server, see User's Manual for details).
    pub reverse_debugging: Option<bool>,
    /// Base directory used for resolution of relative source paths.  Defaults to \"${workspaceFolder}\".
    pub relative_path_base: Option<String>,
    /// Specifies how source breakpoints should be set
    pub breakpoint_mode: Option<BreakpointMode>,
    #[serde(rename = "_adapterSettings")]
    #[schemars(skip)]
    pub adapter_settings: Option<AdapterSettings>,
}
#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone)]
#[serde(rename_all = "camelCase")]
#[schemars(deny_unknown_fields)]
pub struct LaunchRequestArguments {
    #[serde(flatten)]
    pub common: CommonLaunchFields,
    pub no_debug: Option<bool>,
    /// Path to the program to debug
    pub program: Option<String>,
    /// Program arguments
    pub args: Option<Vec<String>>,
    /// Program working directory
    pub cwd: Option<String>,
    /// Additional environment variables
    pub env: Option<JsonMap<String>>,
    /// File to read the environment variables from
    pub env_file: Option<String>,
    /// Destination for stdio streams: null = send to the debugger console or the terminal, "<path>" = attach to a file/tty/fifo
    pub stdio: Option<Either<String, Vec<Option<String>>>>,
    /// Automatically stop debuggee after launch
    pub stop_on_entry: Option<bool>,
    /// Terminal type to use
    pub terminal: Option<TerminalKind>,
    /// Terminal type to use. (This setting is a compatibility alias of 'terminal'.)
    pub console: Option<ConsoleKind>,
    /// Commands that create the debug target
    pub target_create_commands: Option<Vec<String>>,
    /// Commands that create the debuggee process
    pub process_create_commands: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone)]
#[serde(rename_all = "camelCase")]
#[schemars(deny_unknown_fields)]
pub struct AttachRequestArguments {
    #[serde(flatten)]
    pub common: CommonLaunchFields,
    /// Path to the program to attach to
    pub program: Option<String>,
    /// Process id to attach to
    pub pid: Option<Either<u64, String>>,
    /// Wait for the process to launch (MacOS only
    pub wait_for: Option<bool>,
    /// Automatically stop debuggee after attach
    pub stop_on_entry: Option<bool>,
    /// Commands that create the debug target
    pub target_create_commands: Option<Vec<String>>,
    /// Commands that create the debuggee process
    pub process_create_commands: Option<Vec<String>>,
}

/// Launch environment provided by codelldb-launch
#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone)]
#[serde(tag = "type", rename_all = "camelCase")]
#[schemars(deny_unknown_fields)]
pub struct LaunchEnvironment {
    /// Command line to launch the debuggee.
    pub cmd: Vec<String>,
    /// Working directory.
    pub cwd: PathBuf,
    /// Environment variables present when codelldb-launch was invoked.
    pub env: JsonMap<String>,
    /// Terminal device identifier
    pub terminal_id: Option<TerminalId>,
    /// Debug configuration
    pub config: Option<String>,
}

/// Response to LaunchEnvironment request
#[derive(Serialize, Deserialize, JsonSchema, Debug, Clone)]
#[serde(rename_all = "camelCase")]
#[schemars(deny_unknown_fields)]
pub struct LaunchResponse {
    pub success: bool,
    pub message: Option<String>,
}

#[test]
fn serialization() {
    let kind = serde_json::from_str::<TerminalKind>(r#""integrated""#).unwrap();
    assert!(matches!(kind, TerminalKind::Integrated));

    let kind = serde_json::from_str::<TerminalKind>(r#""/dev/ttyX""#).unwrap();
    assert!(matches!(kind, TerminalKind::TerminalId(TerminalId::TTY(name)) if name == "/dev/ttyX"));

    let kind = serde_json::from_str::<TerminalKind>(r#"42"#).unwrap();
    assert!(matches!(kind, TerminalKind::TerminalId(TerminalId::PID(pid)) if pid == 42));
}
