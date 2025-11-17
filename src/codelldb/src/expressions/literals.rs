use std::borrow::Cow;

use nom::{
    branch::alt,
    bytes::complete::tag,
    character::complete::{digit1, hex_digit1, one_of, satisfy},
    combinator::{not, recognize, verify},
    multi::many1,
    number::complete::recognize_float,
    sequence::{pair, terminated},
    Parser,
};

use super::prelude::*;

fn float_literal(input: Span) -> IResult<Span, Span> {
    verify(recognize_float, |lexeme: &Span| {
        lexeme.contains('.') || lexeme.contains('e') || lexeme.contains('E')
    })
    .parse(input)
}

fn binary_literal(input: Span) -> IResult<Span, Span> {
    recognize(pair(alt((tag("0b"), tag("0B"))), many1(one_of("01")))).parse(input)
}

fn octal_literal(input: Span) -> IResult<Span, Span> {
    recognize(pair(alt((tag("0o"), tag("0O"))), many1(one_of("01234567")))).parse(input)
}

fn hex_literal(input: Span) -> IResult<Span, Span> {
    recognize(pair(alt((tag("0x"), tag("0X"))), hex_digit1)).parse(input)
}

fn decimal_literal(input: Span) -> IResult<Span, Span> {
    digit1.parse(input)
}

pub fn integer_literal(input: Span) -> IResult<Span, Span> {
    alt((hex_literal, binary_literal, octal_literal, decimal_literal)).parse(input)
}

pub fn numeric_literal(input: Span) -> IResult<Span, Span> {
    alt((float_literal, integer_literal)).parse(input)
}

pub fn boolean_literal(input: Span) -> IResult<Span, Cow<str>> {
    terminated(
        alt((
            alt((tag("true"), tag("True"))).map(|_| Cow::from("True")),
            alt((tag("false"), tag("False"))).map(|_| Cow::from("False")),
        )),
        not(satisfy(|c| c.is_ascii_alphanumeric() || c == '_')),
    )
    .parse(input)
}

#[test]
fn decimal_literal_test() {
    test_parser!(decimal_literal, "3 ", "3", " ");
    test_parser!(decimal_literal, "12  ", "12", "  ");
    test_parser!(decimal_literal, "24", "24");
    test_parser!(decimal_literal, "537  ", "537", "  ");
}

#[test]
fn float_literal_test() {
    test_parser!(float_literal, "3.", "3.");
    test_parser!(float_literal, "3.14", "3.14");
    test_parser!(float_literal, ".14", ".14");
    test_parser!(float_literal, "6.02e23", "6.02e23");
    test_parser!(float_literal, "6E-12", "6E-12");
}

#[test]
fn numeric_literal_test() {
    test_parser!(numeric_literal, "3.", "3.");
    test_parser!(numeric_literal, "3.14", "3.14");
    test_parser!(numeric_literal, ".14", ".14");
    test_parser!(numeric_literal, "6.02e23", "6.02e23");
    test_parser!(numeric_literal, "6E-12", "6E-12");

    test_parser!(numeric_literal, "3 ", "3", " ");
    test_parser!(numeric_literal, "12  ", "12", "  ");
    test_parser!(numeric_literal, "24", "24");
    test_parser!(numeric_literal, "537  ", "537", "  ");
    test_parser!(numeric_literal, "0xff", "0xff");
    test_parser!(numeric_literal, "0B1010", "0B1010");
    test_parser!(numeric_literal, "0o12", "0o12");
}

#[test]
fn boolean_literal_test() {
    test_parser!(boolean_literal, "true", "True");
    test_parser!(boolean_literal, "True foo", "True", " foo");
    test_parser!(boolean_literal, "false", "False");
    assert!(boolean_literal.parse("trueValue").is_err());
}
