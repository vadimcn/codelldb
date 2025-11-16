use nom::{
    branch::alt, bytes::complete::tag, character::complete::space0, combinator::map, sequence::preceded, Parser,
};

use super::prelude::*;

#[derive(Debug, Clone)]
pub enum HitCondition {
    LT(u32),
    LE(u32),
    EQ(u32),
    GE(u32),
    GT(u32),
    MOD(u32),
}

pub fn parse_hit_condition(expr: &str) -> Result<HitCondition, ()> {
    fn parser(input: Span) -> IResult<Span, HitCondition> {
        alt((
            map(preceded(tag("<="), preceded(space0, unsigned)), |n| HitCondition::LE(n)),
            map(preceded(tag("<"), preceded(space0, unsigned)), |n| HitCondition::LT(n)),
            map(preceded(tag("=="), preceded(space0, unsigned)), |n| HitCondition::EQ(n)),
            map(preceded(tag("="), preceded(space0, unsigned)), |n| HitCondition::EQ(n)),
            map(preceded(tag(">="), preceded(space0, unsigned)), |n| HitCondition::GE(n)),
            map(preceded(tag(">"), preceded(space0, unsigned)), |n| HitCondition::GT(n)),
            map(preceded(tag("%"), preceded(space0, unsigned)), |n| HitCondition::MOD(n)),
            map(unsigned, |n| HitCondition::GE(n)),
        ))
        .parse(input)
    }

    match parser.parse(expr.trim()) {
        Ok((_, hc)) => Ok(hc),
        Err(_) => Err(()),
    }
}

///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

#[test]
fn test_parse_hit_condition() {
    assert_matches!(parse_hit_condition(" 13   "), Ok(HitCondition::GE(13)));
    assert_matches!(parse_hit_condition(" < 42"), Ok(HitCondition::LT(42)));
    assert_matches!(parse_hit_condition(" <=53 "), Ok(HitCondition::LE(53)));
    assert_matches!(parse_hit_condition("=  61"), Ok(HitCondition::EQ(61)));
    assert_matches!(parse_hit_condition("==62 "), Ok(HitCondition::EQ(62)));
    assert_matches!(parse_hit_condition(">=76 "), Ok(HitCondition::GE(76)));
    assert_matches!(parse_hit_condition(">85"), Ok(HitCondition::GT(85)));
    assert_matches!(parse_hit_condition(""), Err(_));
    assert_matches!(parse_hit_condition("      "), Err(_));
    assert_matches!(parse_hit_condition("!90"), Err(_));
    assert_matches!(parse_hit_condition("=>92"), Err(_));
    assert_matches!(parse_hit_condition("<"), Err(_));
    assert_matches!(parse_hit_condition("=AA"), Err(_));
    assert_matches!(parse_hit_condition("XYZ"), Err(_));
}
