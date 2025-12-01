#![allow(non_camel_case_types)]

mod dap;

use serde_derive::*;

pub use codelldb_types::*;

pub use crate::dap::{
    Breakpoint, BreakpointEventBody, CancelArguments, Capabilities, CapabilitiesEventBody, CompletionItem,
    CompletionsArguments, CompletionsResponseBody, ContinueArguments, ContinueResponseBody, ContinuedEventBody,
    DataBreakpoint, DataBreakpointAccessType, DataBreakpointInfoArguments, DataBreakpointInfoResponseBody,
    DisassembleArguments, DisassembleResponseBody, DisassembledInstruction, DisassembledInstructionPresentationHint,
    DisconnectArguments, EvaluateArguments, EvaluateResponseBody, ExceptionBreakMode, ExceptionBreakpointsFilter,
    ExceptionInfoArguments, ExceptionInfoResponseBody, ExitedEventBody, GotoArguments, GotoTarget,
    GotoTargetsArguments, GotoTargetsResponseBody, InitializeRequestArguments, InstructionBreakpoint, InvalidatedAreas,
    InvalidatedEventBody, Module, ModuleEventBody, ModuleEventBodyReason, ModuleId, ModulesArguments,
    ModulesResponseBody, NextArguments, OutputEventBody, PauseArguments, ReadMemoryArguments, ReadMemoryResponseBody,
    RestartFrameArguments, ReverseContinueArguments, RunInTerminalRequestArguments, RunInTerminalRequestArgumentsKind,
    RunInTerminalResponseBody, Scope, ScopesArguments, ScopesResponseBody, SetBreakpointsArguments,
    SetBreakpointsResponseBody, SetDataBreakpointsArguments, SetDataBreakpointsResponseBody,
    SetExceptionBreakpointsArguments, SetExceptionBreakpointsResponseBody, SetFunctionBreakpointsArguments,
    SetInstructionBreakpointsArguments, SetInstructionBreakpointsResponseBody, SetVariableArguments,
    SetVariableResponseBody, Source, SourceArguments, SourceBreakpoint, SourceResponseBody, StackFrame,
    StackFrameModuleId, StackFramePresentationHint, StackTraceArguments, StackTraceResponseBody, StepBackArguments,
    StepInArguments, StepInTarget, StepInTargetsArguments, StepInTargetsResponseBody, StepOutArguments,
    SteppingGranularity, StoppedEventBody, TerminateArguments, TerminatedEventBody, Thread, ThreadEventBody,
    ThreadsResponseBody, Variable, VariablePresentationHint, VariablesArguments, VariablesResponseBody,
    WriteMemoryArguments, WriteMemoryResponseBody,
};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProtocolMessage {
    pub seq: u32,
    #[serde(flatten)]
    pub type_: ProtocolMessageType,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type")]
pub enum ProtocolMessageType {
    #[serde(rename = "request")]
    Request(RequestArguments),
    #[serde(rename = "response")]
    Response(Response),
    #[serde(rename = "event")]
    Event(EventBody),
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
#[serde(tag = "command", content = "arguments")]
pub enum RequestArguments {
    initialize(InitializeRequestArguments),
    cancel(CancelArguments),
    launch(Either<LaunchRequestArguments, serde_json::Value>),
    attach(Either<AttachRequestArguments, serde_json::Value>),
    restart(Either<RestartRequestArguments, serde_json::Value>),
    setBreakpoints(SetBreakpointsArguments),
    setInstructionBreakpoints(SetInstructionBreakpointsArguments),
    setFunctionBreakpoints(SetFunctionBreakpointsArguments),
    setExceptionBreakpoints(SetExceptionBreakpointsArguments),
    exceptionInfo(ExceptionInfoArguments),
    configurationDone(Option<NoArguments>),
    pause(PauseArguments),
    #[serde(rename = "continue")]
    continue_(ContinueArguments),
    next(NextArguments),
    stepInTargets(StepInTargetsArguments),
    stepIn(StepInArguments),
    stepOut(StepOutArguments),
    stepBack(StepBackArguments),
    reverseContinue(ReverseContinueArguments),
    threads(Option<NoArguments>),
    stackTrace(StackTraceArguments),
    scopes(ScopesArguments),
    source(SourceArguments),
    modules(ModulesArguments),
    variables(VariablesArguments),
    completions(CompletionsArguments),
    gotoTargets(GotoTargetsArguments),
    goto(GotoArguments),
    restartFrame(RestartFrameArguments),
    evaluate(EvaluateArguments),
    setVariable(SetVariableArguments),
    dataBreakpointInfo(DataBreakpointInfoArguments),
    setDataBreakpoints(SetDataBreakpointsArguments),
    disassemble(DisassembleArguments),
    readMemory(ReadMemoryArguments),
    writeMemory(WriteMemoryArguments),
    terminate(Option<TerminateArguments>),
    disconnect(Option<DisconnectArguments>),
    // Reverse
    runInTerminal(RunInTerminalRequestArguments),
    // Custom
    _adapterSettings(AdapterSettings),
    _symbols(SymbolsRequest),
    _excludeCaller(ExcludeCallerRequest),
    _setExcludedCallers(SetExcludedCallersRequest),
    _pythonMessage(serde_json::Value),
    #[serde(other)]
    unknown,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "command", content = "body")]
pub enum ResponseBody {
    initialize(Capabilities),
    cancel,
    launch,
    attach,
    restart,
    setBreakpoints(SetBreakpointsResponseBody),
    setInstructionBreakpoints(SetInstructionBreakpointsResponseBody),
    setFunctionBreakpoints(SetBreakpointsResponseBody),
    setExceptionBreakpoints(SetExceptionBreakpointsResponseBody),
    exceptionInfo(ExceptionInfoResponseBody),
    configurationDone,
    pause,
    #[serde(rename = "continue")]
    continue_(ContinueResponseBody),
    next,
    stepInTargets(StepInTargetsResponseBody),
    stepIn,
    stepOut,
    stepBack,
    reverseContinue,
    threads(ThreadsResponseBody),
    stackTrace(StackTraceResponseBody),
    scopes(ScopesResponseBody),
    source(SourceResponseBody),
    modules(ModulesResponseBody),
    variables(VariablesResponseBody),
    completions(CompletionsResponseBody),
    gotoTargets(GotoTargetsResponseBody),
    goto,
    restartFrame,
    evaluate(EvaluateResponseBody),
    setVariable(SetVariableResponseBody),
    dataBreakpointInfo(DataBreakpointInfoResponseBody),
    setDataBreakpoints(SetDataBreakpointsResponseBody),
    disassemble(DisassembleResponseBody),
    readMemory(ReadMemoryResponseBody),
    writeMemory(WriteMemoryResponseBody),
    terminate,
    disconnect,
    // Reverse
    runInTerminal(RunInTerminalResponseBody),
    // Custom
    _adapterSettings,
    _symbols(SymbolsResponse),
    _excludeCaller(ExcludeCallerResponse),
    _setExcludedCallers,
    _pythonMessage,
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
    _pythonMessage(serde_json::Value),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RestartRequestArguments {
    pub arguments: Either<LaunchRequestArguments, AttachRequestArguments>,
}

#[derive(Serialize, Deserialize, Debug, Copy, Clone)]
#[serde(rename_all = "camelCase")]
pub struct NoArguments {}

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
            ProtocolMessage {
                seq: 1,
                type_: ProtocolMessageType::Request(RequestArguments::initialize(..))
            }
        );

        let response = parse(br#"{"seq":2, "request_seq":1,"command":"initialize","body":{"supportsDelayedStackTraceLoading":true,"supportsEvaluateForHovers":true,"exceptionBreakpointFilters":[{"filter":"rust_panic","default":true,"label":"Rust: on panic"}],"supportsCompletionsRequest":true,"supportsConditionalBreakpoints":true,"supportsStepBack":false,"supportsConfigurationDoneRequest":true,"supportTerminateDebuggee":true,"supportsLogPoints":true,"supportsFunctionBreakpoints":true,"supportsHitConditionalBreakpoints":true,"supportsSetVariable":true},"type":"response","success":true}"#);
        assert_matches!(
            response,
            ProtocolMessage {
                seq: 2,
                type_: ProtocolMessageType::Response(Response {
                    result: ResponseResult::Success {
                        body: ResponseBody::initialize(..)
                    },
                    ..
                })
            }
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
            ProtocolMessage {
                seq: 2,
                type_: ProtocolMessageType::Request(RequestArguments::launch(..))
            }
        );

        let response =
            parse(br#"{"seq": 3, "request_seq":2,"command":"launch","body":null,"type":"response","success":true}"#);
        assert_matches!(
            response,
            ProtocolMessage {
                seq: 3,
                type_: ProtocolMessageType::Response(Response {
                    result: ResponseResult::Success {
                        body: ResponseBody::launch
                    },
                    ..
                })
            }
        );
    }

    #[test]
    fn test_event() {
        let event = parse(br#"{"type":"event","event":"initialized","seq":0}"#);
        assert_matches!(
            event,
            ProtocolMessage {
                seq: 0,
                type_: ProtocolMessageType::Event(EventBody::initialized)
            }
        );

        let event = parse(br#"{"body":{"reason":"started","threadId":7537},"type":"event","event":"thread","seq":0}"#);
        assert_matches!(
            event,
            ProtocolMessage {
                seq: 0,
                type_: ProtocolMessageType::Event(EventBody::thread(..))
            }
        );
    }

    #[test]
    fn test_scopes() {
        let request = parse(br#"{"command":"scopes","arguments":{"frameId":1000},"type":"request","seq":12}"#);
        assert_matches!(
            request,
            ProtocolMessage {
                seq: 12,
                type_: ProtocolMessageType::Request(RequestArguments::scopes(..))
            }
        );

        let response = parse(br#"{"seq":34,"request_seq":12,"command":"scopes","body":{"scopes":[{"variablesReference":1001,"name":"Local","expensive":false},{"variablesReference":1002,"name":"Static","expensive":false},{"variablesReference":1003,"name":"Global","expensive":false},{"variablesReference":1004,"name":"Registers","expensive":false}]},"type":"response","success":true}"#);
        assert_matches!(
            response,
            ProtocolMessage {
                seq: 34,
                type_: ProtocolMessageType::Response(Response {
                    request_seq: 12,
                    success: true,
                    result: ResponseResult::Success {
                        body: ResponseBody::scopes(..)
                    },
                    ..
                })
            }
        );
    }

    #[test]
    fn test_configuration_done() {
        let request = parse(br#"{"type":"request", "seq":12, "command":"configurationDone"}"#);
        println!("{:?}", request);
        assert_matches!(
            request,
            ProtocolMessage {
                seq: 12,
                type_: ProtocolMessageType::Request(RequestArguments::configurationDone(None))
            }
        );

        let request =
            parse(br#"{"type":"request", "seq":12, "command":"configurationDone", "arguments": {"foo": "bar"}}"#);
        println!("{:?}", request);
        assert_matches!(
            request,
            ProtocolMessage {
                seq: 12,
                type_: ProtocolMessageType::Request(RequestArguments::configurationDone(Some(_)))
            }
        );
    }

    #[test]
    fn test_disconnect() {
        let request =
            parse(br#"{"type":"request", "seq":12, "command":"disconnect", "arguments":{"terminateDebuggee":true} }"#);
        assert_matches!(
            request,
            ProtocolMessage {
                seq: 12,
                type_: ProtocolMessageType::Request(RequestArguments::disconnect(Some(..)))
            }
        );

        let request = parse(br#"{"type":"request", "seq":12, "command":"disconnect"}"#);
        assert_matches!(
            request,
            ProtocolMessage {
                seq: 12,
                type_: ProtocolMessageType::Request(RequestArguments::disconnect(None))
            }
        );
    }

    #[test]
    fn test_unknown() {
        let request = parse(br#"{"type":"request", "seq":12, "command":"foobar"}"#);
        assert_matches!(
            request,
            ProtocolMessage {
                seq: 12,
                type_: ProtocolMessageType::Request(RequestArguments::unknown)
            }
        );
    }
}
