use lldb::*;

#[derive(Debug)]
pub enum PythonValue {
    SBValue(SBValue),
    Int(i64),
    Bool(bool),
    String(String),
    Object(String),
}

pub type Error = Box<dyn std::error::Error>;

pub trait EventSink {
    fn display_html(&self, html: String, title: Option<String>, position: Option<i32>, reveal: bool);
}

pub trait PythonInterface {
    fn evaluate(&self, expr: &str, is_simple_expr: bool, context: &SBExecutionContext) -> Result<SBValue, String>;
    fn evaluate_as_bool(&self, expr: &str, is_simple_expr: bool, context: &SBExecutionContext) -> Result<bool, String>;
    fn modules_loaded(&self, modules: &mut dyn Iterator<Item = &SBModule>);
}

pub type Entry = fn() -> Result<(), Error>;

pub type NewSession = fn(
    interpreter: SBCommandInterpreter,
    event_sink: Box<dyn EventSink + Send>,
) -> Result<Box<dyn PythonInterface>, Error>;
