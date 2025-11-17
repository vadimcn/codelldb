use std::borrow::Cow;

use nom::{
    branch::alt,
    bytes::complete::tag,
    character::complete::char,
    combinator::recognize,
    sequence::{delimited, preceded},
    Parser,
};
use nom_language::precedence::{binary_op, precedence, unary_op, Assoc, Binary, Unary};

use super::literals::{boolean_literal, integer_literal, numeric_literal};
use super::preprocess::{native_expr, python_string};
use super::qualified_ident::{ident, qualified_ident};

use super::prelude::*;

#[allow(mismatched_lifetime_syntaxes)]
pub fn expression(input: Span) -> IResult<Span, String> {
    fn operand(input: Span) -> IResult<Span, Cow<str>> {
        ws(alt((
            numeric_literal.map(Cow::from),
            boolean_literal,
            python_string.map(Cow::from),
            recognize(qualified_ident).map(|e| format!("__eval('{e}')").into()),
            native_expr.map(|e| format!("__eval('{e}')").into()),
            delimited(tag("("), expression, tag(")")).map(|e| format!("({e})").into()),
        )))
        .parse(input)
    }

    fn prefix(input: Span) -> IResult<Span, Unary<&str, i32>> {
        unary_op(
            3,
            ws(alt((
                tag("!"),
                tag("not"),
                tag("-"),
                tag("+"),
                tag("~"),
                tag("*"),
                tag("&"),
            ))),
        )
        .parse(input)
    }

    // Operators and their precedence mostly follows C++, with two Python additions: `**` and `//`.
    // Nom tries alternates in the order they are declared, so for ambiguous tokens (e.g., `&&` vs `&`,
    // or `<<` vs `<`), the longer operators are listed before the shorter ones so they are matched first.
    fn binary(input: Span) -> IResult<Span, Binary<&str, i32>> {
        ws(alt((
            binary_op(15, Assoc::Left, alt((tag("or"), tag("||"))).map(|_| "or")),
            binary_op(14, Assoc::Left, alt((tag("and"), tag("&&"))).map(|_| "and")),
            binary_op(7, Assoc::Left, alt((tag("<<"), tag(">>")))),
            binary_op(4, Assoc::Right, tag("**")),
            binary_op(9, Assoc::Left, alt((tag(">="), tag(">"), tag("<="), tag("<")))),
            binary_op(6, Assoc::Left, alt((tag("+"), tag("-")))),
            binary_op(5, Assoc::Left, alt((tag("*"), tag("//"), tag("/"), tag("%")))),
            binary_op(13, Assoc::Left, tag("|")),
            binary_op(12, Assoc::Left, tag("^")),
            binary_op(11, Assoc::Left, tag("&")),
            binary_op(10, Assoc::Left, alt((tag("=="), tag("!=")))),
        )))
        .parse(input)
    }

    fn postfix(input: Span) -> IResult<Span, Unary<(&str, Cow<str>), i32>> {
        unary_op(
            2,
            alt((
                preceded(ws(char('.')), ident).map(|arg| (".", Cow::from(arg))),
                preceded(ws(char('.')), recognize(unsigned)).map(|arg| (".", Cow::from(arg))),
                preceded(ws(tag("->")), ident).map(|arg| ("->", Cow::from(arg))),
                delimited(
                    ws(char('[')),
                    alt((integer_literal.map(Cow::from), expression.map(Cow::from))),
                    ws(char(']')),
                )
                .map(|arg| ("[]", arg)),
            )),
        )
        .parse(input)
    }

    precedence(prefix, postfix, binary, operand, |op| {
        use nom_language::precedence::Operation::*;
        Ok(match op {
            Prefix(op, o) => match op {
                "*" => format!("Value.dereference({o})").into(),
                "&" => format!("Value.address_of({o})").into(),
                "!" | "not" => format!("(not {o})").into(),
                _ => format!("{op}{o}").into(),
            },
            Binary(lhs, op, rhs) => format!("({lhs} {op} {rhs})").into(),
            Postfix(o, (op, arg)) => match op {
                "." => format!("{o}.__getattr__('{arg}')").into(),
                "->" => format!("Value.dereference({o}).__getattr__('{arg}')").into(),
                "[]" => format!("{o}[{arg}]").into(),
                _ => return Err(""),
            },
        })
    })
    .map(|r| r.into_owned())
    .parse(input)
}

#[cfg(test)]
#[rustfmt::skip::macros(test_parser)]
mod test {
    use super::*;

    #[test]
    fn primary_test() {
        test_parser!(expression, " foo", "__eval('foo')");
        test_parser!(expression, " foo::bar  ", "__eval('foo::bar')");
        test_parser!(expression, " $foo::bar  ", "__eval('foo::bar')");
        test_parser!(expression, "  ${foo::bar + 3}  ", "__eval('foo::bar + 3')");
        test_parser!(expression, " 'st\ring'  ", "'st\ring'");
        test_parser!(expression, " \"string\"  ", "\"string\"");
        test_parser!(expression, "  std::numeric_limits<float>::digits ", "__eval('std::numeric_limits<float>::digits')");
    }

    #[test]
    fn postfix_test() {
        test_parser!(expression, " foo . bar", "__eval('foo').__getattr__('bar')");
        test_parser!(expression, " foo . 0", "__eval('foo').__getattr__('0')");
        test_parser!(expression, " foo -> bar", "Value.dereference(__eval('foo')).__getattr__('bar')");
        test_parser!(expression, " foo . 0 . bar . 42",
                                 "__eval('foo').__getattr__('0').__getattr__('bar').__getattr__('42')");
        test_parser!(expression, " foo::foo . bar . baz", "__eval('foo::foo').__getattr__('bar').__getattr__('baz')");
        test_parser!(expression, " foo::foo2 . bar [ 32 ] . baz",
                                 "__eval('foo::foo2').__getattr__('bar')[32].__getattr__('baz')");
        test_parser!(expression, " foo [ 0o77 ]", "__eval('foo')[0o77]");
        test_parser!(expression, " foo [ bar + 1 ]", "__eval('foo')[(__eval('bar') + 1)]");
    }

    #[test]
    fn locical_test() {
        test_parser!(expression, "!true && not false ||True", "(((not True) and (not False)) or True)");
    }

    #[test]
    fn expression_test() {
        test_parser!(expression, " 12 *2 /  3", "((12 * 2) / 3)");
        test_parser!(expression, " 2* 3  *2 *2 /  3", "((((2 * 3) * 2) * 2) / 3)");
        test_parser!(expression, " 48 +  3/2", "(48 + (3 / 2))");
        test_parser!(expression, " 3 *foo/ 4 ", "((3 * __eval('foo')) / 4)");
        test_parser!(expression, " 3 *'foo'/ 4 ", "((3 * 'foo') / 4)");
        test_parser!(expression, " 3 *foo::bar/ 4 ", "((3 * __eval('foo::bar')) / 4)");
        test_parser!(expression, " 3 *${foo::bar - 13}/ 4 ", "((3 * __eval('foo::bar - 13')) / 4)");
        test_parser!(expression, " 1 +  2 ", "(1 + 2)");
        test_parser!(expression, " 12 + 6 - 4+  3", "(((12 + 6) - 4) + 3)");
        test_parser!(expression, " 1 + 2*3 + 4", "((1 + (2 * 3)) + 4)");
        test_parser!(expression, " 1 + 2*${foo::bar - 13} + 4 ", "((1 + (2 * __eval('foo::bar - 13'))) + 4)" );
        test_parser!(expression, " (  2 )", "(2)");
        test_parser!(expression, " 2* (  3 + 4 ) ", "(2 * ((3 + 4)))");
        test_parser!(expression, " a << 12345", "(__eval('a') << 12345)");
        test_parser!(expression, " a >> 12345", "(__eval('a') >> 12345)");
        test_parser!(expression, " a >= 12345", "(__eval('a') >= 12345)");
        test_parser!(expression, " a <= 12345", "(__eval('a') <= 12345)");
        test_parser!(expression, " a // 12345", "(__eval('a') // 12345)");
        test_parser!(expression, " 0xff + 0b10", "(0xff + 0b10)");
        test_parser!(expression, "  2*2 / ( 5 - 1) + 3", "(((2 * 2) / ((5 - 1))) + 3)");
        test_parser!(expression, "2 ** 2 ** 3", "(2 ** (2 ** 3))");
        test_parser!(expression, " 1 + (2 * ${foo::bar - 13}** 4) + 4 ",
                                 "((1 + ((2 * (__eval('foo::bar - 13') ** 4)))) + 4)");
        test_parser!(expression, " 1 + (2 * $foo::bar.baz[ $quoox ** 4 ] ) + 5 ",
                                 "((1 + ((2 * __eval('foo::bar').__getattr__('baz')[(__eval('quoox') ** 4)]))) + 5)");
        test_parser!(expression, "  aa and not b or not True",
                                 "((__eval('aa') and (not __eval('b'))) or (not True))");
        test_parser!(expression, "  aa && !b || not True",
                                 "((__eval('aa') and (not __eval('b'))) or (not True))");
        test_parser!(expression, "  $and and not $not or not True",
                                 "((__eval('and') and (not __eval('not'))) or (not True))");
        test_parser!(expression, "  true && false", "(True and False)");
        test_parser!(expression, " * foo.bar", "Value.dereference(__eval('foo').__getattr__('bar'))");
        test_parser!(expression, " & foo.bar", "Value.address_of(__eval('foo').__getattr__('bar'))");
        test_parser!(expression, " & foo->bar", "Value.address_of(Value.dereference(__eval('foo')).__getattr__('bar'))");
    }
}
