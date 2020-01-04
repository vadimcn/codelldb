use crate::debug_protocol::Expressions;
use lldb::SBValue;
use regex::{Captures, Regex, RegexBuilder};
use std::borrow::Cow;

#[derive(Debug)]
pub enum PreparedExpression {
    Native(String),
    Simple(String),
    Python(String),
}

#[derive(Debug, Clone)]
pub enum HitCondition {
    LT(u32),
    LE(u32),
    EQ(u32),
    GE(u32),
    GT(u32),
    MOD(u32),
}

#[derive(Debug, Clone)]
pub enum FormatSpec {
    Format(lldb::Format),
    Array(u32),
}

// Parse expression type and preprocess it.
pub fn prepare(expression: &str, default_type: Expressions) -> PreparedExpression {
    let (expr, ty) = get_expression_type(expression, default_type);
    match ty {
        Expressions::Native => PreparedExpression::Native(expr.to_owned()),
        Expressions::Simple => PreparedExpression::Simple(preprocess_simple_expr(expr)),
        Expressions::Python => PreparedExpression::Python(preprocess_python_expr(expr)),
    }
}

// Same as prepare(), but also parses formatting options at the end of expression,
// for example, `value,x` to format value as hex or `ptr,[50]` to interpret `ptr` as an array of 50 elements.
pub fn prepare_with_format(
    expression: &str,
    default_type: Expressions,
) -> Result<(PreparedExpression, Option<FormatSpec>), String> {
    let (expr, ty) = get_expression_type(expression, default_type);
    let (expr, format) = get_expression_format(expr)?;
    let pp_expr = match ty {
        Expressions::Native => PreparedExpression::Native(expr.to_owned()),
        Expressions::Simple => PreparedExpression::Simple(preprocess_simple_expr(expr)),
        Expressions::Python => PreparedExpression::Python(preprocess_python_expr(expr)),
    };
    Ok((pp_expr, format))
}

pub fn parse_hit_condition(expr: &str) -> Result<HitCondition, ()> {
    if let Some(captures) = HIT_COUNT.captures(expr) {
        let number = match captures.get(2).unwrap().as_str().parse::<u32>() {
            Ok(n) => n,
            Err(_) => return Err(()),
        };

        let hit_cond = if let Some(op) = captures.get(1) {
            match op.as_str() {
                "<" => HitCondition::LT(number),
                "<=" => HitCondition::LE(number),
                "=" | "==" => HitCondition::EQ(number),
                ">=" => HitCondition::GE(number),
                ">" => HitCondition::GT(number),
                "%" => HitCondition::MOD(number),
                _ => unreachable!(),
            }
        } else {
            HitCondition::GE(number) // `10` is the same as `>= 10`
        };
        Ok(hit_cond)
    } else {
        Err(())
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

fn get_expression_format<'a>(expr: &'a str) -> Result<(&'a str, Option<FormatSpec>), String> {
    if let Some(captures) = EXPRESSION_FORMAT.captures(expr) {
        let expr = &expr[..captures.get(0).unwrap().start()];

        if let Some(m) = captures.get(1) {
            let format = match m.as_str() {
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
                _ => bail!(format!("Invalid format specifier: {}", m.as_str())),
            };
            Ok((expr, Some(FormatSpec::Format(format))))
        } else if let Some(m) = captures.get(2) {
            let size = m.as_str().parse::<u32>().map_err(|err| err.to_string())?;
            Ok((expr, Some(FormatSpec::Array(size))))
        } else {
            unreachable!()
        }
    } else {
        Ok((expr, None))
    }
}

fn compile_regex(pattern: &str) -> Regex {
    RegexBuilder::new(pattern).ignore_whitespace(true).multi_line(true).build().unwrap()
}

fn create_regexes() -> [Regex; 3] {
    // Matches Python strings
    let pystring =
        [r#"(?:"(?:\\"|\\\\|[^"])*")"#, r#"(?:'(?:\\'|\\\\|[^'])*')"#, r#"(?:r"[^"]*")"#, r#"(?:r'[^']*')"#].join("|");

    let kwlist = [
        "as", "assert", "break", "class", "continue", "def", "del", "elif", "else", "except", "exec", "finally", "for",
        "from", "global", "if", "import", "in", "is", "lambda", "pass", "print", "raise", "return", "try", "while",
        "with", "yield", // except "and", "or", "not"
    ];

    // # Matches Python keywords
    let keywords = kwlist.join("|");

    // # Matches identifiers
    let ident = r#"[A-Za-z_] [A-Za-z0-9_]*"#;

    // # Matches `::xxx`, `xxx::yyy`, `::xxx::yyy`, `xxx::yyy::zzz`, etc
    let qualified_ident = format!(r#"(?: (?: ::)? (?: {ident} ::)+ | :: ) {ident}"#, ident = ident);
    #[cfg(test)]
    {
        let regex = compile_regex(&qualified_ident);
        assert!(regex.is_match("::xxx"));
        assert!(regex.is_match("xxx::yyy"));
        assert!(regex.is_match("::xxx::yyy"));
        assert!(regex.is_match("xxx::yyy::zzz"));
    }

    // # Matches `xxx`, `::xxx`, `xxx::yyy`, `::xxx::yyy`, `xxx::yyy::zzz`, etc
    let maybe_qualified_ident = format!(r#"(?: ::)? (?: {ident} ::)* {ident}"#, ident = ident);
    #[cfg(test)]
    {
        let regex = compile_regex(&maybe_qualified_ident);
        assert!(regex.is_match("xxx"));
        assert!(regex.is_match("::xxx"));
        assert!(regex.is_match("xxx::yyy"));
        assert!(regex.is_match("::xxx::yyy"));
    }

    // # Matches `$xxx`, `$xxx::yyy::zzz` or `${...}`, captures the escaped text.
    let escaped_ident =
        format!(r#"\$ ({maybe_qualified_ident}) | \$ \{{ ([^}}]*) \}}"#, maybe_qualified_ident = maybe_qualified_ident);
    #[cfg(test)]
    {
        let regex = compile_regex(&escaped_ident);
        assert!(regex.is_match("$xxx"));
        assert!(regex.is_match("$xxx::yyy::zzz"));
        assert!(regex.is_match("${boo::bar.baz}"));
        assert!(regex.is_match("${23ro0c1934!#$%0wf87145798145}"));
    }

    let maybe_qualified_ident_only =
        format!(r#"^ {maybe_qualified_ident} $"#, maybe_qualified_ident = maybe_qualified_ident);

    let preprocess_simple = format!(
        r#"(\.)? (?: {pystring} | \b ({keywords}) \b | ({qualified_ident}) | {escaped_ident} )"#,
        pystring = pystring,
        keywords = keywords,
        qualified_ident = qualified_ident,
        escaped_ident = escaped_ident
    );

    let preprocess_python =
        format!(r#"(\.)? (?: {pystring} | {escaped_ident} )"#, pystring = pystring, escaped_ident = escaped_ident);

    [compile_regex(&maybe_qualified_ident_only), compile_regex(&preprocess_simple), compile_regex(&preprocess_python)]
}

lazy_static::lazy_static! {
    static ref EXPRESSIONS: [Regex; 3] = create_regexes();
    static ref MAYBE_QUALIFIED_IDENT: &'static Regex = &EXPRESSIONS[0];
    static ref PREPROCESS_SIMPLE: &'static Regex = &EXPRESSIONS[1];
    static ref PREPROCESS_PYTHON: &'static Regex = &EXPRESSIONS[2];
    static ref HIT_COUNT: Regex = compile_regex(r"\A\s*(>|>=|=|==|<|<=|%)?\s*([0-9]+)\s*\z");
    static ref EXPRESSION_FORMAT: Regex = compile_regex(r", (?: ([A-Za-z]) | (?: \[ (\d+) \] ) )\z");
}

pub fn escape_variable_name<'a>(name: &'a str) -> Cow<'a, str> {
    if MAYBE_QUALIFIED_IDENT.is_match(name) {
        name.into()
    } else {
        format!("${{{}}}", name).into()
    }
}

fn replacer(captures: &Captures) -> String {
    let mut iter = captures.iter();
    iter.next(); // Skip the full match
    let have_prefix = iter.next().unwrap().is_some();
    for ident in iter {
        if let Some(ident) = ident {
            if have_prefix {
                return format!(r#".__getattr__("{}")"#, ident.as_str());
            } else {
                return format!(r#"__frame_vars["{}"]"#, ident.as_str());
            }
        }
    }
    return captures[0].into();
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
    // TODO: Cow?
    PREPROCESS_SIMPLE.replace_all(expr, replacer).into_owned()
}

// Replaces variable placeholders in native Python expressions with access via __frame_vars,
// or `__getattr__()` (if prefixed by a dot).
// For example, `$var + 42` will be translated to `__frame_vars["var"] + 42`.
pub fn preprocess_python_expr(expr: &str) -> String {
    PREPROCESS_PYTHON.replace_all(expr, replacer).into_owned()
}

///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

macro_rules! assert_match(($e:expr, $p:pat) => { assert!(match $e { $p => true, _ => false }, stringify!($e ~ $p)) });

#[test]
fn test_simple() {
    let expr = r#"
        class = from.global.finally
        local::bar::__BAZ()
        local_string()
        ::foo
        ::foo::bar::baz
        foo::bar::baz
        $local::foo::bar
        ${std::integral_constant<long, 1l>::value}
        ${std::integral_constant<long, 1l, foo<123>>::value}
        ${std::allocator_traits<std::allocator<std::thread::_Impl<std::_Bind_simple<threads(int)::__lambda0(int)> > > >::__construct_helper<std::thread::_Impl<std::_Bind_simple<threads(int)::__lambda0(int)> >, std::_Bind_simple<threads(int)::__lambda0(int)> >::value}
        vec_int.${std::_Vector_base<std::vector<int, std::allocator<int> >, std::allocator<std::vector<int, std::allocator<int> > > >}._M_impl._M_start

        """continue.exec = pass.print; yield.with = 3"""
        \'''continue.exec = pass.print; yield.with = 3\'''
        "continue.exec = pass.print; yield.with = 3"
    "#;
    let expected = r#"
        __frame_vars["class"] = __frame_vars["from"].__getattr__("global").__getattr__("finally")
        __frame_vars["local::bar::__BAZ"]()
        local_string()
        __frame_vars["::foo"]
        __frame_vars["::foo::bar::baz"]
        __frame_vars["foo::bar::baz"]
        __frame_vars["local::foo::bar"]
        __frame_vars["std::integral_constant<long, 1l>::value"]
        __frame_vars["std::integral_constant<long, 1l, foo<123>>::value"]
        __frame_vars["std::allocator_traits<std::allocator<std::thread::_Impl<std::_Bind_simple<threads(int)::__lambda0(int)> > > >::__construct_helper<std::thread::_Impl<std::_Bind_simple<threads(int)::__lambda0(int)> >, std::_Bind_simple<threads(int)::__lambda0(int)> >::value"]
        vec_int.__getattr__("std::_Vector_base<std::vector<int, std::allocator<int> >, std::allocator<std::vector<int, std::allocator<int> > > >")._M_impl._M_start

        """continue.exec = pass.print; yield.with = 3"""
        \'''continue.exec = pass.print; yield.with = 3\'''
        "continue.exec = pass.print; yield.with = 3"
    "#;
    let prepr = preprocess_simple_expr(expr);
    assert_eq!(expected, prepr);
}

#[test]
fn test_python() {
    let expr = r#"
        for x in $foo: print x
        $xxx.$yyy.$zzz
        $xxx::yyy::zzz
        $::xxx
        "$xxx::yyy::zzz"
    "#;
    let expected = r#"
        for x in __frame_vars["foo"]: print x
        __frame_vars["xxx"].__getattr__("yyy").__getattr__("zzz")
        __frame_vars["xxx::yyy::zzz"]
        __frame_vars["::xxx"]
        "$xxx::yyy::zzz"
    "#;
    let prepr = preprocess_python_expr(expr);
    assert_eq!(expected, prepr);
}

#[test]
fn test_escape_variable_name() {
    assert_eq!(escape_variable_name("foo"), "foo");
    assert_eq!(escape_variable_name("foo::bar"), "foo::bar");
    assert_eq!(escape_variable_name("foo::bar<34>"), "${foo::bar<34>}");
    assert_eq!(escape_variable_name("foo::bar<34>::value"), "${foo::bar<34>::value}");
}

#[test]
fn test_expression_format() {
    assert_match!(get_expression_format("foo"), Ok(("foo", None)));
    assert_match!(get_expression_format("foo,bar"), Ok(("foo,bar", None)));

    assert_match!(get_expression_format("foo,h"), Ok(("foo", Some(FormatSpec::Format(lldb::Format::Hex)))));
    assert_match!(get_expression_format("foo,x"), Ok(("foo", Some(FormatSpec::Format(lldb::Format::Hex)))));
    assert_match!(get_expression_format("foo,y"), Ok(("foo", Some(FormatSpec::Format(lldb::Format::Bytes)))));
    assert_match!(get_expression_format("foo,Y"), Ok(("foo", Some(FormatSpec::Format(lldb::Format::BytesWithASCII)))));

    assert_match!(get_expression_format("foo,[42]"), Ok(("foo", Some(FormatSpec::Array(42)))));

    assert_match!(get_expression_format("foo,Z"), Err(_));
}

#[test]
fn test_parse_hit_condition() {
    assert_match!(parse_hit_condition(" 13   "), Ok(HitCondition::GE(13)));
    assert_match!(parse_hit_condition(" < 42"), Ok(HitCondition::LT(42)));
    assert_match!(parse_hit_condition(" <=53 "), Ok(HitCondition::LE(53)));
    assert_match!(parse_hit_condition("=  61"), Ok(HitCondition::EQ(61)));
    assert_match!(parse_hit_condition("==62 "), Ok(HitCondition::EQ(62)));
    assert_match!(parse_hit_condition(">=76 "), Ok(HitCondition::GE(76)));
    assert_match!(parse_hit_condition(">85"), Ok(HitCondition::GT(85)));
    assert_match!(parse_hit_condition(""), Err(_));
    assert_match!(parse_hit_condition("      "), Err(_));
    assert_match!(parse_hit_condition("!90"), Err(_));
    assert_match!(parse_hit_condition("=>92"), Err(_));
    assert_match!(parse_hit_condition("<"), Err(_));
    assert_match!(parse_hit_condition("=AA"), Err(_));
    assert_match!(parse_hit_condition("XYZ"), Err(_));
}
