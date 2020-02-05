#![allow(non_camel_case_types)]

use crate::vec_map::VecMap;
use serde_derive::*;

pub use raw_debug_protocol::{
    Breakpoint, BreakpointEventBody, Capabilities, CapabilitiesEventBody, CompletionItem, CompletionsArguments,
    CompletionsResponseBody, ContinueArguments, ContinueResponseBody, ContinuedEventBody, DataBreakpoint,
    DataBreakpointAccessType, DataBreakpointInfoArguments, DataBreakpointInfoResponseBody, DisconnectArguments,
    EvaluateArguments, EvaluateResponseBody, ExceptionBreakpointsFilter, ExitedEventBody, GotoArguments, GotoTarget,
    GotoTargetsArguments, GotoTargetsResponseBody, InitializeRequestArguments, Module, ModuleEventBody, NextArguments,
    OutputEventBody, PauseArguments, RestartFrameArguments, ReverseContinueArguments,
    RunInTerminalRequestArguments, Scope, ScopesArguments, ScopesResponseBody, SetBreakpointsArguments,
    SetBreakpointsResponseBody, SetDataBreakpointsArguments, SetDataBreakpointsResponseBody,
    SetExceptionBreakpointsArguments, SetFunctionBreakpointsArguments, SetVariableArguments, SetVariableResponseBody,
    Source, SourceArguments, SourceBreakpoint, SourceResponseBody, StackFrame, StackTraceArguments,
    StackTraceResponseBody, StepBackArguments, StepInArguments, StepOutArguments, StoppedEventBody,
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
    reverseContinue(ReverseContinueArguments),
    threads,
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
    disconnect(DisconnectArguments),
    // Custom
    adapterSettings(AdapterSettings),
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
    gotoTargets(GotoTargetsResponseBody),
    goto,
    restartFrame,
    evaluate(EvaluateResponseBody),
    setVariable(SetVariableResponseBody),
    dataBreakpointInfo(DataBreakpointInfoResponseBody),
    setDataBreakpoints(SetDataBreakpointsResponseBody),
    disconnect,
    // Custom
    adapterSettings,
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
    stopped(StoppedEventBody),
    // Custom
    displayHtml(DisplayHtmlEventBody),
}

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
    pub default_panic_breakpoint: Option<bool>,
    pub default_catch_breakpoint: Option<bool>
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

#[derive(Serialize, Deserialize, Debug, Copy, Clone)]
#[serde(untagged)]
pub enum Either<T1, T2> {
    First(T1),
    Second(T2),
}

////////////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! assert_match(($e:expr, $p:pat) => { assert!(match $e { $p => true, _ => false }, stringify!($e ~ $p)) });

    fn parse(s: &[u8]) -> ProtocolMessage {
        serde_json::from_slice::<ProtocolMessage>(s).unwrap()
    }

    #[test]
    fn test_initialize() {
        let request = parse(br#"{"command":"initialize","arguments":{"clientID":"vscode","clientName":"Visual Studio Code","adapterID":"lldb","pathFormat":"path","linesStartAt1":true,"columnsStartAt1":true,"supportsVariableType":true,"supportsVariablePaging":true,"supportsRunInTerminalRequest":true,"locale":"en-us"},"type":"request","seq":1}"#);
        assert_match!(
            request,
            ProtocolMessage::Request(Request {
                command: Command::Known(RequestArguments::initialize(..)),
                ..
            })
        );

        let response = parse(br#"{"request_seq":1,"command":"initialize","body":{"supportsDelayedStackTraceLoading":true,"supportsEvaluateForHovers":true,"exceptionBreakpointFilters":[{"filter":"rust_panic","default":true,"label":"Rust: on panic"}],"supportsCompletionsRequest":true,"supportsConditionalBreakpoints":true,"supportsStepBack":false,"supportsConfigurationDoneRequest":true,"supportTerminateDebuggee":true,"supportsLogPoints":true,"supportsFunctionBreakpoints":true,"supportsHitConditionalBreakpoints":true,"supportsSetVariable":true},"type":"response","success":true}"#);
        assert_match!(
            response,
            ProtocolMessage::Response(Response {
                body: Some(ResponseBody::initialize(..)),
                ..
            })
        );
    }

    #[test]
    fn test_launch() {
        let request = parse(br#"{"type":"request","seq":2, "command":"launch","arguments":{"type":"lldb","request":"launch","name":"Debug tests in types_lib",
                        "program":"/home/chega/NW/vscode-lldb/debuggee/target/debug/types_lib-d6a67ab7ca515c6b",
                        "args":[],
                        "cwd":"/home/chega/NW/vscode-lldb/debuggee",
                        "initCommands":["platform shell echo 'init'"],
                        "env":{"TEST":"folder"},
                        "sourceMap":{"/checkout/src":"/home/chega/.rustup/toolchains/nightly-x86_64-unknown-linux-gnu/lib/rustlib/src/rust/src"},
                        "debugServer":41025,
                        "_displaySettings":{"showDisassembly":"always","displayFormat":"auto","dereferencePointers":true,"toggleContainerSummary":false,"containerSummary":true},
                        "__sessionId":"81865613-a1ee-4a66-b449-a94165625fd2"}
                      }"#);
        assert_match!(
            request,
            ProtocolMessage::Request(Request {
                command: Command::Known(RequestArguments::launch(..)),
                ..
            })
        );

        let response = parse(br#"{"request_seq":2,"command":"launch","body":null,"type":"response","success":true}"#);
        assert_match!(
            response,
            ProtocolMessage::Response(Response {
                body: Some(ResponseBody::launch),
                ..
            })
        );
    }

    #[test]
    fn test_event() {
        let event = parse(br#"{"type":"event","event":"initialized","seq":0}"#);
        assert_match!(
            event,
            ProtocolMessage::Event(Event {
                body: EventBody::initialized,
                ..
            })
        );

        let event = parse(br#"{"body":{"reason":"started","threadId":7537},"type":"event","event":"thread","seq":0}"#);
        assert_match!(
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
        assert_match!(
            request,
            ProtocolMessage::Request(Request {
                command: Command::Known(RequestArguments::scopes(..)),
                ..
            })
        );

        let response = parse(br#"{"request_seq":12,"command":"scopes","body":{"scopes":[{"variablesReference":1001,"name":"Local","expensive":false},{"variablesReference":1002,"name":"Static","expensive":false},{"variablesReference":1003,"name":"Global","expensive":false},{"variablesReference":1004,"name":"Registers","expensive":false}]},"type":"response","success":true}"#);
        assert_match!(
            response,
            ProtocolMessage::Response(Response {
                body: Some(ResponseBody::scopes(..)),
                ..
            })
        );
    }

    #[test]
    fn test_disconnect() {
        let request =
            parse(br#"{"type":"request", "seq":12, "command":"disconnect", "arguments":{"terminateDebuggee":true} }"#);
        assert_match!(
            request,
            ProtocolMessage::Request(Request {
                command: Command::Known(RequestArguments::disconnect(..)),
                ..
            })
        );

        let request = parse(br#"{"type":"request", "seq":12, "command":"disconnect"}"#);
        assert_match!(
            request,
            ProtocolMessage::Request(Request {
                command: Command::Unknown {
                    ..
                },
                ..
            })
        );
    }

    #[test]
    fn test_unknown() {
        let request = parse(br#"{"type":"request", "seq":12, "command":"foobar"}"#);
        assert_match!(
            request,
            ProtocolMessage::Request(Request {
                command: Command::Unknown {
                    ..
                },
                ..
            })
        );
    }
}
