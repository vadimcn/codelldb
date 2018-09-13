
use serde_json;
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct AttachRequest {
        /// Object containing arguments for the command.
        pub arguments: AttachRequestArguments,
        /// The command to execute.
        pub command: String,
        /// Sequence number.
        pub seq: i64,
        /// Message type.
        #[serde(rename = "type")]
        pub type_: String,
}
/// Arguments for 'attach' request. Additional attributes are implementation specific.
#[derive(Clone, PartialEq, Debug, Default, Deserialize, Serialize)]
pub struct AttachRequestArguments {
        /// Optional data from the previous, restarted session.
        /// The data is sent as the 'restart' attribute of the 'terminated' event.
        /// The client should leave the data intact.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub __restart: Option<serde_json::Value>,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct AttachResponse {
        /// Contains request result if success is true and optional error details if success is false.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub body: Option<serde_json::Value>,
        /// The command requested.
        pub command: String,
        /// Contains error message if success == false.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub message: Option<String>,
        /// Sequence number of the corresponding request.
        pub request_seq: i64,
        /// Sequence number.
        pub seq: i64,
        /// Outcome of the request.
        pub success: bool,
        /// Message type.
        #[serde(rename = "type")]
        pub type_: String,
}
/// Information about a Breakpoint created in setBreakpoints or setFunctionBreakpoints.
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct Breakpoint {
        /// An optional start column of the actual range covered by the breakpoint.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub column: Option<i64>,
        /// An optional end column of the actual range covered by the breakpoint. If no end line is
        /// given, then the end column is assumed to be in the start line.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "endColumn")]
        pub end_column: Option<i64>,
        /// An optional end line of the actual range covered by the breakpoint.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "endLine")]
        pub end_line: Option<i64>,
        /// An optional unique identifier for the breakpoint.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub id: Option<i64>,
        /// The start line of the actual range covered by the breakpoint.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub line: Option<i64>,
        /// An optional message about the state of the breakpoint. This is shown to the user and can be
        /// used to explain why a breakpoint could not be verified.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub message: Option<String>,
        /// The source where the breakpoint is located.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub source: Option<Source>,
        /// If true breakpoint could be set (but not necessarily at the desired location).
        pub verified: bool,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct BreakpointEventBody {
        /// The breakpoint.
        pub breakpoint: Breakpoint,
        /// The reason for the event.
        pub reason: String,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct BreakpointEvent {
        /// Event-specific information.
        pub body: BreakpointEventBody,
        /// Type of event.
        pub event: String,
        /// Sequence number.
        pub seq: i64,
        /// Message type.
        #[serde(rename = "type")]
        pub type_: String,
}
/// Information about the capabilities of a debug adapter.
#[derive(Clone, PartialEq, Debug, Default, Deserialize, Serialize)]
pub struct Capabilities {
        /// The set of additional module information exposed by the debug adapter.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "additionalModuleColumns")]
        pub additional_module_columns: Option<Vec<ColumnDescriptor>>,
        /// Available filters or options for the setExceptionBreakpoints request.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "exceptionBreakpointFilters")]
        pub exception_breakpoint_filters: Option<Vec<ExceptionBreakpointsFilter>>,
        /// The debug adapter supports the 'terminateDebuggee' attribute on the 'disconnect' request.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "supportTerminateDebuggee")]
        pub support_terminate_debuggee: Option<bool>,
        /// Checksum algorithms supported by the debug adapter.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "supportedChecksumAlgorithms")]
        pub supported_checksum_algorithms: Option<Vec<ChecksumAlgorithm>>,
        /// The debug adapter supports the 'completions' request.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "supportsCompletionsRequest")]
        pub supports_completions_request: Option<bool>,
        /// The debug adapter supports conditional breakpoints.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "supportsConditionalBreakpoints")]
        pub supports_conditional_breakpoints: Option<bool>,
        /// The debug adapter supports the 'configurationDone' request.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "supportsConfigurationDoneRequest")]
        pub supports_configuration_done_request: Option<bool>,
        /// The debug adapter supports the delayed loading of parts of the stack, which requires that
        /// both the 'startFrame' and 'levels' arguments and the 'totalFrames' result of the
        /// 'StackTrace' request are supported.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "supportsDelayedStackTraceLoading")]
        pub supports_delayed_stack_trace_loading: Option<bool>,
        /// The debug adapter supports a (side effect free) evaluate request for data hovers.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "supportsEvaluateForHovers")]
        pub supports_evaluate_for_hovers: Option<bool>,
        /// The debug adapter supports the 'exceptionInfo' request.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "supportsExceptionInfoRequest")]
        pub supports_exception_info_request: Option<bool>,
        /// The debug adapter supports 'exceptionOptions' on the setExceptionBreakpoints request.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "supportsExceptionOptions")]
        pub supports_exception_options: Option<bool>,
        /// The debug adapter supports function breakpoints.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "supportsFunctionBreakpoints")]
        pub supports_function_breakpoints: Option<bool>,
        /// The debug adapter supports the 'gotoTargets' request.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "supportsGotoTargetsRequest")]
        pub supports_goto_targets_request: Option<bool>,
        /// The debug adapter supports breakpoints that break execution after a specified number of
        /// hits.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "supportsHitConditionalBreakpoints")]
        pub supports_hit_conditional_breakpoints: Option<bool>,
        /// The debug adapter supports the 'loadedSources' request.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "supportsLoadedSourcesRequest")]
        pub supports_loaded_sources_request: Option<bool>,
        /// The debug adapter supports logpoints by interpreting the 'logMessage' attribute of the
        /// SourceBreakpoint.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "supportsLogPoints")]
        pub supports_log_points: Option<bool>,
        /// The debug adapter supports the 'modules' request.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "supportsModulesRequest")]
        pub supports_modules_request: Option<bool>,
        /// The debug adapter supports restarting a frame.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "supportsRestartFrame")]
        pub supports_restart_frame: Option<bool>,
        /// The debug adapter supports the 'restart' request. In this case a client should not
        /// implement 'restart' by terminating and relaunching the adapter but by calling the
        /// RestartRequest.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "supportsRestartRequest")]
        pub supports_restart_request: Option<bool>,
        /// The debug adapter supports the 'setExpression' request.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "supportsSetExpression")]
        pub supports_set_expression: Option<bool>,
        /// The debug adapter supports setting a variable to a value.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "supportsSetVariable")]
        pub supports_set_variable: Option<bool>,
        /// The debug adapter supports stepping back via the 'stepBack' and 'reverseContinue' requests.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "supportsStepBack")]
        pub supports_step_back: Option<bool>,
        /// The debug adapter supports the 'stepInTargets' request.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "supportsStepInTargetsRequest")]
        pub supports_step_in_targets_request: Option<bool>,
        /// The debug adapter supports the 'terminateThreads' request.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "supportsTerminateThreadsRequest")]
        pub supports_terminate_threads_request: Option<bool>,
        /// The debug adapter supports a 'format' attribute on the stackTraceRequest, variablesRequest,
        /// and evaluateRequest.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "supportsValueFormattingOptions")]
        pub supports_value_formatting_options: Option<bool>,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct CapabilitiesEventBody {
        /// The set of updated capabilities.
        pub capabilities: Capabilities,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct CapabilitiesEvent {
        /// Event-specific information.
        pub body: CapabilitiesEventBody,
        /// Type of event.
        pub event: String,
        /// Sequence number.
        pub seq: i64,
        /// Message type.
        #[serde(rename = "type")]
        pub type_: String,
}
/// The checksum of an item calculated by the specified algorithm.
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct Checksum {
        /// The algorithm used to calculate this checksum.
        pub algorithm: ChecksumAlgorithm,
        /// Value of the checksum.
        pub checksum: String,
}
/// Names of checksum algorithms that may be supported by a debug adapter.
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub enum ChecksumAlgorithm {
        #[serde(rename = "MD5")]
        Md5,
        #[serde(rename = "SHA1")]
        Sha1,
        #[serde(rename = "SHA256")]
        Sha256,
        #[serde(rename = "timestamp")]
        Timestamp,
}
/// A ColumnDescriptor specifies what module attribute to show in a column of the ModulesView, how
/// to format it, and what the column's label should be.
/// It is only used if the underlying UI actually supports this level of customization.
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct ColumnDescriptor {
        /// Name of the attribute rendered in this column.
        #[serde(rename = "attributeName")]
        pub attribute_name: String,
        /// Format to use for the rendered values in this column. TBD how the format strings looks
        /// like.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub format: Option<String>,
        /// Header UI label of column.
        pub label: String,
        /// Datatype of values in this column.  Defaults to 'string' if not specified.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "type")]
        pub type_: Option<String>,
        /// Width of this column in characters (hint only).
        #[serde(skip_serializing_if = "Option::is_none")]
        pub width: Option<i64>,
}
/// CompletionItems are the suggestions returned from the CompletionsRequest.
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct CompletionItem {
        /// The label of this completion item. By default this is also the text that is inserted when
        /// selecting this completion.
        pub label: String,
        /// This value determines how many characters are overwritten by the completion text.
        /// If missing the value 0 is assumed which results in the completion text being inserted.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub length: Option<i64>,
        /// This value determines the location (in the CompletionsRequest's 'text' attribute) where the
        /// completion text is added.
        /// If missing the text is added at the location specified by the CompletionsRequest's 'column'
        /// attribute.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub start: Option<i64>,
        /// If text is not falsy then it is inserted instead of the label.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub text: Option<String>,
        /// The item's type. Typically the client uses this information to render the item in the UI
        /// with an icon.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "type")]
        pub type_: Option<CompletionItemType>,
}
/// Some predefined types for the CompletionItem. Please note that not all clients have specific
/// icons for all of them.
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub enum CompletionItemType {
        #[serde(rename = "method")]
        Method,
        #[serde(rename = "function")]
        Function,
        #[serde(rename = "constructor")]
        Constructor,
        #[serde(rename = "field")]
        Field,
        #[serde(rename = "variable")]
        Variable,
        #[serde(rename = "class")]
        Class,
        #[serde(rename = "interface")]
        Interface,
        #[serde(rename = "module")]
        Module,
        #[serde(rename = "property")]
        Property,
        #[serde(rename = "unit")]
        Unit,
        #[serde(rename = "value")]
        Value,
        #[serde(rename = "enum")]
        Enum,
        #[serde(rename = "keyword")]
        Keyword,
        #[serde(rename = "snippet")]
        Snippet,
        #[serde(rename = "text")]
        Text,
        #[serde(rename = "color")]
        Color,
        #[serde(rename = "file")]
        File,
        #[serde(rename = "reference")]
        Reference,
        #[serde(rename = "customcolor")]
        Customcolor,
}
/// Arguments for 'completions' request.
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct CompletionsArguments {
        /// The character position for which to determine the completion proposals.
        pub column: i64,
        /// Returns completions in the scope of this stack frame. If not specified, the completions are
        /// returned for the global scope.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "frameId")]
        pub frame_id: Option<i64>,
        /// An optional line for which to determine the completion proposals. If missing the first line
        /// of the text is assumed.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub line: Option<i64>,
        /// One or more source lines. Typically this is the text a user has typed into the debug
        /// console before he asked for completion.
        pub text: String,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct CompletionsRequest {
        /// Object containing arguments for the command.
        pub arguments: CompletionsArguments,
        /// The command to execute.
        pub command: String,
        /// Sequence number.
        pub seq: i64,
        /// Message type.
        #[serde(rename = "type")]
        pub type_: String,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct CompletionsResponseBody {
        /// The possible completions for .
        pub targets: Vec<CompletionItem>,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct CompletionsResponse {
        /// Contains request result if success is true and optional error details if success is false.
        pub body: CompletionsResponseBody,
        /// The command requested.
        pub command: String,
        /// Contains error message if success == false.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub message: Option<String>,
        /// Sequence number of the corresponding request.
        pub request_seq: i64,
        /// Sequence number.
        pub seq: i64,
        /// Outcome of the request.
        pub success: bool,
        /// Message type.
        #[serde(rename = "type")]
        pub type_: String,
}
/// Arguments for 'configurationDone' request.
/// The configurationDone request has no standardized attributes.
pub type ConfigurationDoneArguments = ::std::collections::BTreeMap<String, serde_json::Value>;
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct ConfigurationDoneRequest {
        /// Object containing arguments for the command.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub arguments: Option<ConfigurationDoneArguments>,
        /// The command to execute.
        pub command: String,
        /// Sequence number.
        pub seq: i64,
        /// Message type.
        #[serde(rename = "type")]
        pub type_: String,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct ConfigurationDoneResponse {
        /// Contains request result if success is true and optional error details if success is false.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub body: Option<serde_json::Value>,
        /// The command requested.
        pub command: String,
        /// Contains error message if success == false.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub message: Option<String>,
        /// Sequence number of the corresponding request.
        pub request_seq: i64,
        /// Sequence number.
        pub seq: i64,
        /// Outcome of the request.
        pub success: bool,
        /// Message type.
        #[serde(rename = "type")]
        pub type_: String,
}
/// Arguments for 'continue' request.
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct ContinueArguments {
        /// Continue execution for the specified thread (if possible). If the backend cannot continue
        /// on a single thread but will continue on all threads, it should set the allThreadsContinued
        /// attribute in the response to true.
        #[serde(rename = "threadId")]
        pub thread_id: i64,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct ContinueRequest {
        /// Object containing arguments for the command.
        pub arguments: ContinueArguments,
        /// The command to execute.
        pub command: String,
        /// Sequence number.
        pub seq: i64,
        /// Message type.
        #[serde(rename = "type")]
        pub type_: String,
}
#[derive(Clone, PartialEq, Debug, Default, Deserialize, Serialize)]
pub struct ContinueResponseBody {
        /// If true, the continue request has ignored the specified thread and continued all threads
        /// instead. If this attribute is missing a value of 'true' is assumed for backward
        /// compatibility.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "allThreadsContinued")]
        pub all_threads_continued: Option<bool>,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct ContinueResponse {
        /// Contains request result if success is true and optional error details if success is false.
        pub body: ContinueResponseBody,
        /// The command requested.
        pub command: String,
        /// Contains error message if success == false.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub message: Option<String>,
        /// Sequence number of the corresponding request.
        pub request_seq: i64,
        /// Sequence number.
        pub seq: i64,
        /// Outcome of the request.
        pub success: bool,
        /// Message type.
        #[serde(rename = "type")]
        pub type_: String,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct ContinuedEventBody {
        /// If allThreadsContinued is true, a debug adapter can announce that all threads have
        /// continued.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "allThreadsContinued")]
        pub all_threads_continued: Option<bool>,
        /// The thread which was continued.
        #[serde(rename = "threadId")]
        pub thread_id: i64,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct ContinuedEvent {
        /// Event-specific information.
        pub body: ContinuedEventBody,
        /// Type of event.
        pub event: String,
        /// Sequence number.
        pub seq: i64,
        /// Message type.
        #[serde(rename = "type")]
        pub type_: String,
}
/// Arguments for 'disconnect' request.
#[derive(Clone, PartialEq, Debug, Default, Deserialize, Serialize)]
pub struct DisconnectArguments {
        /// Indicates whether the debuggee should be terminated when the debugger is disconnected.
        /// If unspecified, the debug adapter is free to do whatever it thinks is best.
        /// A client can only rely on this attribute being properly honored if a debug adapter returns
        /// true for the 'supportTerminateDebuggee' capability.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "terminateDebuggee")]
        pub terminate_debuggee: Option<bool>,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct DisconnectRequest {
        /// Object containing arguments for the command.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub arguments: Option<DisconnectArguments>,
        /// The command to execute.
        pub command: String,
        /// Sequence number.
        pub seq: i64,
        /// Message type.
        #[serde(rename = "type")]
        pub type_: String,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct DisconnectResponse {
        /// Contains request result if success is true and optional error details if success is false.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub body: Option<serde_json::Value>,
        /// The command requested.
        pub command: String,
        /// Contains error message if success == false.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub message: Option<String>,
        /// Sequence number of the corresponding request.
        pub request_seq: i64,
        /// Sequence number.
        pub seq: i64,
        /// Outcome of the request.
        pub success: bool,
        /// Message type.
        #[serde(rename = "type")]
        pub type_: String,
}
#[derive(Clone, PartialEq, Debug, Default, Deserialize, Serialize)]
pub struct ErrorResponseBody {
        /// An optional, structured error message.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub error: Option<Message>,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct ErrorResponse {
        /// Contains request result if success is true and optional error details if success is false.
        pub body: ErrorResponseBody,
        /// The command requested.
        pub command: String,
        /// Contains error message if success == false.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub message: Option<String>,
        /// Sequence number of the corresponding request.
        pub request_seq: i64,
        /// Sequence number.
        pub seq: i64,
        /// Outcome of the request.
        pub success: bool,
        /// Message type.
        #[serde(rename = "type")]
        pub type_: String,
}
/// Arguments for 'evaluate' request.
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct EvaluateArguments {
        /// The context in which the evaluate request is run.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub context: Option<String>,
        /// The expression to evaluate.
        pub expression: String,
        /// Specifies details on how to format the Evaluate result.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub format: Option<ValueFormat>,
        /// Evaluate the expression in the scope of this stack frame. If not specified, the expression
        /// is evaluated in the global scope.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "frameId")]
        pub frame_id: Option<i64>,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct EvaluateRequest {
        /// Object containing arguments for the command.
        pub arguments: EvaluateArguments,
        /// The command to execute.
        pub command: String,
        /// Sequence number.
        pub seq: i64,
        /// Message type.
        #[serde(rename = "type")]
        pub type_: String,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct EvaluateResponseBody {
        /// The number of indexed child variables.
        /// The client can use this optional information to present the variables in a paged UI and
        /// fetch them in chunks.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "indexedVariables")]
        pub indexed_variables: Option<i64>,
        /// The number of named child variables.
        /// The client can use this optional information to present the variables in a paged UI and
        /// fetch them in chunks.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "namedVariables")]
        pub named_variables: Option<i64>,
        /// Properties of a evaluate result that can be used to determine how to render the result in
        /// the UI.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "presentationHint")]
        pub presentation_hint: Option<VariablePresentationHint>,
        /// The result of the evaluate request.
        pub result: String,
        /// The optional type of the evaluate result.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "type")]
        pub type_: Option<String>,
        /// If variablesReference is > 0, the evaluate result is structured and its children can be
        /// retrieved by passing variablesReference to the VariablesRequest.
        #[serde(rename = "variablesReference")]
        pub variables_reference: i64,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct EvaluateResponse {
        /// Contains request result if success is true and optional error details if success is false.
        pub body: EvaluateResponseBody,
        /// The command requested.
        pub command: String,
        /// Contains error message if success == false.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub message: Option<String>,
        /// Sequence number of the corresponding request.
        pub request_seq: i64,
        /// Sequence number.
        pub seq: i64,
        /// Outcome of the request.
        pub success: bool,
        /// Message type.
        #[serde(rename = "type")]
        pub type_: String,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct Event {
        /// Event-specific information.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub body: Option<serde_json::Value>,
        /// Type of event.
        pub event: String,
        /// Sequence number.
        pub seq: i64,
        /// Message type.
        #[serde(rename = "type")]
        pub type_: String,
}
/// This enumeration defines all possible conditions when a thrown exception should result in a
/// break.
/// never: never breaks,
/// always: always breaks,
/// unhandled: breaks when excpetion unhandled,
/// userUnhandled: breaks if the exception is not handled by user code.
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub enum ExceptionBreakMode {
        #[serde(rename = "never")]
        Never,
        #[serde(rename = "always")]
        Always,
        #[serde(rename = "unhandled")]
        Unhandled,
        #[serde(rename = "userUnhandled")]
        UserUnhandled,
}
/// An ExceptionBreakpointsFilter is shown in the UI as an option for configuring how exceptions
/// are dealt with.
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct ExceptionBreakpointsFilter {
        /// Initial value of the filter. If not specified a value 'false' is assumed.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub default: Option<bool>,
        /// The internal ID of the filter. This value is passed to the setExceptionBreakpoints request.
        pub filter: String,
        /// The name of the filter. This will be shown in the UI.
        pub label: String,
}
/// Detailed information about an exception that has occurred.
#[derive(Clone, PartialEq, Debug, Default, Deserialize, Serialize)]
pub struct ExceptionDetails {
        /// Optional expression that can be evaluated in the current scope to obtain the exception
        /// object.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "evaluateName")]
        pub evaluate_name: Option<String>,
        /// Fully-qualified type name of the exception object.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "fullTypeName")]
        pub full_type_name: Option<String>,
        /// Details of the exception contained by this exception, if any.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "innerException")]
        pub inner_exception: Option<Vec<ExceptionDetails>>,
        /// Message contained in the exception.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub message: Option<String>,
        /// Stack trace at the time the exception was thrown.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "stackTrace")]
        pub stack_trace: Option<String>,
        /// Short type name of the exception object.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "typeName")]
        pub type_name: Option<String>,
}
/// Arguments for 'exceptionInfo' request.
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct ExceptionInfoArguments {
        /// Thread for which exception information should be retrieved.
        #[serde(rename = "threadId")]
        pub thread_id: i64,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct ExceptionInfoRequest {
        /// Object containing arguments for the command.
        pub arguments: ExceptionInfoArguments,
        /// The command to execute.
        pub command: String,
        /// Sequence number.
        pub seq: i64,
        /// Message type.
        #[serde(rename = "type")]
        pub type_: String,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct ExceptionInfoResponseBody {
        /// Mode that caused the exception notification to be raised.
        #[serde(rename = "breakMode")]
        pub break_mode: ExceptionBreakMode,
        /// Descriptive text for the exception provided by the debug adapter.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub description: Option<String>,
        /// Detailed information about the exception.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub details: Option<ExceptionDetails>,
        /// ID of the exception that was thrown.
        #[serde(rename = "exceptionId")]
        pub exception_id: String,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct ExceptionInfoResponse {
        /// Contains request result if success is true and optional error details if success is false.
        pub body: ExceptionInfoResponseBody,
        /// The command requested.
        pub command: String,
        /// Contains error message if success == false.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub message: Option<String>,
        /// Sequence number of the corresponding request.
        pub request_seq: i64,
        /// Sequence number.
        pub seq: i64,
        /// Outcome of the request.
        pub success: bool,
        /// Message type.
        #[serde(rename = "type")]
        pub type_: String,
}
/// An ExceptionOptions assigns configuration options to a set of exceptions.
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct ExceptionOptions {
        /// Condition when a thrown exception should result in a break.
        #[serde(rename = "breakMode")]
        pub break_mode: ExceptionBreakMode,
        /// A path that selects a single or multiple exceptions in a tree. If 'path' is missing, the
        /// whole tree is selected. By convention the first segment of the path is a category that is
        /// used to group exceptions in the UI.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub path: Option<Vec<ExceptionPathSegment>>,
}
/// An ExceptionPathSegment represents a segment in a path that is used to match leafs or nodes in
/// a tree of exceptions. If a segment consists of more than one name, it matches the names
/// provided if 'negate' is false or missing or it matches anything except the names provided if
/// 'negate' is true.
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct ExceptionPathSegment {
        /// Depending on the value of 'negate' the names that should match or not match.
        pub names: Vec<String>,
        /// If false or missing this segment matches the names provided, otherwise it matches anything
        /// except the names provided.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub negate: Option<bool>,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct ExitedEventBody {
        /// The exit code returned from the debuggee.
        #[serde(rename = "exitCode")]
        pub exit_code: i64,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct ExitedEvent {
        /// Event-specific information.
        pub body: ExitedEventBody,
        /// Type of event.
        pub event: String,
        /// Sequence number.
        pub seq: i64,
        /// Message type.
        #[serde(rename = "type")]
        pub type_: String,
}
/// Properties of a breakpoint passed to the setFunctionBreakpoints request.
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct FunctionBreakpoint {
        /// An optional expression for conditional breakpoints.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub condition: Option<String>,
        /// An optional expression that controls how many hits of the breakpoint are ignored. The
        /// backend is expected to interpret the expression as needed.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "hitCondition")]
        pub hit_condition: Option<String>,
        /// The name of the function.
        pub name: String,
}
/// Arguments for 'goto' request.
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct GotoArguments {
        /// The location where the debuggee will continue to run.
        #[serde(rename = "targetId")]
        pub target_id: i64,
        /// Set the goto target for this thread.
        #[serde(rename = "threadId")]
        pub thread_id: i64,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct GotoRequest {
        /// Object containing arguments for the command.
        pub arguments: GotoArguments,
        /// The command to execute.
        pub command: String,
        /// Sequence number.
        pub seq: i64,
        /// Message type.
        #[serde(rename = "type")]
        pub type_: String,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct GotoResponse {
        /// Contains request result if success is true and optional error details if success is false.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub body: Option<serde_json::Value>,
        /// The command requested.
        pub command: String,
        /// Contains error message if success == false.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub message: Option<String>,
        /// Sequence number of the corresponding request.
        pub request_seq: i64,
        /// Sequence number.
        pub seq: i64,
        /// Outcome of the request.
        pub success: bool,
        /// Message type.
        #[serde(rename = "type")]
        pub type_: String,
}
/// A GotoTarget describes a code location that can be used as a target in the 'goto' request.
/// The possible goto targets can be determined via the 'gotoTargets' request.
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct GotoTarget {
        /// An optional column of the goto target.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub column: Option<i64>,
        /// An optional end column of the range covered by the goto target.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "endColumn")]
        pub end_column: Option<i64>,
        /// An optional end line of the range covered by the goto target.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "endLine")]
        pub end_line: Option<i64>,
        /// Unique identifier for a goto target. This is used in the goto request.
        pub id: i64,
        /// The name of the goto target (shown in the UI).
        pub label: String,
        /// The line of the goto target.
        pub line: i64,
}
/// Arguments for 'gotoTargets' request.
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct GotoTargetsArguments {
        /// An optional column location for which the goto targets are determined.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub column: Option<i64>,
        /// The line location for which the goto targets are determined.
        pub line: i64,
        /// The source location for which the goto targets are determined.
        pub source: Source,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct GotoTargetsRequest {
        /// Object containing arguments for the command.
        pub arguments: GotoTargetsArguments,
        /// The command to execute.
        pub command: String,
        /// Sequence number.
        pub seq: i64,
        /// Message type.
        #[serde(rename = "type")]
        pub type_: String,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct GotoTargetsResponseBody {
        /// The possible goto targets of the specified location.
        pub targets: Vec<GotoTarget>,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct GotoTargetsResponse {
        /// Contains request result if success is true and optional error details if success is false.
        pub body: GotoTargetsResponseBody,
        /// The command requested.
        pub command: String,
        /// Contains error message if success == false.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub message: Option<String>,
        /// Sequence number of the corresponding request.
        pub request_seq: i64,
        /// Sequence number.
        pub seq: i64,
        /// Outcome of the request.
        pub success: bool,
        /// Message type.
        #[serde(rename = "type")]
        pub type_: String,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct InitializeRequest {
        /// Object containing arguments for the command.
        pub arguments: InitializeRequestArguments,
        /// The command to execute.
        pub command: String,
        /// Sequence number.
        pub seq: i64,
        /// Message type.
        #[serde(rename = "type")]
        pub type_: String,
}
/// Arguments for 'initialize' request.
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct InitializeRequestArguments {
        /// The ID of the debug adapter.
        #[serde(rename = "adapterID")]
        pub adapter_id: String,
        /// The ID of the (frontend) client using this adapter.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "clientID")]
        pub client_id: Option<String>,
        /// The human readable name of the (frontend) client using this adapter.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "clientName")]
        pub client_name: Option<String>,
        /// If true all column numbers are 1-based (default).
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "columnsStartAt1")]
        pub columns_start_at_1: Option<bool>,
        /// If true all line numbers are 1-based (default).
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "linesStartAt1")]
        pub lines_start_at_1: Option<bool>,
        /// The ISO-639 locale of the (frontend) client using this adapter, e.g. en-US or de-CH.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub locale: Option<String>,
        /// Determines in what format paths are specified. The default is 'path', which is the native
        /// format.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "pathFormat")]
        pub path_format: Option<String>,
        /// Client supports the runInTerminal request.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "supportsRunInTerminalRequest")]
        pub supports_run_in_terminal_request: Option<bool>,
        /// Client supports the paging of variables.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "supportsVariablePaging")]
        pub supports_variable_paging: Option<bool>,
        /// Client supports the optional type attribute for variables.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "supportsVariableType")]
        pub supports_variable_type: Option<bool>,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct InitializeResponse {
        /// The capabilities of this debug adapter.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub body: Option<Capabilities>,
        /// The command requested.
        pub command: String,
        /// Contains error message if success == false.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub message: Option<String>,
        /// Sequence number of the corresponding request.
        pub request_seq: i64,
        /// Sequence number.
        pub seq: i64,
        /// Outcome of the request.
        pub success: bool,
        /// Message type.
        #[serde(rename = "type")]
        pub type_: String,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct InitializedEvent {
        /// Event-specific information.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub body: Option<serde_json::Value>,
        /// Type of event.
        pub event: String,
        /// Sequence number.
        pub seq: i64,
        /// Message type.
        #[serde(rename = "type")]
        pub type_: String,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct LaunchRequest {
        /// Object containing arguments for the command.
        pub arguments: LaunchRequestArguments,
        /// The command to execute.
        pub command: String,
        /// Sequence number.
        pub seq: i64,
        /// Message type.
        #[serde(rename = "type")]
        pub type_: String,
}
/// Arguments for 'launch' request. Additional attributes are implementation specific.
#[derive(Clone, PartialEq, Debug, Default, Deserialize, Serialize)]
pub struct LaunchRequestArguments {
        /// Optional data from the previous, restarted session.
        /// The data is sent as the 'restart' attribute of the 'terminated' event.
        /// The client should leave the data intact.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub __restart: Option<serde_json::Value>,
        /// If noDebug is true the launch request should launch the program without enabling debugging.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "noDebug")]
        pub no_debug: Option<bool>,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct LaunchResponse {
        /// Contains request result if success is true and optional error details if success is false.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub body: Option<serde_json::Value>,
        /// The command requested.
        pub command: String,
        /// Contains error message if success == false.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub message: Option<String>,
        /// Sequence number of the corresponding request.
        pub request_seq: i64,
        /// Sequence number.
        pub seq: i64,
        /// Outcome of the request.
        pub success: bool,
        /// Message type.
        #[serde(rename = "type")]
        pub type_: String,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct LoadedSourceEventBody {
        /// The reason for the event.
        pub reason: String,
        /// The new, changed, or removed source.
        pub source: Source,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct LoadedSourceEvent {
        /// Event-specific information.
        pub body: LoadedSourceEventBody,
        /// Type of event.
        pub event: String,
        /// Sequence number.
        pub seq: i64,
        /// Message type.
        #[serde(rename = "type")]
        pub type_: String,
}
/// Arguments for 'loadedSources' request.
/// The 'loadedSources' request has no standardized arguments.
pub type LoadedSourcesArguments = ::std::collections::BTreeMap<String, serde_json::Value>;
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct LoadedSourcesRequest {
        /// Object containing arguments for the command.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub arguments: Option<LoadedSourcesArguments>,
        /// The command to execute.
        pub command: String,
        /// Sequence number.
        pub seq: i64,
        /// Message type.
        #[serde(rename = "type")]
        pub type_: String,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct LoadedSourcesResponseBody {
        /// Set of loaded sources.
        pub sources: Vec<Source>,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct LoadedSourcesResponse {
        /// Contains request result if success is true and optional error details if success is false.
        pub body: LoadedSourcesResponseBody,
        /// The command requested.
        pub command: String,
        /// Contains error message if success == false.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub message: Option<String>,
        /// Sequence number of the corresponding request.
        pub request_seq: i64,
        /// Sequence number.
        pub seq: i64,
        /// Outcome of the request.
        pub success: bool,
        /// Message type.
        #[serde(rename = "type")]
        pub type_: String,
}
/// A structured message object. Used to return errors from requests.
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct Message {
        /// A format string for the message. Embedded variables have the form '{name}'.
        /// If variable name starts with an underscore character, the variable does not contain user
        /// data (PII) and can be safely used for telemetry purposes.
        pub format: String,
        /// Unique identifier for the message.
        pub id: i64,
        /// If true send to telemetry.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "sendTelemetry")]
        pub send_telemetry: Option<bool>,
        /// If true show user.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "showUser")]
        pub show_user: Option<bool>,
        /// An optional url where additional information about this message can be found.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub url: Option<String>,
        /// An optional label that is presented to the user as the UI for opening the url.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "urlLabel")]
        pub url_label: Option<String>,
        /// An object used as a dictionary for looking up the variables in the format string.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub variables: Option<::std::collections::BTreeMap<String, String>>,
}
/// A Module object represents a row in the modules view.
/// Two attributes are mandatory: an id identifies a module in the modules view and is used in a
/// ModuleEvent for identifying a module for adding, updating or deleting.
/// The name is used to minimally render the module in the UI.
///
/// Additional attributes can be added to the module. They will show up in the module View if they
/// have a corresponding ColumnDescriptor.
///
/// To avoid an unnecessary proliferation of additional attributes with similar semantics but
/// different names
/// we recommend to re-use attributes from the 'recommended' list below first, and only introduce
/// new attributes if nothing appropriate could be found.
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct Module {
        /// Address range covered by this module.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "addressRange")]
        pub address_range: Option<String>,
        /// Module created or modified.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "dateTimeStamp")]
        pub date_time_stamp: Option<String>,
        /// Unique identifier for the module.
        pub id: serde_json::Value,
        /// True if the module is optimized.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "isOptimized")]
        pub is_optimized: Option<bool>,
        /// True if the module is considered 'user code' by a debugger that supports 'Just My Code'.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "isUserCode")]
        pub is_user_code: Option<bool>,
        /// A name of the module.
        pub name: String,
        /// optional but recommended attributes.
        /// always try to use these first before introducing additional attributes.
        ///
        /// Logical full path to the module. The exact definition is implementation defined, but
        /// usually this would be a full path to the on-disk file for the module.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub path: Option<String>,
        /// Logical full path to the symbol file. The exact definition is implementation defined.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "symbolFilePath")]
        pub symbol_file_path: Option<String>,
        /// User understandable description of if symbols were found for the module (ex: 'Symbols
        /// Loaded', 'Symbols not found', etc.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "symbolStatus")]
        pub symbol_status: Option<String>,
        /// Version of Module.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub version: Option<String>,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct ModuleEventBody {
        /// The new, changed, or removed module. In case of 'removed' only the module id is used.
        pub module: Module,
        /// The reason for the event.
        pub reason: String,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct ModuleEvent {
        /// Event-specific information.
        pub body: ModuleEventBody,
        /// Type of event.
        pub event: String,
        /// Sequence number.
        pub seq: i64,
        /// Message type.
        #[serde(rename = "type")]
        pub type_: String,
}
/// Arguments for 'modules' request.
#[derive(Clone, PartialEq, Debug, Default, Deserialize, Serialize)]
pub struct ModulesArguments {
        /// The number of modules to return. If moduleCount is not specified or 0, all modules are
        /// returned.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "moduleCount")]
        pub module_count: Option<i64>,
        /// The index of the first module to return; if omitted modules start at 0.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "startModule")]
        pub start_module: Option<i64>,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct ModulesRequest {
        /// Object containing arguments for the command.
        pub arguments: ModulesArguments,
        /// The command to execute.
        pub command: String,
        /// Sequence number.
        pub seq: i64,
        /// Message type.
        #[serde(rename = "type")]
        pub type_: String,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct ModulesResponseBody {
        /// All modules or range of modules.
        pub modules: Vec<Module>,
        /// The total number of modules available.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "totalModules")]
        pub total_modules: Option<i64>,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct ModulesResponse {
        /// Contains request result if success is true and optional error details if success is false.
        pub body: ModulesResponseBody,
        /// The command requested.
        pub command: String,
        /// Contains error message if success == false.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub message: Option<String>,
        /// Sequence number of the corresponding request.
        pub request_seq: i64,
        /// Sequence number.
        pub seq: i64,
        /// Outcome of the request.
        pub success: bool,
        /// Message type.
        #[serde(rename = "type")]
        pub type_: String,
}
/// The ModulesViewDescriptor is the container for all declarative configuration options of a
/// ModuleView.
/// For now it only specifies the columns to be shown in the modules view.
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct ModulesViewDescriptor {
        pub columns: Vec<ColumnDescriptor>,
}
/// Arguments for 'next' request.
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct NextArguments {
        /// Execute 'next' for this thread.
        #[serde(rename = "threadId")]
        pub thread_id: i64,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct NextRequest {
        /// Object containing arguments for the command.
        pub arguments: NextArguments,
        /// The command to execute.
        pub command: String,
        /// Sequence number.
        pub seq: i64,
        /// Message type.
        #[serde(rename = "type")]
        pub type_: String,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct NextResponse {
        /// Contains request result if success is true and optional error details if success is false.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub body: Option<serde_json::Value>,
        /// The command requested.
        pub command: String,
        /// Contains error message if success == false.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub message: Option<String>,
        /// Sequence number of the corresponding request.
        pub request_seq: i64,
        /// Sequence number.
        pub seq: i64,
        /// Outcome of the request.
        pub success: bool,
        /// Message type.
        #[serde(rename = "type")]
        pub type_: String,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct OutputEventBody {
        /// The output category. If not specified, 'console' is assumed.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub category: Option<String>,
        /// An optional source location column where the output was produced.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub column: Option<i64>,
        /// Optional data to report. For the 'telemetry' category the data will be sent to telemetry,
        /// for the other categories the data is shown in JSON format.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub data: Option<serde_json::Value>,
        /// An optional source location line where the output was produced.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub line: Option<i64>,
        /// The output to report.
        pub output: String,
        /// An optional source location where the output was produced.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub source: Option<Source>,
        /// If an attribute 'variablesReference' exists and its value is > 0, the output contains
        /// objects which can be retrieved by passing variablesReference to the VariablesRequest.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "variablesReference")]
        pub variables_reference: Option<f64>,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct OutputEvent {
        /// Event-specific information.
        pub body: OutputEventBody,
        /// Type of event.
        pub event: String,
        /// Sequence number.
        pub seq: i64,
        /// Message type.
        #[serde(rename = "type")]
        pub type_: String,
}
/// Arguments for 'pause' request.
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct PauseArguments {
        /// Pause execution for this thread.
        #[serde(rename = "threadId")]
        pub thread_id: i64,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct PauseRequest {
        /// Object containing arguments for the command.
        pub arguments: PauseArguments,
        /// The command to execute.
        pub command: String,
        /// Sequence number.
        pub seq: i64,
        /// Message type.
        #[serde(rename = "type")]
        pub type_: String,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct PauseResponse {
        /// Contains request result if success is true and optional error details if success is false.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub body: Option<serde_json::Value>,
        /// The command requested.
        pub command: String,
        /// Contains error message if success == false.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub message: Option<String>,
        /// Sequence number of the corresponding request.
        pub request_seq: i64,
        /// Sequence number.
        pub seq: i64,
        /// Outcome of the request.
        pub success: bool,
        /// Message type.
        #[serde(rename = "type")]
        pub type_: String,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct ProcessEventBody {
        /// If true, the process is running on the same computer as the debug adapter.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "isLocalProcess")]
        pub is_local_process: Option<bool>,
        /// The logical name of the process. This is usually the full path to process's executable
        /// file. Example: /home/example/myproj/program.js.
        pub name: String,
        /// Describes how the debug engine started debugging this process.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "startMethod")]
        pub start_method: Option<String>,
        /// The system process id of the debugged process. This property will be missing for non-system
        /// processes.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "systemProcessId")]
        pub system_process_id: Option<i64>,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct ProcessEvent {
        /// Event-specific information.
        pub body: ProcessEventBody,
        /// Type of event.
        pub event: String,
        /// Sequence number.
        pub seq: i64,
        /// Message type.
        #[serde(rename = "type")]
        pub type_: String,
}
/// Base class of requests, responses, and events.
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct ProtocolMessage {
        /// Sequence number.
        pub seq: i64,
        /// Message type.
        #[serde(rename = "type")]
        pub type_: String,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct Request {
        /// Object containing arguments for the command.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub arguments: Option<serde_json::Value>,
        /// The command to execute.
        pub command: String,
        /// Sequence number.
        pub seq: i64,
        /// Message type.
        #[serde(rename = "type")]
        pub type_: String,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct Response {
        /// Contains request result if success is true and optional error details if success is false.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub body: Option<serde_json::Value>,
        /// The command requested.
        pub command: String,
        /// Contains error message if success == false.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub message: Option<String>,
        /// Sequence number of the corresponding request.
        pub request_seq: i64,
        /// Sequence number.
        pub seq: i64,
        /// Outcome of the request.
        pub success: bool,
        /// Message type.
        #[serde(rename = "type")]
        pub type_: String,
}
/// Arguments for 'restart' request.
/// The restart request has no standardized attributes.
pub type RestartArguments = ::std::collections::BTreeMap<String, serde_json::Value>;
/// Arguments for 'restartFrame' request.
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct RestartFrameArguments {
        /// Restart this stackframe.
        #[serde(rename = "frameId")]
        pub frame_id: i64,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct RestartFrameRequest {
        /// Object containing arguments for the command.
        pub arguments: RestartFrameArguments,
        /// The command to execute.
        pub command: String,
        /// Sequence number.
        pub seq: i64,
        /// Message type.
        #[serde(rename = "type")]
        pub type_: String,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct RestartFrameResponse {
        /// Contains request result if success is true and optional error details if success is false.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub body: Option<serde_json::Value>,
        /// The command requested.
        pub command: String,
        /// Contains error message if success == false.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub message: Option<String>,
        /// Sequence number of the corresponding request.
        pub request_seq: i64,
        /// Sequence number.
        pub seq: i64,
        /// Outcome of the request.
        pub success: bool,
        /// Message type.
        #[serde(rename = "type")]
        pub type_: String,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct RestartRequest {
        /// Object containing arguments for the command.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub arguments: Option<RestartArguments>,
        /// The command to execute.
        pub command: String,
        /// Sequence number.
        pub seq: i64,
        /// Message type.
        #[serde(rename = "type")]
        pub type_: String,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct RestartResponse {
        /// Contains request result if success is true and optional error details if success is false.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub body: Option<serde_json::Value>,
        /// The command requested.
        pub command: String,
        /// Contains error message if success == false.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub message: Option<String>,
        /// Sequence number of the corresponding request.
        pub request_seq: i64,
        /// Sequence number.
        pub seq: i64,
        /// Outcome of the request.
        pub success: bool,
        /// Message type.
        #[serde(rename = "type")]
        pub type_: String,
}
/// Arguments for 'reverseContinue' request.
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct ReverseContinueArguments {
        /// Execute 'reverseContinue' for this thread.
        #[serde(rename = "threadId")]
        pub thread_id: i64,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct ReverseContinueRequest {
        /// Object containing arguments for the command.
        pub arguments: ReverseContinueArguments,
        /// The command to execute.
        pub command: String,
        /// Sequence number.
        pub seq: i64,
        /// Message type.
        #[serde(rename = "type")]
        pub type_: String,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct ReverseContinueResponse {
        /// Contains request result if success is true and optional error details if success is false.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub body: Option<serde_json::Value>,
        /// The command requested.
        pub command: String,
        /// Contains error message if success == false.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub message: Option<String>,
        /// Sequence number of the corresponding request.
        pub request_seq: i64,
        /// Sequence number.
        pub seq: i64,
        /// Outcome of the request.
        pub success: bool,
        /// Message type.
        #[serde(rename = "type")]
        pub type_: String,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct RunInTerminalRequest {
        /// Object containing arguments for the command.
        pub arguments: RunInTerminalRequestArguments,
        /// The command to execute.
        pub command: String,
        /// Sequence number.
        pub seq: i64,
        /// Message type.
        #[serde(rename = "type")]
        pub type_: String,
}
/// Arguments for 'runInTerminal' request.
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct RunInTerminalRequestArguments {
        /// List of arguments. The first argument is the command to run.
        pub args: Vec<String>,
        /// Working directory of the command.
        pub cwd: String,
        /// Environment key-value pairs that are added to or removed from the default environment.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub env: Option<::std::collections::BTreeMap<String, Option<String>>>,
        /// What kind of terminal to launch.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub kind: Option<String>,
        /// Optional title of the terminal.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub title: Option<String>,
}
#[derive(Clone, PartialEq, Debug, Default, Deserialize, Serialize)]
pub struct RunInTerminalResponseBody {
        /// The process ID.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "processId")]
        pub process_id: Option<f64>,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct RunInTerminalResponse {
        /// Contains request result if success is true and optional error details if success is false.
        pub body: RunInTerminalResponseBody,
        /// The command requested.
        pub command: String,
        /// Contains error message if success == false.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub message: Option<String>,
        /// Sequence number of the corresponding request.
        pub request_seq: i64,
        /// Sequence number.
        pub seq: i64,
        /// Outcome of the request.
        pub success: bool,
        /// Message type.
        #[serde(rename = "type")]
        pub type_: String,
}
/// A Scope is a named container for variables. Optionally a scope can map to a source or a range
/// within a source.
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct Scope {
        /// Optional start column of the range covered by this scope.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub column: Option<i64>,
        /// Optional end column of the range covered by this scope.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "endColumn")]
        pub end_column: Option<i64>,
        /// Optional end line of the range covered by this scope.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "endLine")]
        pub end_line: Option<i64>,
        /// If true, the number of variables in this scope is large or expensive to retrieve.
        pub expensive: bool,
        /// The number of indexed variables in this scope.
        /// The client can use this optional information to present the variables in a paged UI and
        /// fetch them in chunks.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "indexedVariables")]
        pub indexed_variables: Option<i64>,
        /// Optional start line of the range covered by this scope.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub line: Option<i64>,
        /// Name of the scope such as 'Arguments', 'Locals'.
        pub name: String,
        /// The number of named variables in this scope.
        /// The client can use this optional information to present the variables in a paged UI and
        /// fetch them in chunks.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "namedVariables")]
        pub named_variables: Option<i64>,
        /// Optional source for this scope.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub source: Option<Source>,
        /// The variables of this scope can be retrieved by passing the value of variablesReference to
        /// the VariablesRequest.
        #[serde(rename = "variablesReference")]
        pub variables_reference: i64,
}
/// Arguments for 'scopes' request.
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct ScopesArguments {
        /// Retrieve the scopes for this stackframe.
        #[serde(rename = "frameId")]
        pub frame_id: i64,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct ScopesRequest {
        /// Object containing arguments for the command.
        pub arguments: ScopesArguments,
        /// The command to execute.
        pub command: String,
        /// Sequence number.
        pub seq: i64,
        /// Message type.
        #[serde(rename = "type")]
        pub type_: String,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct ScopesResponseBody {
        /// The scopes of the stackframe. If the array has length zero, there are no scopes available.
        pub scopes: Vec<Scope>,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct ScopesResponse {
        /// Contains request result if success is true and optional error details if success is false.
        pub body: ScopesResponseBody,
        /// The command requested.
        pub command: String,
        /// Contains error message if success == false.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub message: Option<String>,
        /// Sequence number of the corresponding request.
        pub request_seq: i64,
        /// Sequence number.
        pub seq: i64,
        /// Outcome of the request.
        pub success: bool,
        /// Message type.
        #[serde(rename = "type")]
        pub type_: String,
}
/// Arguments for 'setBreakpoints' request.
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct SetBreakpointsArguments {
        /// The code locations of the breakpoints.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub breakpoints: Option<Vec<SourceBreakpoint>>,
        /// Deprecated: The code locations of the breakpoints.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub lines: Option<Vec<i64>>,
        /// The source location of the breakpoints; either source.path or source.reference must be
        /// specified.
        pub source: Source,
        /// A value of true indicates that the underlying source has been modified which results in new
        /// breakpoint locations.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "sourceModified")]
        pub source_modified: Option<bool>,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct SetBreakpointsRequest {
        /// Object containing arguments for the command.
        pub arguments: SetBreakpointsArguments,
        /// The command to execute.
        pub command: String,
        /// Sequence number.
        pub seq: i64,
        /// Message type.
        #[serde(rename = "type")]
        pub type_: String,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct SetBreakpointsResponseBody {
        /// Information about the breakpoints. The array elements are in the same order as the elements
        /// of the 'breakpoints' (or the deprecated 'lines') in the SetBreakpointsArguments.
        pub breakpoints: Vec<Breakpoint>,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct SetBreakpointsResponse {
        /// Contains request result if success is true and optional error details if success is false.
        pub body: SetBreakpointsResponseBody,
        /// The command requested.
        pub command: String,
        /// Contains error message if success == false.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub message: Option<String>,
        /// Sequence number of the corresponding request.
        pub request_seq: i64,
        /// Sequence number.
        pub seq: i64,
        /// Outcome of the request.
        pub success: bool,
        /// Message type.
        #[serde(rename = "type")]
        pub type_: String,
}
/// Arguments for 'setExceptionBreakpoints' request.
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct SetExceptionBreakpointsArguments {
        /// Configuration options for selected exceptions.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "exceptionOptions")]
        pub exception_options: Option<Vec<ExceptionOptions>>,
        /// IDs of checked exception options. The set of IDs is returned via the
        /// 'exceptionBreakpointFilters' capability.
        pub filters: Vec<String>,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct SetExceptionBreakpointsRequest {
        /// Object containing arguments for the command.
        pub arguments: SetExceptionBreakpointsArguments,
        /// The command to execute.
        pub command: String,
        /// Sequence number.
        pub seq: i64,
        /// Message type.
        #[serde(rename = "type")]
        pub type_: String,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct SetExceptionBreakpointsResponse {
        /// Contains request result if success is true and optional error details if success is false.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub body: Option<serde_json::Value>,
        /// The command requested.
        pub command: String,
        /// Contains error message if success == false.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub message: Option<String>,
        /// Sequence number of the corresponding request.
        pub request_seq: i64,
        /// Sequence number.
        pub seq: i64,
        /// Outcome of the request.
        pub success: bool,
        /// Message type.
        #[serde(rename = "type")]
        pub type_: String,
}
/// Arguments for 'setExpression' request.
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct SetExpressionArguments {
        /// The l-value expression to assign to.
        pub expression: String,
        /// Specifies how the resulting value should be formatted.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub format: Option<ValueFormat>,
        /// Evaluate the expressions in the scope of this stack frame. If not specified, the
        /// expressions are evaluated in the global scope.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "frameId")]
        pub frame_id: Option<i64>,
        /// The value expression to assign to the l-value expression.
        pub value: String,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct SetExpressionRequest {
        /// Object containing arguments for the command.
        pub arguments: SetExpressionArguments,
        /// The command to execute.
        pub command: String,
        /// Sequence number.
        pub seq: i64,
        /// Message type.
        #[serde(rename = "type")]
        pub type_: String,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct SetExpressionResponseBody {
        /// The number of indexed child variables.
        /// The client can use this optional information to present the variables in a paged UI and
        /// fetch them in chunks.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "indexedVariables")]
        pub indexed_variables: Option<f64>,
        /// The number of named child variables.
        /// The client can use this optional information to present the variables in a paged UI and
        /// fetch them in chunks.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "namedVariables")]
        pub named_variables: Option<f64>,
        /// Properties of a value that can be used to determine how to render the result in the UI.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "presentationHint")]
        pub presentation_hint: Option<VariablePresentationHint>,
        /// The optional type of the value.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "type")]
        pub type_: Option<String>,
        /// The new value of the expression.
        pub value: String,
        /// If variablesReference is > 0, the value is structured and its children can be retrieved by
        /// passing variablesReference to the VariablesRequest.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "variablesReference")]
        pub variables_reference: Option<f64>,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct SetExpressionResponse {
        /// Contains request result if success is true and optional error details if success is false.
        pub body: SetExpressionResponseBody,
        /// The command requested.
        pub command: String,
        /// Contains error message if success == false.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub message: Option<String>,
        /// Sequence number of the corresponding request.
        pub request_seq: i64,
        /// Sequence number.
        pub seq: i64,
        /// Outcome of the request.
        pub success: bool,
        /// Message type.
        #[serde(rename = "type")]
        pub type_: String,
}
/// Arguments for 'setFunctionBreakpoints' request.
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct SetFunctionBreakpointsArguments {
        /// The function names of the breakpoints.
        pub breakpoints: Vec<FunctionBreakpoint>,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct SetFunctionBreakpointsRequest {
        /// Object containing arguments for the command.
        pub arguments: SetFunctionBreakpointsArguments,
        /// The command to execute.
        pub command: String,
        /// Sequence number.
        pub seq: i64,
        /// Message type.
        #[serde(rename = "type")]
        pub type_: String,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct SetFunctionBreakpointsResponseBody {
        /// Information about the breakpoints. The array elements correspond to the elements of the
        /// 'breakpoints' array.
        pub breakpoints: Vec<Breakpoint>,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct SetFunctionBreakpointsResponse {
        /// Contains request result if success is true and optional error details if success is false.
        pub body: SetFunctionBreakpointsResponseBody,
        /// The command requested.
        pub command: String,
        /// Contains error message if success == false.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub message: Option<String>,
        /// Sequence number of the corresponding request.
        pub request_seq: i64,
        /// Sequence number.
        pub seq: i64,
        /// Outcome of the request.
        pub success: bool,
        /// Message type.
        #[serde(rename = "type")]
        pub type_: String,
}
/// Arguments for 'setVariable' request.
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct SetVariableArguments {
        /// Specifies details on how to format the response value.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub format: Option<ValueFormat>,
        /// The name of the variable.
        pub name: String,
        /// The value of the variable.
        pub value: String,
        /// The reference of the variable container.
        #[serde(rename = "variablesReference")]
        pub variables_reference: i64,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct SetVariableRequest {
        /// Object containing arguments for the command.
        pub arguments: SetVariableArguments,
        /// The command to execute.
        pub command: String,
        /// Sequence number.
        pub seq: i64,
        /// Message type.
        #[serde(rename = "type")]
        pub type_: String,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct SetVariableResponseBody {
        /// The number of indexed child variables.
        /// The client can use this optional information to present the variables in a paged UI and
        /// fetch them in chunks.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "indexedVariables")]
        pub indexed_variables: Option<f64>,
        /// The number of named child variables.
        /// The client can use this optional information to present the variables in a paged UI and
        /// fetch them in chunks.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "namedVariables")]
        pub named_variables: Option<f64>,
        /// The type of the new value. Typically shown in the UI when hovering over the value.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "type")]
        pub type_: Option<String>,
        /// The new value of the variable.
        pub value: String,
        /// If variablesReference is > 0, the new value is structured and its children can be retrieved
        /// by passing variablesReference to the VariablesRequest.
        #[serde(rename = "variablesReference")]
        pub variables_reference: i64,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct SetVariableResponse {
        /// Contains request result if success is true and optional error details if success is false.
        pub body: SetVariableResponseBody,
        /// The command requested.
        pub command: String,
        /// Contains error message if success == false.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub message: Option<String>,
        /// Sequence number of the corresponding request.
        pub request_seq: i64,
        /// Sequence number.
        pub seq: i64,
        /// Outcome of the request.
        pub success: bool,
        /// Message type.
        #[serde(rename = "type")]
        pub type_: String,
}
/// A Source is a descriptor for source code. It is returned from the debug adapter as part of a
/// StackFrame and it is used by clients when specifying breakpoints.
#[derive(Clone, PartialEq, Debug, Default, Deserialize, Serialize)]
pub struct Source {
        /// Optional data that a debug adapter might want to loop through the client. The client should
        /// leave the data intact and persist it across sessions. The client should not interpret the
        /// data.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "adapterData")]
        pub adapter_data: Option<serde_json::Value>,
        /// The checksums associated with this file.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub checksums: Option<Vec<Checksum>>,
        /// The short name of the source. Every source returned from the debug adapter has a name. When
        /// sending a source to the debug adapter this name is optional.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub name: Option<String>,
        /// The (optional) origin of this source: possible values 'internal module', 'inlined content
        /// from source map', etc.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub origin: Option<String>,
        /// The path of the source to be shown in the UI. It is only used to locate and load the
        /// content of the source if no sourceReference is specified (or its value is 0).
        #[serde(skip_serializing_if = "Option::is_none")]
        pub path: Option<String>,
        /// An optional hint for how to present the source in the UI. A value of 'deemphasize' can be
        /// used to indicate that the source is not available or that it is skipped on stepping.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "presentationHint")]
        pub presentation_hint: Option<String>,
        /// If sourceReference > 0 the contents of the source must be retrieved through the
        /// SourceRequest (even if a path is specified). A sourceReference is only valid for a session,
        /// so it must not be used to persist a source.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "sourceReference")]
        pub source_reference: Option<i64>,
        /// An optional list of sources that are related to this source. These may be the source that
        /// generated this source.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub sources: Option<Vec<Source>>,
}
/// Arguments for 'source' request.
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct SourceArguments {
        /// Specifies the source content to load. Either source.path or source.sourceReference must be
        /// specified.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub source: Option<Source>,
        /// The reference to the source. This is the same as source.sourceReference. This is provided
        /// for backward compatibility since old backends do not understand the 'source' attribute.
        #[serde(rename = "sourceReference")]
        pub source_reference: i64,
}
/// Properties of a breakpoint or logpoint passed to the setBreakpoints request.
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct SourceBreakpoint {
        /// An optional source column of the breakpoint.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub column: Option<i64>,
        /// An optional expression for conditional breakpoints.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub condition: Option<String>,
        /// An optional expression that controls how many hits of the breakpoint are ignored. The
        /// backend is expected to interpret the expression as needed.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "hitCondition")]
        pub hit_condition: Option<String>,
        /// The source line of the breakpoint or logpoint.
        pub line: i64,
        /// If this attribute exists and is non-empty, the backend must not 'break' (stop) but log the
        /// message instead. Expressions within {} are interpolated.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "logMessage")]
        pub log_message: Option<String>,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct SourceRequest {
        /// Object containing arguments for the command.
        pub arguments: SourceArguments,
        /// The command to execute.
        pub command: String,
        /// Sequence number.
        pub seq: i64,
        /// Message type.
        #[serde(rename = "type")]
        pub type_: String,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct SourceResponseBody {
        /// Content of the source reference.
        pub content: String,
        /// Optional content type (mime type) of the source.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "mimeType")]
        pub mime_type: Option<String>,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct SourceResponse {
        /// Contains request result if success is true and optional error details if success is false.
        pub body: SourceResponseBody,
        /// The command requested.
        pub command: String,
        /// Contains error message if success == false.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub message: Option<String>,
        /// Sequence number of the corresponding request.
        pub request_seq: i64,
        /// Sequence number.
        pub seq: i64,
        /// Outcome of the request.
        pub success: bool,
        /// Message type.
        #[serde(rename = "type")]
        pub type_: String,
}
/// A Stackframe contains the source location.
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct StackFrame {
        /// The column within the line. If source is null or doesn't exist, column is 0 and must be
        /// ignored.
        pub column: i64,
        /// An optional end column of the range covered by the stack frame.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "endColumn")]
        pub end_column: Option<i64>,
        /// An optional end line of the range covered by the stack frame.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "endLine")]
        pub end_line: Option<i64>,
        /// An identifier for the stack frame. It must be unique across all threads. This id can be
        /// used to retrieve the scopes of the frame with the 'scopesRequest' or to restart the
        /// execution of a stackframe.
        pub id: i64,
        /// The line within the file of the frame. If source is null or doesn't exist, line is 0 and
        /// must be ignored.
        pub line: i64,
        /// The module associated with this frame, if any.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "moduleId")]
        pub module_id: Option<serde_json::Value>,
        /// The name of the stack frame, typically a method name.
        pub name: String,
        /// An optional hint for how to present this frame in the UI. A value of 'label' can be used to
        /// indicate that the frame is an artificial frame that is used as a visual label or separator.
        /// A value of 'subtle' can be used to change the appearance of a frame in a 'subtle' way.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "presentationHint")]
        pub presentation_hint: Option<String>,
        /// The optional source of the frame.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub source: Option<Source>,
}
#[derive(Clone, PartialEq, Debug, Default, Deserialize, Serialize)]
pub struct StackFrameFormat {
        /// Display the value in hex.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub hex: Option<bool>,
        /// Includes all stack frames, including those the debug adapter might otherwise hide.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "includeAll")]
        pub include_all: Option<bool>,
        /// Displays the line number of the stack frame.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub line: Option<bool>,
        /// Displays the module of the stack frame.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub module: Option<bool>,
        /// Displays the names of parameters for the stack frame.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "parameterNames")]
        pub parameter_names: Option<bool>,
        /// Displays the types of parameters for the stack frame.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "parameterTypes")]
        pub parameter_types: Option<bool>,
        /// Displays the values of parameters for the stack frame.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "parameterValues")]
        pub parameter_values: Option<bool>,
        /// Displays parameters for the stack frame.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub parameters: Option<bool>,
}
/// Arguments for 'stackTrace' request.
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct StackTraceArguments {
        /// Specifies details on how to format the stack frames.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub format: Option<StackFrameFormat>,
        /// The maximum number of frames to return. If levels is not specified or 0, all frames are
        /// returned.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub levels: Option<i64>,
        /// The index of the first frame to return; if omitted frames start at 0.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "startFrame")]
        pub start_frame: Option<i64>,
        /// Retrieve the stacktrace for this thread.
        #[serde(rename = "threadId")]
        pub thread_id: i64,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct StackTraceRequest {
        /// Object containing arguments for the command.
        pub arguments: StackTraceArguments,
        /// The command to execute.
        pub command: String,
        /// Sequence number.
        pub seq: i64,
        /// Message type.
        #[serde(rename = "type")]
        pub type_: String,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct StackTraceResponseBody {
        /// The frames of the stackframe. If the array has length zero, there are no stackframes
        /// available.
        /// This means that there is no location information available.
        #[serde(rename = "stackFrames")]
        pub stack_frames: Vec<StackFrame>,
        /// The total number of frames available.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "totalFrames")]
        pub total_frames: Option<i64>,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct StackTraceResponse {
        /// Contains request result if success is true and optional error details if success is false.
        pub body: StackTraceResponseBody,
        /// The command requested.
        pub command: String,
        /// Contains error message if success == false.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub message: Option<String>,
        /// Sequence number of the corresponding request.
        pub request_seq: i64,
        /// Sequence number.
        pub seq: i64,
        /// Outcome of the request.
        pub success: bool,
        /// Message type.
        #[serde(rename = "type")]
        pub type_: String,
}
/// Arguments for 'stepBack' request.
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct StepBackArguments {
        /// Execute 'stepBack' for this thread.
        #[serde(rename = "threadId")]
        pub thread_id: i64,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct StepBackRequest {
        /// Object containing arguments for the command.
        pub arguments: StepBackArguments,
        /// The command to execute.
        pub command: String,
        /// Sequence number.
        pub seq: i64,
        /// Message type.
        #[serde(rename = "type")]
        pub type_: String,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct StepBackResponse {
        /// Contains request result if success is true and optional error details if success is false.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub body: Option<serde_json::Value>,
        /// The command requested.
        pub command: String,
        /// Contains error message if success == false.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub message: Option<String>,
        /// Sequence number of the corresponding request.
        pub request_seq: i64,
        /// Sequence number.
        pub seq: i64,
        /// Outcome of the request.
        pub success: bool,
        /// Message type.
        #[serde(rename = "type")]
        pub type_: String,
}
/// Arguments for 'stepIn' request.
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct StepInArguments {
        /// Optional id of the target to step into.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "targetId")]
        pub target_id: Option<i64>,
        /// Execute 'stepIn' for this thread.
        #[serde(rename = "threadId")]
        pub thread_id: i64,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct StepInRequest {
        /// Object containing arguments for the command.
        pub arguments: StepInArguments,
        /// The command to execute.
        pub command: String,
        /// Sequence number.
        pub seq: i64,
        /// Message type.
        #[serde(rename = "type")]
        pub type_: String,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct StepInResponse {
        /// Contains request result if success is true and optional error details if success is false.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub body: Option<serde_json::Value>,
        /// The command requested.
        pub command: String,
        /// Contains error message if success == false.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub message: Option<String>,
        /// Sequence number of the corresponding request.
        pub request_seq: i64,
        /// Sequence number.
        pub seq: i64,
        /// Outcome of the request.
        pub success: bool,
        /// Message type.
        #[serde(rename = "type")]
        pub type_: String,
}
/// A StepInTarget can be used in the 'stepIn' request and determines into which single target the
/// stepIn request should step.
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct StepInTarget {
        /// Unique identifier for a stepIn target.
        pub id: i64,
        /// The name of the stepIn target (shown in the UI).
        pub label: String,
}
/// Arguments for 'stepInTargets' request.
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct StepInTargetsArguments {
        /// The stack frame for which to retrieve the possible stepIn targets.
        #[serde(rename = "frameId")]
        pub frame_id: i64,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct StepInTargetsRequest {
        /// Object containing arguments for the command.
        pub arguments: StepInTargetsArguments,
        /// The command to execute.
        pub command: String,
        /// Sequence number.
        pub seq: i64,
        /// Message type.
        #[serde(rename = "type")]
        pub type_: String,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct StepInTargetsResponseBody {
        /// The possible stepIn targets of the specified source location.
        pub targets: Vec<StepInTarget>,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct StepInTargetsResponse {
        /// Contains request result if success is true and optional error details if success is false.
        pub body: StepInTargetsResponseBody,
        /// The command requested.
        pub command: String,
        /// Contains error message if success == false.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub message: Option<String>,
        /// Sequence number of the corresponding request.
        pub request_seq: i64,
        /// Sequence number.
        pub seq: i64,
        /// Outcome of the request.
        pub success: bool,
        /// Message type.
        #[serde(rename = "type")]
        pub type_: String,
}
/// Arguments for 'stepOut' request.
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct StepOutArguments {
        /// Execute 'stepOut' for this thread.
        #[serde(rename = "threadId")]
        pub thread_id: i64,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct StepOutRequest {
        /// Object containing arguments for the command.
        pub arguments: StepOutArguments,
        /// The command to execute.
        pub command: String,
        /// Sequence number.
        pub seq: i64,
        /// Message type.
        #[serde(rename = "type")]
        pub type_: String,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct StepOutResponse {
        /// Contains request result if success is true and optional error details if success is false.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub body: Option<serde_json::Value>,
        /// The command requested.
        pub command: String,
        /// Contains error message if success == false.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub message: Option<String>,
        /// Sequence number of the corresponding request.
        pub request_seq: i64,
        /// Sequence number.
        pub seq: i64,
        /// Outcome of the request.
        pub success: bool,
        /// Message type.
        #[serde(rename = "type")]
        pub type_: String,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct StoppedEventBody {
        /// If allThreadsStopped is true, a debug adapter can announce that all threads have stopped.
        /// *  The client should use this information to enable that all threads can be expanded to
        /// access their stacktraces.
        /// *  If the attribute is missing or false, only the thread with the given threadId can be
        /// expanded.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "allThreadsStopped")]
        pub all_threads_stopped: Option<bool>,
        /// The full reason for the event, e.g. 'Paused on exception'. This string is shown in the UI
        /// as is.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub description: Option<String>,
        /// A value of true hints to the frontend that this event should not change the focus.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "preserveFocusHint")]
        pub preserve_focus_hint: Option<bool>,
        /// The reason for the event.
        /// For backward compatibility this string is shown in the UI if the 'description' attribute is
        /// missing (but it must not be translated).
        pub reason: String,
        /// Additional information. E.g. if reason is 'exception', text contains the exception name.
        /// This string is shown in the UI.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub text: Option<String>,
        /// The thread which was stopped.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "threadId")]
        pub thread_id: Option<i64>,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct StoppedEvent {
        /// Event-specific information.
        pub body: StoppedEventBody,
        /// Type of event.
        pub event: String,
        /// Sequence number.
        pub seq: i64,
        /// Message type.
        #[serde(rename = "type")]
        pub type_: String,
}
/// Arguments for 'terminateThreads' request.
#[derive(Clone, PartialEq, Debug, Default, Deserialize, Serialize)]
pub struct TerminateThreadsArguments {
        /// Ids of threads to be terminated.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "threadIds")]
        pub thread_ids: Option<Vec<i64>>,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct TerminateThreadsRequest {
        /// Object containing arguments for the command.
        pub arguments: TerminateThreadsArguments,
        /// The command to execute.
        pub command: String,
        /// Sequence number.
        pub seq: i64,
        /// Message type.
        #[serde(rename = "type")]
        pub type_: String,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct TerminateThreadsResponse {
        /// Contains request result if success is true and optional error details if success is false.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub body: Option<serde_json::Value>,
        /// The command requested.
        pub command: String,
        /// Contains error message if success == false.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub message: Option<String>,
        /// Sequence number of the corresponding request.
        pub request_seq: i64,
        /// Sequence number.
        pub seq: i64,
        /// Outcome of the request.
        pub success: bool,
        /// Message type.
        #[serde(rename = "type")]
        pub type_: String,
}
#[derive(Clone, PartialEq, Debug, Default, Deserialize, Serialize)]
pub struct TerminatedEventBody {
        /// A debug adapter may set 'restart' to true (or to an arbitrary object) to request that the
        /// front end restarts the session.
        /// The value is not interpreted by the client and passed unmodified as an attribute
        /// '__restart' to the 'launch' and 'attach' requests.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub restart: Option<serde_json::Value>,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct TerminatedEvent {
        /// Event-specific information.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub body: Option<TerminatedEventBody>,
        /// Type of event.
        pub event: String,
        /// Sequence number.
        pub seq: i64,
        /// Message type.
        #[serde(rename = "type")]
        pub type_: String,
}
/// A Thread
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct Thread {
        /// Unique identifier for the thread.
        pub id: i64,
        /// A name of the thread.
        pub name: String,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct ThreadEventBody {
        /// The reason for the event.
        pub reason: String,
        /// The identifier of the thread.
        #[serde(rename = "threadId")]
        pub thread_id: i64,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct ThreadEvent {
        /// Event-specific information.
        pub body: ThreadEventBody,
        /// Type of event.
        pub event: String,
        /// Sequence number.
        pub seq: i64,
        /// Message type.
        #[serde(rename = "type")]
        pub type_: String,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct ThreadsRequest {
        /// Object containing arguments for the command.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub arguments: Option<serde_json::Value>,
        /// The command to execute.
        pub command: String,
        /// Sequence number.
        pub seq: i64,
        /// Message type.
        #[serde(rename = "type")]
        pub type_: String,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct ThreadsResponseBody {
        /// All threads.
        pub threads: Vec<Thread>,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct ThreadsResponse {
        /// Contains request result if success is true and optional error details if success is false.
        pub body: ThreadsResponseBody,
        /// The command requested.
        pub command: String,
        /// Contains error message if success == false.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub message: Option<String>,
        /// Sequence number of the corresponding request.
        pub request_seq: i64,
        /// Sequence number.
        pub seq: i64,
        /// Outcome of the request.
        pub success: bool,
        /// Message type.
        #[serde(rename = "type")]
        pub type_: String,
}
/// Provides formatting information for a value.
#[derive(Clone, PartialEq, Debug, Default, Deserialize, Serialize)]
pub struct ValueFormat {
        /// Display the value in hex.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub hex: Option<bool>,
}
/// A Variable is a name/value pair.
/// Optionally a variable can have a 'type' that is shown if space permits or when hovering over
/// the variable's name.
/// An optional 'kind' is used to render additional properties of the variable, e.g. different
/// icons can be used to indicate that a variable is public or private.
/// If the value is structured (has children), a handle is provided to retrieve the children with
/// the VariablesRequest.
/// If the number of named or indexed children is large, the numbers should be returned via the
/// optional 'namedVariables' and 'indexedVariables' attributes.
/// The client can use this optional information to present the children in a paged UI and fetch
/// them in chunks.
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct Variable {
        /// Optional evaluatable name of this variable which can be passed to the 'EvaluateRequest' to
        /// fetch the variable's value.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "evaluateName")]
        pub evaluate_name: Option<String>,
        /// The number of indexed child variables.
        /// The client can use this optional information to present the children in a paged UI and
        /// fetch them in chunks.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "indexedVariables")]
        pub indexed_variables: Option<i64>,
        /// The variable's name.
        pub name: String,
        /// The number of named child variables.
        /// The client can use this optional information to present the children in a paged UI and
        /// fetch them in chunks.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "namedVariables")]
        pub named_variables: Option<i64>,
        /// Properties of a variable that can be used to determine how to render the variable in the
        /// UI.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "presentationHint")]
        pub presentation_hint: Option<VariablePresentationHint>,
        /// The type of the variable's value. Typically shown in the UI when hovering over the value.
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(rename = "type")]
        pub type_: Option<String>,
        /// The variable's value. This can be a multi-line text, e.g. for a function the body of a
        /// function.
        pub value: String,
        /// If variablesReference is > 0, the variable is structured and its children can be retrieved
        /// by passing variablesReference to the VariablesRequest.
        #[serde(rename = "variablesReference")]
        pub variables_reference: i64,
}
/// Optional properties of a variable that can be used to determine how to render the variable in
/// the UI.
#[derive(Clone, PartialEq, Debug, Default, Deserialize, Serialize)]
pub struct VariablePresentationHint {
        /// Set of attributes represented as an array of strings. Before introducing additional values,
        /// try to use the listed values.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub attributes: Option<Vec<String>>,
        /// The kind of variable. Before introducing additional values, try to use the listed values.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub kind: Option<String>,
        /// Visibility of variable. Before introducing additional values, try to use the listed values.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub visibility: Option<String>,
}
/// Arguments for 'variables' request.
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct VariablesArguments {
        /// The number of variables to return. If count is missing or 0, all variables are returned.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub count: Option<i64>,
        /// Optional filter to limit the child variables to either named or indexed. If ommited, both
        /// types are fetched.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub filter: Option<String>,
        /// Specifies details on how to format the Variable values.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub format: Option<ValueFormat>,
        /// The index of the first variable to return; if omitted children start at 0.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub start: Option<i64>,
        /// The Variable reference.
        #[serde(rename = "variablesReference")]
        pub variables_reference: i64,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct VariablesRequest {
        /// Object containing arguments for the command.
        pub arguments: VariablesArguments,
        /// The command to execute.
        pub command: String,
        /// Sequence number.
        pub seq: i64,
        /// Message type.
        #[serde(rename = "type")]
        pub type_: String,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct VariablesResponseBody {
        /// All (or a range) of variables for the given variable reference.
        pub variables: Vec<Variable>,
}
#[derive(Clone, PartialEq, Debug, Deserialize, Serialize)]
pub struct VariablesResponse {
        /// Contains request result if success is true and optional error details if success is false.
        pub body: VariablesResponseBody,
        /// The command requested.
        pub command: String,
        /// Contains error message if success == false.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub message: Option<String>,
        /// Sequence number of the corresponding request.
        pub request_seq: i64,
        /// Sequence number.
        pub seq: i64,
        /// Outcome of the request.
        pub success: bool,
        /// Message type.
        #[serde(rename = "type")]
        pub type_: String,
}
