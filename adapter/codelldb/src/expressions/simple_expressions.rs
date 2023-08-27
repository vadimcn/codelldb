use std::borrow::Cow;
use std::ops::RangeTo;

use nom::{
    branch::alt,
    bytes::complete::tag,
    character::complete::char,
    character::complete::{digit0, digit1},
    combinator::{map, opt, recognize},
    error::ParseError,
    multi::fold_many0,
    sequence::{delimited, pair, preceded},
    AsChar, InputLength, InputTakeAtPosition, Offset, Parser, Slice,
};

use super::prelude::*;
use super::preprocess::{native_expr, python_string};
use super::qualified_ident::{ident, qualified_ident};

fn python_number(input: Span) -> IResult<Span, Span> {
    alt((
        recognize(pair(digit1, opt(pair(tag("."), digit0)))),
        recognize(pair(tag("."), digit1)),
    ))(input)
}

// Parse <term> [<op> <term> [<op> <term> [...]]]
fn binary_op<'a, I, O1, E: ParseError<I>>(
    operator: impl Parser<I, O1, E>,
    mut term: impl Parser<I, Cow<'a, str>, E>,
) -> impl FnOnce(I) -> IResult<I, Cow<'a, str>, E>
where
    I: 'a,
    I: Clone + Offset + Slice<RangeTo<usize>> + InputLength,
    I: AsRef<str>,
    I: InputTakeAtPosition,
    <I as InputTakeAtPosition>::Item: AsChar + Clone,
    O1: AsRef<str>,
{
    move |input: I| {
        let (input, init) = term.parse(input)?;
        fold_many0(
            pair(ws(operator), term),
            move || init.to_string(),
            |mut acc: String, (op, val): (O1, Cow<str>)| {
                acc.push_str(op.as_ref());
                acc.push_str(val.as_ref());
                acc
            },
        )(input)
        .map(|(input, res)| (input, Cow::from(res)))
    }
}

fn disjunction(input: Span) -> IResult<Span, Cow<str>> {
    binary_op(tag("or").map(|_| " or "), conjunction)(input)
}

fn conjunction(input: Span) -> IResult<Span, Cow<str>> {
    binary_op(tag("and").map(|_| " and "), inversion)(input)
}

fn inversion(input: Span) -> IResult<Span, Cow<str>> {
    alt((
        preceded(ws(tag("not")), inversion) //
            .map(|inv| format!("not {}", inv).into()),
        comparison,
    ))(input)
}

fn comparison(input: Span) -> IResult<Span, Cow<str>> {
    binary_op(
        alt((tag("=="), tag("!="), tag(">="), tag(">"), tag("<="), tag("<"))),
        bitwise_or,
    )(input)
}

fn bitwise_or(input: Span) -> IResult<Span, Cow<str>> {
    binary_op(tag("|"), bitwise_xor)(input)
}

fn bitwise_xor(input: Span) -> IResult<Span, Cow<str>> {
    binary_op(tag("^"), bitwise_and)(input)
}

fn bitwise_and(input: Span) -> IResult<Span, Cow<str>> {
    binary_op(tag("&"), bitwise_shift)(input)
}

fn bitwise_shift(input: Span) -> IResult<Span, Cow<str>> {
    binary_op(alt((tag("<<"), tag(">>"))), addsub)(input)
}

fn addsub(input: Span) -> IResult<Span, Cow<str>> {
    binary_op(alt((tag("+"), tag("-"))), muldiv)(input)
}

fn muldiv(input: Span) -> IResult<Span, Cow<str>> {
    binary_op(alt((tag("*"), tag("//"), tag("/"), tag("%"))), unary)(input)
}

fn unary(input: Span) -> IResult<Span, Cow<str>> {
    alt((
        pair(ws(alt((char('-'), char('+'), char('~')))), power) //
            .map(|(op, pow)| format!("{}{}", op, pow).into()),
        power,
    ))(input)
}

fn power(input: Span) -> IResult<Span, Cow<str>> {
    binary_op(tag("**"), primary)(input)
}

fn primary(input: Span) -> IResult<Span, Cow<str>> {
    let (input, init) = atom(input)?;
    fold_many0(
        alt((
            preceded(ws(char('.')), alt((ident, recognize(unsigned))))
                .map(|id| format!(".__getattr__('{}')", id).into()),
            delimited(ws(char('[')), expression, ws(char(']'))) //
                .map(|e| format!("[{}]", e).into()),
        )),
        move || init.to_string(),
        |mut acc, item: Cow<str>| {
            acc.push_str(item.as_ref());
            acc
        },
    )(input)
    .map(|(input, res)| (input, Cow::from(res)))
}

fn atom(i: Span) -> IResult<Span, Cow<str>> {
    ws(alt((
        python_number.map(Into::into),
        python_string.map(Into::into),
        tag("True").map(Into::into),
        tag("False").map(Into::into),
        recognize(qualified_ident).map(|e| format!("__eval('{}')", e).into()),
        native_expr.map(|e| format!("__eval('{}')", e).into()),
        group,
    )))(i)
}

fn group(input: Span) -> IResult<Span, Cow<str>> {
    map(delimited(char('('), expression, char(')')), |e| {
        format!("({})", e).into()
    })(input)
}

pub fn expression(input: Span) -> IResult<Span, Cow<str>> {
    ws(disjunction)(input)
}

#[cfg(test)]
#[rustfmt::skip::macros(test_parser)]
mod test {
    macro_rules! test_parser {
        ($parser:ident, $input:literal, $expected:literal) => {
            assert_eq!($parser($input), Ok(("", $expected.into())))
        };
    }

    use super::*;
    #[test]
    fn atom_test() {
        test_parser!(atom, "3", "3");
        test_parser!(atom, " 12", "12");
        test_parser!(atom, "537  ", "537");
        test_parser!(atom, "  24   ", "24");
        test_parser!(atom, "  3. ", "3.");
        test_parser!(atom, "  3.14 ", "3.14");
        test_parser!(atom, "  .14 ", ".14");
        test_parser!(atom, " foo", "__eval('foo')");
        test_parser!(atom, " foo::bar  ", "__eval('foo::bar')");
        test_parser!(atom, " $foo::bar  ", "__eval('foo::bar')");
        test_parser!(atom, "  ${foo::bar + 3}  ", "__eval('foo::bar + 3')");
        test_parser!(atom, " 'st\ring'  ", "'st\ring'");
        test_parser!(atom, " \"string\"  ", "\"string\"");
        test_parser!(atom, "  std::numeric_limits<float>::digits ", "__eval('std::numeric_limits<float>::digits')");
    }

    #[test]
    fn primary_test() {
        test_parser!(primary, " foo . bar",
                              "__eval('foo').__getattr__('bar')");
        test_parser!(primary, " foo . 0",
                              "__eval('foo').__getattr__('0')");
        test_parser!(primary, " foo . 0 . bar . 42",
                              "__eval('foo').__getattr__('0').__getattr__('bar').__getattr__('42')");
        test_parser!(primary, " foo::foo . bar . baz",
                              "__eval('foo::foo').__getattr__('bar').__getattr__('baz')");
        test_parser!(primary, " foo::foo2 . bar [ 32 ] . baz",
                              "__eval('foo::foo2').__getattr__('bar')[32].__getattr__('baz')");
    }

    #[test]
    fn expression_test() {
        test_parser!(expression, " 12 *2 /  3", "12*2/3");
        test_parser!(expression, " 2* 3  *2 *2 /  3", "2*3*2*2/3");
        test_parser!(expression, " 48 /  3/2", "48/3/2");
        test_parser!(expression, " 3 *foo/ 4 ", "3*__eval('foo')/4");
        test_parser!(expression, " 3 *'foo'/ 4 ", "3*'foo'/4");
        test_parser!(expression, " 3 *foo::bar/ 4 ", "3*__eval('foo::bar')/4");
        test_parser!(expression, " 3 *${foo::bar - 13}/ 4 ", "3*__eval('foo::bar - 13')/4");
        test_parser!(expression, " 1 +  2 ", "1+2");
        test_parser!(expression, " 12 + 6 - 4+  3", "12+6-4+3");
        test_parser!(expression, " 1 + 2*3 + 4", "1+2*3+4");
        test_parser!(expression, " 1 + 2*${foo::bar - 13} + 4 ", "1+2*__eval('foo::bar - 13')+4");
        test_parser!(expression, " (  2 )", "(2)");
        test_parser!(expression, " 2* (  3 + 4 ) ", "2*(3+4)");
        test_parser!(expression, " a << 12345", "__eval('a')<<12345");
        test_parser!(expression, " a >> 12345", "__eval('a')>>12345");
        test_parser!(expression, " a >= 12345", "__eval('a')>=12345");
        test_parser!(expression, " a <= 12345", "__eval('a')<=12345");
        test_parser!(expression, " a // 12345", "__eval('a')//12345");
        test_parser!(expression, "  2*2 / ( 5 - 1) + 3",
                                 "2*2/(5-1)+3");
        test_parser!(expression, " 1 + (2 * ${foo::bar - 13}** 4) + 4 ",
                                 "1+(2*__eval('foo::bar - 13')**4)+4");
        test_parser!(expression, " 1 + (2 * $foo::bar.baz[ $quoox ** 4 ] ) + 5 ",
                                 "1+(2*__eval('foo::bar').__getattr__('baz')[__eval('quoox')**4])+5");
        test_parser!(expression, "  aa and not b or not True",
                                 "__eval('aa') and not __eval('b') or not True");
        test_parser!(expression, "  $and and not $not or not True",
                                "__eval('and') and not __eval('not') or not True");
    }
}
