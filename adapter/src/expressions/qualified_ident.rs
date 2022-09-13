use nom::{
    branch::alt,
    bytes::complete::{is_not, tag},
    character::complete::{alpha1, alphanumeric1, space0},
    combinator::{opt, recognize},
    multi::{many0_count, separated_list0, separated_list1},
    sequence::{delimited, pair, preceded, terminated},
};

use super::prelude::*;

pub type QIdent<'a> = Vec<QIdentSegment<'a>>;

#[derive(Debug, PartialEq, Clone)]
pub struct QIdentSegment<'a> {
    ident: &'a str,
    parameters: Vec<QIdentParam<'a>>,
}

#[derive(Debug, PartialEq, Clone)]
pub enum QIdentParam<'a> {
    QIdent(QIdent<'a>),
    Other(&'a str),
}

pub fn ident(input: Span) -> IResult<Span, Span> {
    recognize(pair(alt((alpha1, tag("_"))), many0_count(alt((alphanumeric1, tag("_"))))))(input)
}

fn template_param(input: Span) -> IResult<Span, QIdentParam> {
    match qualified_ident(input) {
        Ok((rest, result)) => Ok((rest, QIdentParam::QIdent(result))),
        Err(_) => match recognize(is_not("<,>"))(input) {
            Ok((rest, result)) => Ok((rest, QIdentParam::Other(result.trim()))),
            Err(err) => Err(err),
        },
    }
}

fn template_params(input: Span) -> IResult<Span, Vec<QIdentParam>> {
    let (rest, parameters) = delimited(tag("<"), separated_list0(tag(","), ws(template_param)), tag(">"))(input)?;
    Ok((rest, parameters))
}

fn qident_segment(input: Span) -> IResult<Span, QIdentSegment> {
    let (rest, (ident, parameters)) = pair(ident, opt(preceded(space0, template_params)))(input)?;
    let parameters = match parameters {
        Some(parameters) => parameters,
        None => Vec::new(),
    };
    Ok((
        rest,
        QIdentSegment {
            ident,
            parameters,
        },
    ))
}

pub fn qualified_ident(input: Span) -> IResult<Span, QIdent> {
    preceded(opt(terminated(tag("::"), space0)), separated_list1(ws(tag("::")), qident_segment))(input)
}

///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

#[test]
fn test_ident() {
    assert_eq!(ident("ident1234"), Ok(("", "ident1234")));
    assert_eq!(ident("_ident_1234::"), Ok(("::", "_ident_1234")));
}

#[test]
fn test_qident_segment() {
    assert_eq!(
        qident_segment("string"),
        Ok((
            "",
            QIdentSegment {
                ident: "string",
                parameters: vec![],
            }
        ))
    );

    assert_eq!(
        qident_segment("string<42>"),
        Ok((
            "",
            QIdentSegment {
                ident: "string",
                parameters: vec![QIdentParam::Other("42")]
            }
        ))
    );

    assert_eq!(
        qident_segment("string < 42 >"),
        Ok((
            "",
            QIdentSegment {
                ident: "string",
                parameters: vec![QIdentParam::Other("42")]
            }
        ))
    );

    assert_eq!(
        qident_segment("string < 2, 3.0 >"),
        Ok((
            "",
            QIdentSegment {
                ident: "string",
                parameters: vec![QIdentParam::Other("2"), QIdentParam::Other("3.0")]
            }
        ))
    );
}

#[test]
fn test_qualified_ident() {
    let expected = vec![
        QIdentSegment {
            ident: "foo",
            parameters: vec![],
        },
        QIdentSegment {
            ident: "bar",
            parameters: vec![
                QIdentParam::QIdent(vec![
                    QIdentSegment {
                        ident: "baz",
                        parameters: vec![],
                    },
                    QIdentSegment {
                        ident: "quox",
                        parameters: vec![QIdentParam::Other("3")],
                    },
                ]),
                QIdentParam::Other("1 +0.234"),
            ],
        },
    ];

    assert_eq!(qualified_ident("foo :: bar < baz::quox< 3>, 1 +0.234>"), Ok(("", expected.clone())));
    assert_eq!(qualified_ident(":: foo :: bar < baz::quox< 3>, 1 +0.234>"), Ok(("", expected)));
}
