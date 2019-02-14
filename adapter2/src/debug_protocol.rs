#![allow(non_camel_case_types)]

use serde_derive::*;

use std::collections::HashMap as Map;

pub use raw_debug_protocol::{
    Breakpoint, BreakpointEventBody, CompletionItem, CompletionsArguments, CompletionsResponseBody, ContinueArguments,
    ContinueResponseBody, ContinuedEventBody, DisconnectArguments, EvaluateArguments, EvaluateResponseBody,
    ExitedEventBody, InitializeRequestArguments, ModuleEventBody, NextArguments, OutputEventBody, PauseArguments,
    RunInTerminalRequestArguments, Scope, ScopesArguments, ScopesResponseBody, SetBreakpointsArguments,
    SetBreakpointsResponseBody, SetExceptionBreakpointsArguments, SetFunctionBreakpointsArguments,
    SetVariableArguments, SetVariableResponseBody, Source, SourceArguments, SourceBreakpoint, SourceResponseBody,
    StackFrame, StackTraceArguments, StackTraceResponseBody, StepBackArguments, StepInArguments, StepOutArguments,
    StoppedEventBody, TerminatedEventBody, Thread, ThreadEventBody, ThreadsResponseBody, Variable, VariablesArguments,
    VariablesResponseBody,
};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type")]
pub enum ProtocolMessage {
    #[serde(rename = "request")]
    Request(Request),
    #[serde(rename = "response")]
    Response(Response),
    #[serde(rename = "event")]
    Event(Event),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Request {
    pub seq: u32,
    #[serde(flatten)]
    pub arguments: Option<RequestArguments>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Response {
    pub request_seq: u32,
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show_user: Option<bool>,
    #[serde(flatten)]
    pub body: Option<ResponseBody>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Event {
    pub seq: u32,
    #[serde(flatten)]
    pub body: EventBody,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "command", content = "arguments")]
pub enum RequestArguments {
    initialize(InitializeRequestArguments),
    launch(LaunchRequestArguments),
    attach(AttachRequestArguments),
    setBreakpoints(SetBreakpointsArguments),
    setFunctionBreakpoints(SetFunctionBreakpointsArguments),
    setExceptionBreakpoints(SetExceptionBreakpointsArguments),
    configurationDone,
    pause(PauseArguments),
    #[serde(rename = "continue")]
    continue_(ContinueArguments),
    next(NextArguments),
    stepIn(StepInArguments),
    stepOut(StepOutArguments),
    stepBack(StepBackArguments),
    reverseContinue,
    threads,
    stackTrace(StackTraceArguments),
    scopes(ScopesArguments),
    source(SourceArguments),
    variables(VariablesArguments),
    completions(CompletionsArguments),
    evaluate(EvaluateArguments),
    setVariable(SetVariableArguments),
    disconnect(DisconnectArguments),
    // Custom
    displaySettings(DisplaySettingsArguments),
    // Reverse
    runInTerminal(RunInTerminalRequestArguments),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "command", content = "body")]
pub enum ResponseBody {
    Async,
    initialize(Capabilities),
    launch,
    attach,
    setBreakpoints(SetBreakpointsResponseBody),
    setFunctionBreakpoints(SetBreakpointsResponseBody),
    setExceptionBreakpoints,
    configurationDone,
    pause,
    #[serde(rename = "continue")]
    continue_(ContinueResponseBody),
    next,
    stepIn,
    stepOut,
    stepBack,
    reverseContinue,
    threads(ThreadsResponseBody),
    stackTrace(StackTraceResponseBody),
    scopes(ScopesResponseBody),
    source(SourceResponseBody),
    variables(VariablesResponseBody),
    completions(CompletionsResponseBody),
    evaluate(EvaluateResponseBody),
    setVariable(SetVariableResponseBody),
    disconnect,
    // Custom
    displaySettings,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "event", content = "body")]
pub enum EventBody {
    initialized,
    output(OutputEventBody),
    breakpoint(BreakpointEventBody),
    module(ModuleEventBody),
    thread(ThreadEventBody),
    stopped(StoppedEventBody),
    continued(ContinuedEventBody),
    exited(ExitedEventBody),
    terminated(TerminatedEventBody),
    // Custom
    displayHtml(DisplayHtmlEventBody),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LaunchRequestArguments {
    pub no_debug: Option<bool>,
    pub program: Option<String>,
    pub args: Option<Vec<String>>,
    pub cwd: Option<String>,
    pub env: Option<Map<String, String>>,
    pub stdio: Option<Vec<Option<String>>>,
    pub terminal: Option<TerminalKind>,
    pub stop_on_entry: Option<bool>,
    pub init_commands: Option<Vec<String>>,
    pub target_create_commands: Option<Vec<String>>,
    pub pre_run_commands: Option<Vec<String>>,
    pub process_create_commands: Option<Vec<String>>,
    pub post_run_commands: Option<Vec<String>>,
    pub exit_commands: Option<Vec<String>>,
    pub expressions: Option<Expressions>,
    pub source_map: Option<Map<String, Option<String>>>,
    pub source_languages: Option<Vec<String>>,
    #[serde(rename = "_displaySettings")]
    pub display_settings: Option<DisplaySettingsArguments>,
    pub custom: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AttachRequestArguments {
    pub program: Option<String>,
    pub pid: Option<Pid>,
    pub wait_for: Option<bool>,
    pub stop_on_entry: Option<bool>,
    pub init_commands: Option<Vec<String>>,
    pub pre_run_commands: Option<Vec<String>>,
    pub post_run_commands: Option<Vec<String>>,
    pub exit_commands: Option<Vec<String>>,
    pub expressions: Option<Expressions>,
    pub source_map: Option<Map<String, Option<String>>>,
    pub source_languages: Option<Vec<String>>,
    #[serde(rename = "_displaySettings")]
    pub display_settings: Option<DisplaySettingsArguments>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Capabilities {
    pub supports_configuration_done_request: bool,
    pub supports_function_breakpoints: bool,
    pub supports_conditional_breakpoints: bool,
    pub supports_hit_conditional_breakpoints: bool,
    pub supports_evaluate_for_hovers: bool,
    pub supports_set_variable: bool,
    pub supports_completions_request: bool,
    pub support_terminate_debuggee: bool,
    pub supports_delayed_stack_trace_loading: bool,
    pub supports_log_points: bool,
    pub exception_breakpoint_filters: Vec<ExceptionBreakpointsFilter>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ExceptionBreakpointsFilter {
    pub filter: String,
    pub label: String,
    pub default: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DisplayHtmlEventBody {
    pub html: String,
    pub title: Option<String>,
    pub position: Option<i32>,
    pub reveal: bool,
}

#[derive(Serialize, Deserialize, Debug, Copy, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DisplaySettingsArguments {
    pub display_format: Option<DisplayFormat>,
    pub show_disassembly: Option<ShowDisassembly>,
    pub dereference_pointers: Option<bool>,
    pub container_summary: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug, Copy, Clone)]
#[serde(rename_all = "camelCase")]
pub enum DisplayFormat {
    Auto,
    Hex,
    Decimal,
    Binary,
}

#[derive(Serialize, Deserialize, Debug, Copy, Clone)]
#[serde(rename_all = "camelCase")]
pub enum ShowDisassembly {
    Always,
    Never,
    Auto,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum Pid {
    Number(u32),
    String(String),
}

#[derive(Serialize, Deserialize, Debug, Copy, Clone)]
#[serde(rename_all = "camelCase")]
pub enum TerminalKind {
    Integrated,
    External,
    Console,
}

#[derive(Serialize, Deserialize, Debug, Copy, Clone)]
#[serde(rename_all = "camelCase")]
pub enum Expressions {
    Simple,
    Python,
    Native,
}

////////////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;
    fn parse(s: &[u8]) {
        let _msg = serde_json::from_slice::<ProtocolMessage>(s).unwrap();
    }

    #[test]
    fn test1() {
        parse(br#"{"command":"initialize","arguments":{"clientID":"vscode","clientName":"Visual Studio Code","adapterID":"lldb","pathFormat":"path","linesStartAt1":true,"columnsStartAt1":true,"supportsVariableType":true,"supportsVariablePaging":true,"supportsRunInTerminalRequest":true,"locale":"en-us"},"type":"request","seq":1}"#);
        parse(br#"{"request_seq":1,"command":"initialize","body":{"supportsDelayedStackTraceLoading":true,"supportsEvaluateForHovers":true,"exceptionBreakpointFilters":[{"filter":"rust_panic","default":true,"label":"Rust: on panic"}],"supportsCompletionsRequest":true,"supportsConditionalBreakpoints":true,"supportsStepBack":false,"supportsConfigurationDoneRequest":true,"supportTerminateDebuggee":true,"supportsLogPoints":true,"supportsFunctionBreakpoints":true,"supportsHitConditionalBreakpoints":true,"supportsSetVariable":true},"type":"response","success":true}"#);
    }

    #[test]
    fn test2() {
        parse(br#"{"command":"launch","arguments":{"type":"lldb","request":"launch","name":"Debug tests in types_lib","args":[],"cwd":"/home/chega/NW/vscode-lldb/debuggee","initCommands":["platform shell echo 'init'"],"env":{"TEST":"folder"},"sourceMap":{"/checkout/src":"/home/chega/.rustup/toolchains/nightly-x86_64-unknown-linux-gnu/lib/rustlib/src/rust/src"},"program":"/home/chega/NW/vscode-lldb/debuggee/target/debug/types_lib-d6a67ab7ca515c6b","debugServer":41025,"_displaySettings":{"showDisassembly":"always","displayFormat":"auto","dereferencePointers":true,"toggleContainerSummary":false,"containerSummary":true},"__sessionId":"81865613-a1ee-4a66-b449-a94165625fd2"},"type":"request","seq":2}"#);
        parse(br#"{"request_seq":2,"command":"launch","body":null,"type":"response","success":true}"#);
    }

    #[test]
    fn test3() {
        parse(br#"{"type":"event","event":"initialized","seq":0}"#);
        parse(br#"{"body":{"reason":"started","threadId":7537},"type":"event","event":"thread","seq":0}"#);
    }

    #[test]
    fn test4() {
        parse(br#"{"command":"scopes","arguments":{"frameId":1000},"type":"request","seq":12}"#);
        parse(br#"{"request_seq":12,"command":"scopes","body":{"scopes":[{"variablesReference":1001,"name":"Local","expensive":false},{"variablesReference":1002,"name":"Static","expensive":false},{"variablesReference":1003,"name":"Global","expensive":false},{"variablesReference":1004,"name":"Registers","expensive":false}]},"type":"response","success":true}"#);
    }
}
