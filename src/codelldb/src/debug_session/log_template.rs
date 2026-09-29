use crate::expressions::{FormatSpec, PreparedExpression};
use crate::python::PythonSession;

use super::value_formatter::ValueFormatter;
use adapter_protocol::Expressions;
use lldb::*;
use std::sync::Arc;

pub(super) struct LogTemplate(Vec<LogSegment>);

enum LogSegment {
    Text(String),
    Expression(String, PreparedExpression, FormatSpec),
}

impl LogTemplate {
    pub(super) fn prepare(
        message: &str,
        default_type: Expressions,
        python: Option<&Arc<PythonSession>>,
    ) -> Result<Self, String> {
        let mut segments = Vec::new();
        let mut depth = 0;
        let mut opening = 0;
        let mut expression_start = 0;
        let mut literal_start = 0;
        for (index, character) in message.char_indices() {
            match character {
                '{' if depth == 0 => {
                    opening = index;
                    expression_start = index + 1;
                    depth = 1;
                }
                '{' => depth += 1,
                '}' if depth > 0 => {
                    depth -= 1;
                    if depth == 0 {
                        if literal_start < opening {
                            segments.push(LogSegment::Text(message[literal_start..opening].to_owned()));
                        }
                        let source = &message[expression_start..index];
                        let (expression, format_spec) = PreparedExpression::prepare_with_format(
                            source,
                            default_type,
                            python,
                            "<logpoint expression>",
                        )
                        .map_err(|error| format!("Invalid logpoint expression {{{source}}}: {error}"))?;
                        segments.push(LogSegment::Expression(source.to_owned(), expression, format_spec));
                        literal_start = index + 1;
                    }
                }
                _ => {}
            }
        }
        if literal_start < message.len() {
            segments.push(LogSegment::Text(message[literal_start..].to_owned()));
        }
        Ok(Self(segments))
    }

    pub(super) fn render(&self, frame: &SBFrame, formatter: &ValueFormatter) -> Result<String, String> {
        let mut message = String::new();
        for segment in &self.0 {
            match segment {
                LogSegment::Text(text) => message.push_str(text),
                LogSegment::Expression(source, expression, format_spec) => {
                    let value = (|| -> Result<String, String> {
                        let value = expression.evaluate_in_frame(frame).map_err(|err| err.to_string())?;
                        let value = formatter.apply_format_spec(value, format_spec).map_err(|err| err.to_string())?;
                        Ok(formatter.get_var_summary(&value, false))
                    })()
                    .map_err(|error| format!("Could not evaluate logpoint expression {{{source}}}: {error}"))?;
                    message.push_str(&value);
                }
            }
        }
        Ok(message.trim_end_matches('\n').to_owned())
    }
}

#[test]
fn test_log_template_preparation() {
    let template = LogTemplate::prepare("a {fn({x})} b", Expressions::Native, None).unwrap();
    assert_eq!(template.0.len(), 3);
    assert!(matches!(&template.0[0], LogSegment::Text(text) if text == "a "));
    assert!(
        matches!(&template.0[1], LogSegment::Expression(source, PreparedExpression::Native(expr), _)
        if source == "fn({x})" && expr == "fn({x})")
    );
    assert!(matches!(&template.0[2], LogSegment::Text(text) if text == " b"));

    let template = LogTemplate::prepare("{/nat value,x} tail {", Expressions::Simple, None).unwrap();
    assert_eq!(template.0.len(), 2);
    assert!(
        matches!(&template.0[0], LogSegment::Expression(_, PreparedExpression::Native(expr), spec)
        if expr == "value" && spec.format == Some(Format::Hex))
    );
    assert!(matches!(&template.0[1], LogSegment::Text(text) if text == " tail {"));

    let error = LogTemplate::prepare("{/nat value,Z}", Expressions::Native, None).err().unwrap();
    assert!(error.contains("Invalid logpoint expression {/nat value,Z}"));
    assert!(error.contains("format specifier"));
}
