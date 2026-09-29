use adapter_protocol::Expressions;
use lldb::{SBExecutionContext, SBFrame, SBTarget, SBValue};
use std::sync::Arc;

use crate::prelude::{str_error, Error};
use crate::python::{EvalContext, PyObject, PythonSession};

#[macro_use]
pub mod prelude {
    use nom::{
        character::complete::{digit1, space0},
        error::ParseError,
        sequence::delimited,
        AsChar, Input, Parser,
    };

    pub use crate::error::{str_error, Error};

    pub type Span<'a> = &'a str;

    pub use nom::IResult;
    //pub type IResult<I, O, E = nom::error::VerboseError<I>> = Result<(I, O), nom::Err<E>>;

    pub fn ws<I, O, E: ParseError<I>, P>(parser: P) -> impl Parser<I, Output = O, Error = E>
    where
        P: Parser<I, Output = O, Error = E>,
        I: Input,
        <I as Input>::Item: AsChar + Clone,
    {
        delimited(space0, parser, space0)
    }

    pub fn unsigned(input: Span) -> IResult<Span, u32> {
        let (rest, s) = digit1(input)?;
        Ok((rest, parse_int::parse::<u32>(s).unwrap()))
    }

    #[cfg(test)]
    macro_rules! assert_matches(($e:expr, $p:pat) => { let e = $e; assert!(matches!(e, $p), "{:?} !~ {}", e, stringify!($p)) });

    #[cfg(test)]
    macro_rules! test_parser {
        ($parser:expr, $input:expr, $expected_parsed:literal, $expected_rest: literal) => {
            match $parser.parse($input) {
                Ok((rest, parsed)) if parsed == $expected_parsed && rest == $expected_rest => {}
                Ok((rest, parsed)) => panic!(
                    "Expected to parse \"{}\" as \"{}\", \"{}\", got \"{}\", \"{}\"",
                    $input, $expected_parsed, $expected_rest, parsed, rest
                ),
                Err(e) => panic!("Parsing of \"{}\" failed: {}", $input, e),
            }
        };
        ($parser:expr, $input:expr, $parsed:literal) => {
            test_parser!($parser, $input, $parsed, "")
        };
    }
}

mod expression_format;
mod hit_condition;
mod literals;
mod preprocess;
mod qualified_ident;
mod simple_expressions;

pub use expression_format::{get_expression_format, FormatSpec};
pub use hit_condition::HitCondition;
pub use preprocess::{preprocess_python_expr, preprocess_simple_expr};

pub enum PreparedExpression {
    Native(String),
    Python {
        code: PyObject,
        eval_context: EvalContext,
        python: Arc<PythonSession>,
    },
}

impl PreparedExpression {
    // Parse, preprocess and, for Python-based expressions, compile an expression.
    pub fn prepare(
        expression: &str,
        default_type: Expressions,
        python: Option<&Arc<PythonSession>>,
        filename: &str,
    ) -> Result<Self, Error> {
        let (expr, ty) = get_expression_type(expression, default_type);
        Self::prepare_typed(expr, ty, python, filename)
    }

    // Same as prepare(), but also parses formatting options at the end of expression.
    pub fn prepare_with_format(
        expression: &str,
        default_type: Expressions,
        python: Option<&Arc<PythonSession>>,
        filename: &str,
    ) -> Result<(Self, FormatSpec), Error> {
        let (expr, ty) = get_expression_type(expression, default_type);
        let (expr, format_spec) = get_expression_format(expr)?;
        let expression = Self::prepare_typed(expr, ty, python, filename)?;
        Ok((expression, format_spec))
    }

    fn prepare_typed(
        expression: &str,
        expression_type: Expressions,
        python: Option<&Arc<PythonSession>>,
        filename: &str,
    ) -> Result<Self, Error> {
        let (expression, eval_context) = match expression_type {
            Expressions::Native => return Ok(Self::Native(expression.to_owned())),
            Expressions::Simple => (preprocess_simple_expr(expression)?, EvalContext::SimpleExpression),
            Expressions::Python => (preprocess_python_expr(expression)?, EvalContext::PythonExpression),
        };
        let python = python.ok_or_else(|| str_error("Python expressions are disabled."))?;
        let code = python.compile_code(&expression, filename)?;
        Ok(Self::Python {
            code,
            eval_context,
            python: python.clone(),
        })
    }

    pub fn native_source(&self) -> Option<&str> {
        match self {
            Self::Native(expr) => Some(expr),
            _ => None,
        }
    }

    pub fn evaluate_in_frame(&self, frame: &SBFrame) -> Result<SBValue, Error> {
        match self {
            Self::Native(expr) => Ok(frame.evaluate_expression(expr).into_result()?),
            Self::Python {
                code,
                python,
                eval_context,
            } => Ok(python.evaluate(code, &SBExecutionContext::from_frame(frame), *eval_context)?),
        }
    }

    // Preserve the existing frameless behavior: native expressions use the target,
    // while Python-based expressions receive an empty execution context.
    pub fn evaluate_in_target(&self, target: &SBTarget) -> Result<SBValue, Error> {
        match self {
            Self::Native(expr) => Ok(target.evaluate_expression(expr).into_result()?),
            Self::Python {
                code,
                python,
                eval_context,
            } => Ok(python.evaluate(code, &SBExecutionContext::from_target(target), *eval_context)?),
        }
    }

    pub fn evaluate_as_bool_in_frame(&self, frame: &SBFrame) -> Result<bool, Error> {
        let exec_context = SBExecutionContext::from_frame(frame);
        match self {
            Self::Native(expr) => {
                let value = frame.evaluate_expression(expr).into_result()?;
                Ok(value.value_as_unsigned(0) != 0)
            }
            Self::Python {
                code,
                python,
                eval_context,
            } => Ok(python.evaluate_as_bool(code, &exec_context, *eval_context)?),
        }
    }
}

fn get_expression_type<'a>(expr: &'a str, default_type: Expressions) -> (&'a str, Expressions) {
    if expr.starts_with("/nat ") {
        (&expr[5..], Expressions::Native)
    } else if expr.starts_with("/py ") {
        (&expr[4..], Expressions::Python)
    } else if expr.starts_with("/se ") {
        (&expr[4..], Expressions::Simple)
    } else {
        (expr, default_type)
    }
}
