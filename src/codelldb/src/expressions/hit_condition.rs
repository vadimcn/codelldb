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

impl HitCondition {
    pub fn parse(expr: &str) -> Result<Self, ()> {
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
            Ok((_, HitCondition::MOD(0))) => Err(()),
            Ok((_, hc)) => Ok(hc),
            Err(_) => Err(()),
        }
    }

    pub fn matches(&self, hit_count: u32) -> bool {
        match self {
            Self::LT(n) => hit_count < *n,
            Self::LE(n) => hit_count <= *n,
            Self::EQ(n) => hit_count == *n,
            Self::GE(n) => hit_count >= *n,
            Self::GT(n) => hit_count > *n,
            Self::MOD(n) => hit_count % *n == 0,
        }
    }
}

///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

#[test]
fn test_hit_condition_parse() {
    assert_matches!(HitCondition::parse(" 13   "), Ok(HitCondition::GE(13)));
    assert_matches!(HitCondition::parse(" < 42"), Ok(HitCondition::LT(42)));
    assert_matches!(HitCondition::parse(" <=53 "), Ok(HitCondition::LE(53)));
    assert_matches!(HitCondition::parse("=  61"), Ok(HitCondition::EQ(61)));
    assert_matches!(HitCondition::parse("==62 "), Ok(HitCondition::EQ(62)));
    assert_matches!(HitCondition::parse(">=76 "), Ok(HitCondition::GE(76)));
    assert_matches!(HitCondition::parse(">85"), Ok(HitCondition::GT(85)));
    assert_matches!(HitCondition::parse("%0"), Err(_));
    assert_matches!(HitCondition::parse("%2"), Ok(HitCondition::MOD(2)));
    assert_matches!(HitCondition::parse(""), Err(_));
    assert_matches!(HitCondition::parse("      "), Err(_));
    assert_matches!(HitCondition::parse("!90"), Err(_));
    assert_matches!(HitCondition::parse("=>92"), Err(_));
    assert_matches!(HitCondition::parse("<"), Err(_));
    assert_matches!(HitCondition::parse("=AA"), Err(_));
    assert_matches!(HitCondition::parse("XYZ"), Err(_));
}
