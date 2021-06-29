use nom::{bytes::complete::tag, sequence::delimited, IResult, Parser};

use super::prelude::*;

#[derive(Debug, Clone)]
pub enum FormatSpec {
    Format(lldb::Format),
    Array(u32),
}

pub fn get_expression_format<'a>(expr: &'a str) -> Result<(&'a str, Option<FormatSpec>), String> {
    fn array_spec(input: Span) -> IResult<Span, u32> {
        delimited(tag("["), unsigned, tag("]"))(input)
    }

    if let Some(pos) = expr.rfind(',') {
        let spec = &expr[pos + 1..];
        let expr = &expr[..pos];
        if let Ok(("", n)) = array_spec.parse(spec) {
            return Ok((expr, Some(FormatSpec::Array(n))));
        } else if spec.len() == 1 {
            let f = match spec {
                "c" => lldb::Format::Char,
                "h" => lldb::Format::Hex,
                "x" => lldb::Format::Hex,
                "o" => lldb::Format::Octal,
                "d" => lldb::Format::Decimal,
                "b" => lldb::Format::Binary,
                "f" => lldb::Format::Float,
                "p" => lldb::Format::Pointer,
                "u" => lldb::Format::Unsigned,
                "s" => lldb::Format::CString,
                "y" => lldb::Format::Bytes,
                "Y" => lldb::Format::BytesWithASCII,
                _ => bail!(format!("Invalid format specifier: {}", spec)),
            };
            return Ok((expr, Some(FormatSpec::Format(f))));
        }
    }
    Ok((expr, None))
}

///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

#[test]
fn test_expression_format() {
    assert_matches!(get_expression_format("foo"), Ok(("foo", None)));
    assert_matches!(get_expression_format("foo,bar"), Ok(("foo,bar", None)));

    assert_matches!(get_expression_format("foo,h"), Ok(("foo", Some(FormatSpec::Format(lldb::Format::Hex)))));
    assert_matches!(get_expression_format("foo,x"), Ok(("foo", Some(FormatSpec::Format(lldb::Format::Hex)))));
    assert_matches!(get_expression_format("foo,y"), Ok(("foo", Some(FormatSpec::Format(lldb::Format::Bytes)))));
    assert_matches!(
        get_expression_format("foo,Y"),
        Ok(("foo", Some(FormatSpec::Format(lldb::Format::BytesWithASCII))))
    );

    assert_matches!(get_expression_format("foo,[42]"), Ok(("foo", Some(FormatSpec::Array(42)))));

    assert_matches!(get_expression_format("foo,Z"), Err(_));
}
