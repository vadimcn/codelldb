use std::io::Write;
use std::{env, fs::File};

use codelldb_types::*;
use schemars::{schema_for, JsonSchema};

#[derive(JsonSchema)]
pub enum TypeLibrary {
    AdapterSettings(AdapterSettings),
    ExcludeCallerRequest(ExcludeCallerRequest),
    ExcludeCallerResponse(ExcludeCallerResponse),
    SetExcludedCallersRequest(SetExcludedCallersRequest),
    SymbolsRequest(SymbolsRequest),
    SymbolsResponse(SymbolsResponse),
    CommonLaunchFields(CommonLaunchFields),
    LaunchRequestArguments(LaunchRequestArguments),
    AttachRequestArguments(AttachRequestArguments),
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
