use std::io::Write;
use std::{env, fs::File};

use codelldb_types::*;
use schemars::{schema_for, JsonSchema};

#[derive(JsonSchema)]
pub enum TypeLibrary {
    _AdapterSettings(AdapterSettings),
    _ExcludeCallerRequest(ExcludeCallerRequest),
    _ExcludeCallerResponse(ExcludeCallerResponse),
    _SetExcludedCallersRequest(SetExcludedCallersRequest),
    _SymbolsRequest(SymbolsRequest),
    _SymbolsResponse(SymbolsResponse),
    _CommonLaunchFields(CommonLaunchFields),
    _LaunchRequestArguments(LaunchRequestArguments),
    _AttachRequestArguments(AttachRequestArguments),
    _LaunchEnvironment(LaunchEnvironment),
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = env::args().collect::<Vec<_>>();
    let mut output: Box<dyn Write> = if args.len() > 1 {
        Box::new(File::create(&args[1])?) //.
    } else {
        Box::new(std::io::stdout())
    };
    let schema = schema_for!(TypeLibrary);
    write!(&mut output, "{}", serde_json::to_string_pretty(&schema)?)?;
    Ok(())
}
