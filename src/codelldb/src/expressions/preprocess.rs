use nom::{
    branch::alt,
    bytes::complete::{is_not, tag},
    character::complete::{anychar, char, none_of},
    combinator::recognize,
    multi::{fold_many0, many0_count},
    sequence::{delimited, pair, preceded},
    Finish, Parser,
};
use std::{borrow::Cow, fmt::Write};

use super::prelude::*;
use super::qualified_ident::qualified_ident;

// Recognize Python strings
pub fn python_string(input: Span) -> IResult<Span, Span> {
    fn body(delim: &'static str) -> impl Fn(Span) -> IResult<Span, Span> {
        move |input| {
            recognize(many0_count(alt((
                recognize(pair(char('\\'), anychar)), //.
                recognize(none_of(delim)),
            ))))
            .parse(input)
        }
    }
    recognize(alt((
        delimited(char('\"'), body("\""), char('\"')),
        delimited(char('\''), body("\'"), char('\'')),
        delimited(tag("r\""), is_not("\""), char('"')),
        delimited(tag("r\'"), is_not("\'"), char('\'')),
    )))
    .parse(input)
}

// Recognize $-prefixed native expressions, such as "$foo", "$foo::bar", "${foo::bar + 3}"
pub fn native_expr(input: Span) -> IResult<Span, Span> {
    preceded(
        char('$'),
        alt((
            recognize(qualified_ident), //.
            delimited(tag("{"), recognize(is_not("}")), tag("}")),
        )),
    )
    .parse(input)
}

// Translates a Simple Expression into a Python expression.
pub fn preprocess_simple_expr(expr: &str) -> Result<String, Error> {
    match super::simple_expressions::expression(expr).finish() {
        Ok(("", result)) => Ok(result.into_owned()),
        Ok((input, _)) => Err(syntax_error_message(expr, input).into()),
        Err(e) => Err(syntax_error_message(expr, e.input).into()),
    }
}

// Replaces embedded native expressions with calls to __eval().
pub fn preprocess_python_expr(expr: &str) -> Result<String, Error> {
    #[rustfmt::skip]
    fn parser(input: Span) -> IResult<Span, String> {
        fold_many0(
            alt((
                recognize(python_string).map(Into::into),
                native_expr.map(|i| format!("__eval('{}')", i).into()),
                recognize(anychar).map(Into::into),
            )),
            move || String::new(),
            |mut acc, item:Cow<str>| {
                acc.push_str(item.as_ref());
                acc
            },
        )
        .parse(input)
    }

    match parser(expr).finish() {
        Ok(("", result)) => Ok(result),
        Ok((input, _)) => Err(syntax_error_message(expr, input).into()),
        Err(e) => Err(syntax_error_message(expr, e.input).into()),
    }
}

fn syntax_error_message(input: &str, tail: &str) -> String {
    let mut message = String::new();
    log_errors!(write!(message, "Syntax error: "));
    let prefix_len = message.len();
    log_errors!(write!(message, "{}\n", input));
    for _ in 0..(prefix_len + input.len() - tail.len()) {
        log_errors!(write!(message, " "));
    }
    log_errors!(write!(message, "^"));
    message
}

///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

#[test]
fn test_python_string() {
    // Empty
    assert_eq!(python_string.parse(r#""""#), Ok(("", r#""""#)));
    assert_eq!(python_string.parse(r#"''"#), Ok(("", "''")));
    // Opposite type of quote
    assert_eq!(python_string.parse(r#""a'aa""#), Ok(("", r#""a'aa""#)));
    assert_eq!(python_string.parse(r#"'a"aa'"#), Ok(("", r#"'a"aa'"#)));
    // Escape
    assert_eq!(python_string.parse(r#"'aaa\aaaa'"#), Ok(("", r#"'aaa\aaaa'"#)));
    // Quote escape
    assert_eq!(python_string.parse(r#"'aaa\'aaa'"#), Ok(("", r#"'aaa\'aaa'"#)));
    // Raw string
    assert_eq!(python_string.parse(r#"r"aaa\aaa""#), Ok(("", r#"r"aaa\aaa""#)));
    assert_eq!(python_string.parse(r#"r'aaa\aaa'"#), Ok(("", r#"r'aaa\aaa'"#)));
}

#[cfg(test)]
fn test_pair(input: &str, expected: &str, preprocessor: impl Fn(&str) -> Result<String, Error>) {
    {
        let prepr = preprocessor(input).unwrap();
        assert_eq!(prepr, expected);
    }
    {
        let input = format!("   {}", input);
        let expected = format!("   {}", expected);
        let prepr = preprocessor(&input).unwrap();
        assert_eq!(prepr, expected);
    }
    {
        let input = format!("{}   ", input);
        let expected = format!("{}   ", expected);
        let prepr = preprocessor(&input).unwrap();
        assert_eq!(prepr, expected);
    }
}

#[test]
fn test_python_expr() {
    #[rustfmt::skip]
    let pairs = [
        (
            r#"for x in $foo: print x"#,
            r#"for x in __eval('foo'): print x"#
        ),
        (
            r#"$xxx.yyy.zzz"#,
            r#"__eval('xxx').yyy.zzz"#
        ),
        (
            r#"$xxx::yyy::zzz"#,
            r#"__eval('xxx::yyy::zzz')"#
        ),
        (
            r#"$::xxx"#,
            r#"__eval('::xxx')"#
        ),
        (
            r#"${foo::bar::baz}"#,
            r#"__eval('foo::bar::baz')"#
        ),
        (
            r#" "$xxx::yyy::zzz"  "#,
            r#" "$xxx::yyy::zzz"  "#
        ),
    ];
    for (expr, expected) in pairs.iter() {
        test_pair(expr, expected, &preprocess_python_expr);
    }
}
