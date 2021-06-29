use nom::{
    branch::alt,
    bytes::complete::{is_not, tag},
    character::complete::{anychar, char, none_of},
    combinator::{opt, recognize},
    multi::many0_count,
    sequence::{delimited, pair, preceded},
    IResult,
};
use std::fmt::Write;

use super::prelude::*;
use super::qualified_ident::qualified_ident;

fn escaped_ident(input: Span) -> IResult<Span, Span> {
    preceded(
        tag("$"),
        alt((
            recognize(qualified_ident), //
            delimited(tag("{"), recognize(is_not("}")), tag("}")),
        )),
    )(input)
}

// Recognize Python strings
fn python_string(input: Span) -> IResult<Span, Span> {
    fn body(delim: &'static str) -> impl Fn(Span) -> IResult<Span, Span> {
        move |input| {
            recognize(many0_count(alt((
                recognize(pair(char('\\'), anychar)), //.
                recognize(none_of(delim)),
            ))))(input)
        }
    }
    recognize(alt((
        delimited(char('\"'), body("\""), char('\"')), //.
        delimited(char('\''), body("\'"), char('\'')),
        delimited(tag("r\""), is_not("\""), char('"')),
        delimited(tag("r\'"), is_not("\'"), char('\'')),
    )))(input)
}

// Replaces identifiers that are invalid according to Python syntax in simple expressions:
// - identifiers that happen to be Python keywords (e.g.`for`),
// - qualified identifiers (e.g. `foo::bar::baz`),
// - raw identifiers of the form $xxxxxx,
// with access via `__frame_vars`, or `__getattr__()` (if prefixed by a dot).
// For example, `for + foo::bar::baz + foo::bar::baz.class() + $SomeClass<int>::value` will be translated to
// `__frame_vars["for"] + __frame_vars["foo::bar::baz"] +
//  __frame_vars["foo::bar::baz"].__getattr__("class") + __frame_vars["SomeClass<int>::value"]`
pub fn preprocess_simple_expr(expr: &str) -> String {
    let mut pref_qident = pair(opt(char('.')), recognize(qualified_ident));
    let mut pref_eident = pair(opt(char('.')), escaped_ident);
    let handle_prefixed = |result: &mut String, (prefix, ident): (Option<char>, &str)| {
        if prefix.is_none() {
            write!(result, "__frame_vars[\"{}\"]", ident).unwrap();
        } else {
            write!(result, ".__getattr__(\"{}\")", ident).unwrap();
        }
    };
    fn logical_keyword(input: Span) -> IResult<Span, Span> {
        alt((tag("and"), tag("or"), tag("not")))(input)
    }

    let mut expr = expr;
    let mut result = String::new();
    loop {
        if let Ok((rest, s)) = python_string(expr) {
            result.push_str(s);
            expr = rest;
        } else if let Ok((rest, kw)) = logical_keyword(expr) {
            result.push_str(kw);
            expr = rest;
        } else if let Ok((rest, p)) = pref_qident(expr) {
            handle_prefixed(&mut result, p);
            expr = rest;
        } else if let Ok((rest, p)) = pref_eident(expr) {
            handle_prefixed(&mut result, p);
            expr = rest;
        } else {
            let mut chars = expr.chars();
            if let Some(ch) = chars.next() {
                result.push(ch);
                expr = chars.as_str();
            } else {
                break;
            }
        }
    }
    return result;
}

// Replaces variable placeholders in native Python expressions with access via __frame_vars,
// or `__getattr__()` (if prefixed by a dot).
// For example, `$var + 42` will be translated to `__frame_vars["var"] + 42`.
pub fn preprocess_python_expr(expr: &str) -> String {
    let mut expr = expr;
    let mut result = String::new();
    let mut pref_eident = pair(opt(char('.')), escaped_ident);

    loop {
        if let Ok((rest, s)) = python_string(expr) {
            result.push_str(s);
            expr = rest;
        } else if let Ok((rest, (prefix, ident))) = pref_eident(expr) {
            if prefix.is_none() {
                write!(result, "__frame_vars[\"{}\"]", ident).unwrap();
            } else {
                write!(result, ".__getattr__(\"{}\")", ident).unwrap();
            }
            expr = rest;
        } else {
            let mut chars = expr.chars();
            if let Some(ch) = chars.next() {
                result.push(ch);
                expr = chars.as_str();
            } else {
                break;
            }
        }
    }
    return result;
}

///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
use nom::Parser;

#[test]
fn test_escaped_ident() {
    assert_eq!(escaped_ident.parse("$xxx"), Ok(("", "xxx")));
    assert_eq!(escaped_ident.parse("$xxx::yyy::zzz"), Ok(("", "xxx::yyy::zzz")));
    assert_eq!(escaped_ident.parse("${boo::bar.baz}"), Ok(("", "boo::bar.baz")));
    assert_eq!(escaped_ident.parse("${23ro0c1934!#$%0wf87145798145}"), Ok(("", "23ro0c1934!#$%0wf87145798145")));
}

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
fn test_pair(input: &str, expected: &str, preprocessor: impl Fn(&str) -> String) {
    {
        let prepr = preprocessor(input);
        assert_eq!(prepr, expected);
    }
    {
        let input = format!("   {}", input);
        let expected = format!("   {}", expected);
        let prepr = preprocessor(&input);
        assert_eq!(prepr, expected);
    }
    {
        let input = format!("{}   ", input);
        let expected = format!("{}   ", expected);
        let prepr = preprocessor(&input);
        assert_eq!(prepr, expected);
    }
}

#[test]
fn test_simple_expr() {
    #[rustfmt::skip]
    let pairs = [
        (
            r#"class = from.global.finally"#,
            r#"__frame_vars["class"] = __frame_vars["from"].__getattr__("global").__getattr__("finally")"#,
        ),
        (
            r#"local::bar::__BAZ()"#,
            r#"__frame_vars["local::bar::__BAZ"]()"#
        ),
        (
            r#"local_string()"#,
            r#"__frame_vars["local_string"]()"#
        ),
        (
            r#"::foo"#,
            r#"__frame_vars["::foo"]"#
        ),
        (
            r#"::foo::bar::baz"#,
            r#"__frame_vars["::foo::bar::baz"]"#
        ),
        (
            r#"foo::bar::baz"#,
            r#"__frame_vars["foo::bar::baz"]"#
        ),
        (
            r#"$local::foo::bar"#,
            r#"__frame_vars["local::foo::bar"]"#
        ),
        (
            r#"${std::integral_constant<long, 1l>::value}"#,
            r#"__frame_vars["std::integral_constant<long, 1l>::value"]"#
        ),
        (
            r#"${std::integral_constant<long, 1l, foo<123>>::value}"#,
            r#"__frame_vars["std::integral_constant<long, 1l, foo<123>>::value"]"#,
        ),
        (
            r#"${std::allocator_traits<std::allocator<std::thread::_Impl<std::_Bind_simple<threads(int)::__lambda0(int)> > > >::__construct_helper<std::thread::_Impl<std::_Bind_simple<threads(int)::__lambda0(int)> >, std::_Bind_simple<threads(int)::__lambda0(int)> >::value}"#,
            r#"__frame_vars["std::allocator_traits<std::allocator<std::thread::_Impl<std::_Bind_simple<threads(int)::__lambda0(int)> > > >::__construct_helper<std::thread::_Impl<std::_Bind_simple<threads(int)::__lambda0(int)> >, std::_Bind_simple<threads(int)::__lambda0(int)> >::value"]"#,
        ),
        (
            r#"vec_int.${std::_Vector_base<std::vector<int, std::allocator<int> >, std::allocator<std::vector<int, std::allocator<int> > > >}._M_impl._M_start"#,
            r#"__frame_vars["vec_int"].__getattr__("std::_Vector_base<std::vector<int, std::allocator<int> >, std::allocator<std::vector<int, std::allocator<int> > > >").__getattr__("_M_impl").__getattr__("_M_start")"#,
        ),
        (
            r#""""continue.exec = pass.print; yield.with = 3""""#,
            r#""""continue.exec = pass.print; yield.with = 3""""#),
        (
            r#"\'''continue.exec = pass.print; yield.with = 3\'''"#,
            r#"\'''continue.exec = pass.print; yield.with = 3\'''"#,
        ),
        (
            r#""continue.exec = pass.print; yield.with = 3""#,
            r#""continue.exec = pass.print; yield.with = 3""#
        ),
        (
            r#"aaa and (bbb or $ccc::ddd) and not fff.ggg"#,
            r#"__frame_vars["aaa"] and (__frame_vars["bbb"] or __frame_vars["ccc::ddd"]) and not __frame_vars["fff"].__getattr__("ggg")"#
        )
    ];
    for (expr, expected) in pairs.iter() {
        test_pair(expr, expected, &preprocess_simple_expr);
    }
}

#[test]
fn test_python_expr() {
    #[rustfmt::skip]
    let pairs = [
        (
            r#"for x in $foo: print x"#,
            r#"for x in __frame_vars["foo"]: print x"#
        ),
        (
            r#"$xxx.$yyy.$zzz"#,
            r#"__frame_vars["xxx"].__getattr__("yyy").__getattr__("zzz")"#
        ),
        (
            r#"$xxx::yyy::zzz"#,
            r#"__frame_vars["xxx::yyy::zzz"]"#
        ),
        (
            r#"$::xxx"#,
            r#"__frame_vars["::xxx"]"#
        ),
        (
            r#"${foo::bar::baz}"#,
            r#"__frame_vars["foo::bar::baz"]"#
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
