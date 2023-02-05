use nom::{
    character::complete::{anychar, char},
    combinator::{opt, verify},
    sequence::{delimited, pair},
};

use super::prelude::*;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FormatSpec {
    pub format: Option<lldb::Format>,
    pub array: Option<u32>,
}

impl FormatSpec {
    pub const NONE: FormatSpec = FormatSpec {
        format: None,
        array: None,
    };
}

pub fn get_expression_format<'a>(expr: &'a str) -> Result<(&'a str, FormatSpec), String> {
    if let Some(pos) = expr.rfind(',') {
        let spec = &expr[pos + 1..];

        fn convert_format(c: char) -> Option<lldb::Format> {
            match c {
                'c' => Some(lldb::Format::Char),
                'h' => Some(lldb::Format::Hex),
                'x' => Some(lldb::Format::Hex),
                'o' => Some(lldb::Format::Octal),
                'd' => Some(lldb::Format::Decimal),
                'b' => Some(lldb::Format::Binary),
                'f' => Some(lldb::Format::Float),
                'p' => Some(lldb::Format::Pointer),
                'u' => Some(lldb::Format::Unsigned),
                's' => Some(lldb::Format::CString),
                'y' => Some(lldb::Format::Bytes),
                'Y' => Some(lldb::Format::BytesWithASCII),
                _ => None,
            }
        }

        let mut parser = pair(
            opt(verify(anychar, |c| c.is_alphabetic())),
            opt(delimited(char('['), unsigned, char(']'))),
        );

        match parser(spec) {
            // Fully parsed
            Ok(("", (format_ch, array))) => {
                let format = match format_ch {
                    Some(c) => match convert_format(c) {
                        Some(format) => Some(format),
                        None => return Err(format!("Invlaid format specifier: {}", c)),
                    },
                    _ => None,
                };

                Ok((&expr[..pos], FormatSpec { format, array }))
            }
            // Partially parsed
            Ok(_) => Ok((expr, FormatSpec::NONE)),
            // Error
            Err(err) => Err(err.to_string()),
        }
    } else {
        // No format specifier, return expression as-is
        Ok((expr, FormatSpec::NONE))
    }
}

///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

#[test]
#[rustfmt::skip::macros(assert_matches)]
fn test_expression_format() {
    assert_matches!(get_expression_format("foo"), Ok(("foo", FormatSpec::NONE)));
    assert_matches!(get_expression_format("foo,bar"), Ok(("foo,bar", FormatSpec::NONE)));

    assert_matches!(get_expression_format("foo,h"), Ok(("foo", FormatSpec { format: Some(lldb::Format::Hex), array: None})));
    assert_matches!(get_expression_format("foo,x"), Ok(("foo", FormatSpec { format: Some(lldb::Format::Hex), array: None})));
    assert_matches!(get_expression_format("foo,y"), Ok(("foo", FormatSpec { format: Some(lldb::Format::Bytes), array: None})));
    assert_matches!(get_expression_format("foo,Y"), Ok(("foo", FormatSpec { format: Some(lldb::Format::BytesWithASCII), array: None})));

    assert_matches!(get_expression_format("foo,[42]"), Ok(("foo", FormatSpec{ format: None, array: Some(42) })));
    assert_matches!(get_expression_format("foo,x[42]"), Ok(("foo", FormatSpec{ format:Some(lldb::Format::Hex), array: Some(42), })));
    assert_matches!(get_expression_format("foo, x"), Ok(("foo, x", FormatSpec::NONE)));
    assert_matches!(get_expression_format("foo,x [42]"), Ok(("foo,x [42]", FormatSpec::NONE)));

    assert_matches!(get_expression_format("foo,Z"), Err(_));
}
