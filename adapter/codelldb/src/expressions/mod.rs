use adapter_protocol::Expressions;

#[macro_use]
pub mod prelude {
    use nom::{
        character::complete::{digit1, space0},
        error::ParseError,
        sequence::delimited,
        AsChar, InputTakeAtPosition, Parser,
    };

    pub use crate::error::Error;

    pub type Span<'a> = &'a str;

    pub use nom::IResult;
    //pub type IResult<I, O, E = nom::error::VerboseError<I>> = Result<(I, O), nom::Err<E>>;

    pub fn ws<I, O, E: ParseError<I>, P>(parser: P) -> impl FnMut(I) -> IResult<I, O, E>
    where
        P: Parser<I, O, E>,
        I: InputTakeAtPosition,
        <I as InputTakeAtPosition>::Item: AsChar + Clone,
    {
        delimited(space0, parser, space0)
    }

    pub fn unsigned(input: Span) -> IResult<Span, u32> {
        let (rest, s) = digit1(input)?;
        Ok((rest, parse_int::parse::<u32>(s).unwrap()))
    }

    #[cfg(test)]
    macro_rules! assert_matches(($e:expr, $p:pat) => { let e = $e; assert!(matches!(e, $p), "{:?} !~ {}", e, stringify!($p)) });
}

mod expression_format;
mod hit_condition;
mod preprocess;
mod qualified_ident;
mod simple_expressions;

pub use expression_format::{get_expression_format, FormatSpec};
pub use hit_condition::{parse_hit_condition, HitCondition};
pub use preprocess::{preprocess_python_expr, preprocess_simple_expr};

#[derive(Debug)]
pub enum PreparedExpression {
    Native(String),
    Simple(String),
    Python(String),
}

// Parse expression type and preprocess it.
pub fn prepare(expression: &str, default_type: Expressions) -> Result<PreparedExpression, prelude::Error> {
    let (expr, ty) = get_expression_type(expression, default_type);
    match ty {
        Expressions::Native => Ok(PreparedExpression::Native(expr.to_owned())),
        Expressions::Simple => Ok(PreparedExpression::Simple(preprocess_simple_expr(expr)?)),
        Expressions::Python => Ok(PreparedExpression::Python(preprocess_python_expr(expr)?)),
    }
}

// Same as prepare(), but also parses formatting options at the end of expression,
// for example, `value,x` to format value as hex or `ptr,[50]` to interpret `ptr` as an array of 50 elements.
pub fn prepare_with_format(
    expression: &str,
    default_type: Expressions,
) -> Result<(PreparedExpression, FormatSpec), prelude::Error> {
    let (expr, ty) = get_expression_type(expression, default_type);
    let (expr, format_spec) = get_expression_format(expr)?;
    let pp_expr = match ty {
        Expressions::Native => PreparedExpression::Native(expr.to_owned()),
        Expressions::Simple => PreparedExpression::Simple(preprocess_simple_expr(expr)?),
        Expressions::Python => PreparedExpression::Python(preprocess_python_expr(expr)?),
    };
    Ok((pp_expr, format_spec))
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
