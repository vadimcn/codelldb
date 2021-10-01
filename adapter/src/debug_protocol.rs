#![allow(non_camel_case_types)]

use crate::vec_map::VecMap;
use serde_derive::*;

pub use raw_debug_protocol::{
    Breakpoint, BreakpointEventBody, CancelArguments, Capabilities, CapabilitiesEventBody, CompletionItem,
    CompletionsArguments, CompletionsResponseBody, ContinueArguments, ContinueResponseBody, ContinuedEventBody,
    DataBreakpoint, DataBreakpointAccessType, DataBreakpointInfoArguments, DataBreakpointInfoResponseBody,
    DisconnectArguments, EvaluateArguments, EvaluateResponseBody, ExceptionBreakpointsFilter, ExitedEventBody,
    GotoArguments, GotoTarget, GotoTargetsArguments, GotoTargetsResponseBody, InitializeRequestArguments,
    InvalidatedAreas, InvalidatedEventBody, Module, ModuleEventBody, NextArguments, OutputEventBody, PauseArguments,
    ReadMemoryArguments, ReadMemoryResponseBody, RestartFrameArguments, ReverseContinueArguments,
    RunInTerminalRequestArguments, RunInTerminalResponseBody, Scope, ScopesArguments, ScopesResponseBody,
    SetBreakpointsArguments, SetBreakpointsResponseBody, SetDataBreakpointsArguments, SetDataBreakpointsResponseBody,
    SetExceptionBreakpointsArguments, SetFunctionBreakpointsArguments, SetVariableArguments, SetVariableResponseBody,
    Source, SourceArguments, SourceBreakpoint, SourceResponseBody, StackFrame, StackTraceArguments,
    StackTraceResponseBody, StepBackArguments, StepInArguments, StepOutArguments, StoppedEventBody, TerminateArguments,
    TerminatedEventBody, Thread, ThreadEventBody, ThreadsResponseBody, Variable, VariablesArguments,
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
    pub command: Command,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Response {
    pub request_seq: u32,
    pub success: bool,
    #[serde(flatten)]
    pub result: ResponseResult,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum ResponseResult {
    Success {
        #[serde(flatten)]
        body: ResponseBody,
    },
    Error {
        command: String,
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        show_user: Option<bool>,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Event {
    pub seq: u32,
    #[serde(flatten)]
    pub body: EventBody,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum Command {
    Known(RequestArguments),
    Unknown {
        command: String,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "command", content = "arguments")]
pub enum RequestArguments {
    initialize(InitializeRequestArguments),
    cancel(CancelArguments),
    launch(Either<LaunchRequestArguments, serde_json::Value>),
    attach(Either<AttachRequestArguments, serde_json::Value>),
    setBreakpoints(SetBreakpointsArguments),
    setFunctionBreakpoints(SetFunctionBreakpointsArguments),
    setExceptionBreakpoints(SetExceptionBreakpointsArguments),
    configurationDone(Option<NoArguments>),
    pause(PauseArguments),
    #[serde(rename = "continue")]
    continue_(ContinueArguments),
    next(NextArguments),
    stepIn(StepInArguments),
    stepOut(StepOutArguments),
    stepBack(StepBackArguments),
    reverseContinue(ReverseContinueArguments),
    threads(Option<NoArguments>),
    stackTrace(StackTraceArguments),
    scopes(ScopesArguments),
    source(SourceArguments),
    variables(VariablesArguments),
    completions(CompletionsArguments),
    gotoTargets(GotoTargetsArguments),
    goto(GotoArguments),
    restartFrame(RestartFrameArguments),
    evaluate(EvaluateArguments),
    setVariable(SetVariableArguments),
    dataBreakpointInfo(DataBreakpointInfoArguments),
    setDataBreakpoints(SetDataBreakpointsArguments),
    readMemory(ReadMemoryArguments),
    terminate(Option<TerminateArguments>),
    disconnect(Option<DisconnectArguments>),
    // Reverse
    runInTerminal(RunInTerminalRequestArguments),
    // Custom
    _adapterSettings(AdapterSettings),
    _symbols(SymbolsRequest),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "command", content = "body")]
pub enum ResponseBody {
    Async,
    initialize(Capabilities),
    cancel,
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
    gotoTargets(GotoTargetsResponseBody),
    goto,
    restartFrame,
    evaluate(EvaluateResponseBody),
    setVariable(SetVariableResponseBody),
    dataBreakpointInfo(DataBreakpointInfoResponseBody),
    setDataBreakpoints(SetDataBreakpointsResponseBody),
    readMemory(ReadMemoryResponseBody),
    terminate,
    disconnect,
    // Reverse
    runInTerminal(RunInTerminalResponseBody),
    // Custom
    _adapterSettings,
    _symbols(SymbolsResponse),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "event", content = "body")]
pub enum EventBody {
    initialized,
    output(OutputEventBody),
    breakpoint(BreakpointEventBody),
    capabilities(CapabilitiesEventBody),
    continued(ContinuedEventBody),
    exited(ExitedEventBody),
    module(ModuleEventBody),
    terminated(TerminatedEventBody),
    thread(ThreadEventBody),
    invalidated(InvalidatedEventBody),
    stopped(StoppedEventBody),
    // Custom
    displayHtml(DisplayHtmlEventBody),
}

#[derive(Serialize, Deserialize, Debug, Copy, Clone)]
#[serde(rename_all = "camelCase")]
pub struct NoArguments {}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CommonLaunchFields {
    pub name: Option<String>,
    pub stop_on_entry: Option<bool>,
    pub source_map: Option<VecMap<String, Option<String>>>,
    pub expressions: Option<Expressions>,
    pub init_commands: Option<Vec<String>>,
    pub pre_run_commands: Option<Vec<String>>,
    pub post_run_commands: Option<Vec<String>>,
    pub exit_commands: Option<Vec<String>>,
    pub source_languages: Option<Vec<String>>,
    pub reverse_debugging: Option<bool>,
    pub relative_path_base: Option<String>,
    #[serde(rename = "_adapterSettings")]
    pub adapter_settings: Option<AdapterSettings>,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LaunchRequestArguments {
    #[serde(flatten)]
    pub common: CommonLaunchFields,
    pub no_debug: Option<bool>,
    pub program: Option<String>,
    pub args: Option<Vec<String>>,
    pub cwd: Option<String>,
    pub env: Option<VecMap<String, String>>,
    pub stdio: Option<Either<String, Vec<Option<String>>>>,
    pub terminal: Option<TerminalKind>,
    pub console: Option<ConsoleKind>,
    pub target_create_commands: Option<Vec<String>>,
    pub process_create_commands: Option<Vec<String>>,
    pub custom: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AttachRequestArguments {
    #[serde(flatten)]
    pub common: CommonLaunchFields,
    pub program: Option<String>,
    pub pid: Option<Pid>,
    pub wait_for: Option<bool>,
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
pub enum ConsoleMode {
    Commands,
    Evaluate,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct AdapterSettings {
    pub display_format: Option<DisplayFormat>,
    pub show_disassembly: Option<ShowDisassembly>,
    pub dereference_pointers: Option<bool>,
    pub container_summary: Option<bool>,
    pub evaluation_timeout: Option<f32>,
    pub suppress_missing_source_files: Option<bool>,
    pub console_mode: Option<ConsoleMode>,
    pub source_languages: Option<Vec<String>>,
    pub terminal_prompt_clear: Option<Vec<String>>,
    pub evaluate_for_hovers: Option<bool>,
    pub command_completions: Option<bool>,
    pub reproducer: Option<Either<bool, String>>
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
pub enum ConsoleKind {
    IntegratedTerminal,
    ExternalTerminal,
    InternalConsole,
}

#[derive(Serialize, Deserialize, Debug, Copy, Clone)]
#[serde(rename_all = "camelCase")]
pub enum Expressions {
    Simple,
    Python,
    Native,
}

#[derive(Serialize, Deserialize, Debug, Copy, Clone)]
#[serde(untagged)]
pub enum Either<T1, T2> {
    First(T1),
    Second(T2),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SymbolsContinuation {
    pub next_module: u32,
    pub next_symbol: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Symbol {
    pub name: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub address: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SymbolsRequest {
    pub continuation_token: Option<SymbolsContinuation>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SymbolsResponse {
    pub symbols: Vec<Symbol>,
    pub continuation_token: Option<SymbolsContinuation>,
}

////////////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(test)]
    macro_rules! assert_matches(($e:expr, $p:pat) => { let e = $e; assert!(matches!(e, $p), "{:?} !~ {}", e, stringify!($p)) });

    fn parse(s: &[u8]) -> ProtocolMessage {
        serde_json::from_slice::<ProtocolMessage>(s).unwrap()
    }

    #[test]
    fn test_initialize() {
        let request = parse(br#"{"command":"initialize","arguments":{"clientID":"vscode","clientName":"Visual Studio Code","adapterID":"lldb","pathFormat":"path","linesStartAt1":true,"columnsStartAt1":true,"supportsVariableType":true,"supportsVariablePaging":true,"supportsRunInTerminalRequest":true,"locale":"en-us"},"type":"request","seq":1}"#);
        assert_matches!(
            request,
            ProtocolMessage::Request(Request {
                command: Command::Known(RequestArguments::initialize(..)),
                ..
            })
        );

        let response = parse(br#"{"request_seq":1,"command":"initialize","body":{"supportsDelayedStackTraceLoading":true,"supportsEvaluateForHovers":true,"exceptionBreakpointFilters":[{"filter":"rust_panic","default":true,"label":"Rust: on panic"}],"supportsCompletionsRequest":true,"supportsConditionalBreakpoints":true,"supportsStepBack":false,"supportsConfigurationDoneRequest":true,"supportTerminateDebuggee":true,"supportsLogPoints":true,"supportsFunctionBreakpoints":true,"supportsHitConditionalBreakpoints":true,"supportsSetVariable":true},"type":"response","success":true}"#);
        assert_matches!(
            response,
            ProtocolMessage::Response(Response {
                result: ResponseResult::Success {
                    body: ResponseBody::initialize(..)
                },
                ..
            })
        );
    }

    #[test]
    fn test_launch() {
        let request = parse(br#"{"type":"request","seq":2, "command":"launch","arguments":{"type":"lldb","request":"launch","name":"Debug tests in types_lib",
                        "program":"target/debug/types_lib-d6a67ab7ca515c6b",
                        "args":[],
                        "cwd":"/home/debuggee",
                        "initCommands":["platform shell echo 'init'"],
                        "env":{"TEST":"folder"},
                        "sourceMap":{"/checkout/src":"/home/user/nightly-x86_64-unknown-linux-gnu/lib/rustlib/src/rust/src"},
                        "debugServer":41025,
                        "_displaySettings":{"showDisassembly":"always","displayFormat":"auto","dereferencePointers":true,"toggleContainerSummary":false,"containerSummary":true},
                        "__sessionId":"81865613-a1ee-4a66-b449-a94165625fd2"}
                      }"#);
        assert_matches!(
            request,
            ProtocolMessage::Request(Request {
                command: Command::Known(RequestArguments::launch(..)),
                ..
            })
        );

        let response = parse(br#"{"request_seq":2,"command":"launch","body":null,"type":"response","success":true}"#);
        assert_matches!(
            response,
            ProtocolMessage::Response(Response {
                result: ResponseResult::Success {
                    body: ResponseBody::launch,
                },
                ..
            })
        );
    }

    #[test]
    fn test_event() {
        let event = parse(br#"{"type":"event","event":"initialized","seq":0}"#);
        assert_matches!(
            event,
            ProtocolMessage::Event(Event {
                body: EventBody::initialized,
                ..
            })
        );

        let event = parse(br#"{"body":{"reason":"started","threadId":7537},"type":"event","event":"thread","seq":0}"#);
        assert_matches!(
            event,
            ProtocolMessage::Event(Event {
                body: EventBody::thread(..),
                ..
            })
        );
    }

    #[test]
    fn test_scopes() {
        let request = parse(br#"{"command":"scopes","arguments":{"frameId":1000},"type":"request","seq":12}"#);
        assert_matches!(
            request,
            ProtocolMessage::Request(Request {
                command: Command::Known(RequestArguments::scopes(..)),
                ..
            })
        );

        let response = parse(br#"{"request_seq":12,"command":"scopes","body":{"scopes":[{"variablesReference":1001,"name":"Local","expensive":false},{"variablesReference":1002,"name":"Static","expensive":false},{"variablesReference":1003,"name":"Global","expensive":false},{"variablesReference":1004,"name":"Registers","expensive":false}]},"type":"response","success":true}"#);
        assert_matches!(
            response,
            ProtocolMessage::Response(Response {
                success: true,
                result: ResponseResult::Success {
                    body: ResponseBody::scopes(..),
                },
                ..
            })
        );
    }

    #[test]
    fn test_configuration_done() {
        let request = parse(br#"{"type":"request", "seq":12, "command":"configurationDone"}"#);
        println!("{:?}", request);
        assert_matches!(
            request,
            ProtocolMessage::Request(Request {
                command: Command::Known(RequestArguments::configurationDone(None)),
                ..
            })
        );
        let request =
            parse(br#"{"type":"request", "seq":12, "command":"configurationDone", "arguments": {"foo": "bar"}}"#);
        println!("{:?}", request);
        assert_matches!(
            request,
            ProtocolMessage::Request(Request {
                command: Command::Known(RequestArguments::configurationDone(Some(_))),
                ..
            })
        );
    }

    #[test]
    fn test_disconnect() {
        let request =
            parse(br#"{"type":"request", "seq":12, "command":"disconnect", "arguments":{"terminateDebuggee":true} }"#);
        assert_matches!(
            request,
            ProtocolMessage::Request(Request {
                command: Command::Known(RequestArguments::disconnect(Some(..))),
                ..
            })
        );

        let request = parse(br#"{"type":"request", "seq":12, "command":"disconnect"}"#);
        assert_matches!(
            request,
            ProtocolMessage::Request(Request {
                command: Command::Known(RequestArguments::disconnect(None)),
                ..
            })
        );
    }

    #[test]
    fn test_unknown() {
        let request = parse(br#"{"type":"request", "seq":12, "command":"foobar"}"#);
        assert_matches!(
            request,
            ProtocolMessage::Request(Request {
                command: Command::Unknown { .. },
                ..
            })
        );
    }
}
