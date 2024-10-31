use anyhow::Context;

type ParseResult<'input, O> = anyhow::Result<(O, &'input str)>;

pub trait Parser<'input, O>: Sized {
    fn parse(&self, input: &'input str) -> ParseResult<'input, O>;

    fn then<P, O2>(self, then: P) -> impl Parser<'input, (O, O2)>
    where
        P: Parser<'input, O2>,
    {
        Then { first: self, then }
    }

    fn map<F, O2>(self, f: F) -> impl Parser<'input, O2>
    where
        F: Fn(O) -> O2,
    {
        Map {
            inner: self,
            f,
            _phantom: Default::default(),
        }
    }

    fn end(self) -> impl Parser<'input, O> {
        End { inner: self }
    }
}

impl<'input, O, F> Parser<'input, O> for F
where
    F: Fn(&'input str) -> ParseResult<'input, O>,
{
    fn parse(&self, input: &'input str) -> ParseResult<'input, O> {
        self(input)
    }
}

struct Then<P1, P2> {
    first: P1,
    then: P2,
}

impl<'input, P1, P2, O1, O2> Parser<'input, (O1, O2)> for Then<P1, P2>
where
    P1: Parser<'input, O1>,
    P2: Parser<'input, O2>,
{
    fn parse(&self, input: &'input str) -> ParseResult<'input, (O1, O2)> {
        let (first_output, rest) = self
            .first
            .parse(input)
            .context("first parser unsuccessful")?;
        let (second_output, rest) = self
            .then
            .parse(rest)
            .context("second parser unsuccessful")?;
        let output = (first_output, second_output);
        Ok((output, rest))
    }
}

struct Map<P, F, O> {
    inner: P,
    f: F,
    _phantom: std::marker::PhantomData<O>,
}

impl<'input, P, F, O, N> Parser<'input, N> for Map<P, F, O>
where
    P: Parser<'input, O>,
    F: Fn(O) -> N,
{
    fn parse(&self, input: &'input str) -> ParseResult<'input, N> {
        self.inner
            .parse(input)
            .map(|(output, rest)| ((self.f)(output), rest))
    }
}

struct Literal(&'static str);

impl<'input> Parser<'input, &'input str> for Literal {
    fn parse(&self, input: &'input str) -> ParseResult<'input, &'input str> {
        if let Some(rest) = input.strip_prefix(self.0) {
            let found = &input[..self.0.len()];
            Ok((found, rest))
        } else {
            Err(anyhow::format_err!(
                "expected literal `{}` not found in input",
                self.0
            ))
        }
    }
}

pub fn literal(expected: &'static str) -> impl for<'a> Parser<'a, &'a str> {
    Literal(expected)
}

struct Numeric;

impl<'input> Parser<'input, &'input str> for Numeric {
    fn parse(&self, input: &'input str) -> ParseResult<'input, &'input str> {
        match input.chars().next() {
            Some(c) if c.is_numeric() => {
                let found = &input[..c.len_utf8()];
                let rest = &input[c.len_utf8()..];
                Ok((found, rest))
            }
            _ => Err(anyhow::format_err!("non-numeric character found")),
        }
    }
}

pub fn numeric() -> impl for<'a> Parser<'a, &'a str> {
    Numeric
}

struct ZeroOrMore<P>(P);

impl<'input, P, O> Parser<'input, Vec<O>> for ZeroOrMore<P>
where
    P: Parser<'input, O>,
{
    fn parse(&self, input: &'input str) -> ParseResult<'input, Vec<O>> {
        let mut pos = 0;
        let mut outputs = Vec::new();

        loop {
            let idk = &input[pos..];
            match self.0.parse(idk) {
                Ok((found, rest)) => {
                    let consumed = idk.len() - rest.len();
                    pos += consumed;
                    outputs.push(found);
                }
                Err(_) => break,
            }
        }

        Ok((outputs, &input[pos..]))
    }
}

pub fn zero_or_more<'input, P, O>(parser: P) -> impl Parser<'input, Vec<O>>
where
    P: Parser<'input, O>,
{
    ZeroOrMore(parser)
}

struct OneOrMore<P>(P);

impl<'input, P, O> Parser<'input, Vec<O>> for OneOrMore<P>
where
    P: Parser<'input, O>,
{
    fn parse(&self, input: &'input str) -> ParseResult<'input, Vec<O>> {
        let mut pos = 0;
        let mut outputs = Vec::new();

        loop {
            let idk = &input[pos..];
            match self.0.parse(idk) {
                Ok((found, rest)) => {
                    let consumed = idk.len() - rest.len();
                    pos += consumed;
                    outputs.push(found);
                }
                Err(_) => break,
            }
        }

        if outputs.is_empty() {
            Err(anyhow::format_err!(
                "parser did not find any values it could consume"
            ))
        } else {
            Ok((outputs, &input[pos..]))
        }
    }
}

pub fn one_or_more<'input, P, O>(parser: P) -> impl Parser<'input, Vec<O>>
where
    P: Parser<'input, O>,
{
    OneOrMore(parser)
}

struct NOrMore<P> {
    parser: P,
    times: usize,
}

impl<'input, P, O> Parser<'input, Vec<O>> for NOrMore<P>
where
    P: Parser<'input, O>,
{
    fn parse(&self, input: &'input str) -> ParseResult<'input, Vec<O>> {
        let mut pos = 0;
        let mut outputs = Vec::with_capacity(self.times);

        for _ in 0..self.times {
            let idk = &input[pos..];
            match self.parser.parse(idk) {
                Ok((found, rest)) => {
                    let consumed = idk.len() - rest.len();
                    pos += consumed;
                    outputs.push(found);
                }
                Err(_) => break,
            }
        }

        if outputs.is_empty() {
            Err(anyhow::format_err!(
                "parser did not find any values it could consume"
            ))
        } else {
            Ok((outputs, &input[pos..]))
        }
    }
}

pub fn n_or_more<'input, P, O>(times: usize, parser: P) -> impl Parser<'input, Vec<O>>
where
    P: Parser<'input, O>,
{
    NOrMore { parser, times }
}

struct IntegerParser;

impl<'input> Parser<'input, u64> for IntegerParser {
    fn parse(&self, input: &'input str) -> ParseResult<'input, u64> {
        one_or_more(numeric()).parse(input).map(|(out, rest)| {
            let matched = &input[..out.len()];
            let int = matched
                .parse()
                .expect("output from numeric should parse to int without issue");

            (int, rest)
        })
    }
}

pub fn uint() -> impl for<'a> Parser<'a, u64> {
    IntegerParser
}

struct End<P> {
    inner: P,
}

impl<'input, P, O> Parser<'input, O> for End<P>
where
    P: Parser<'input, O>,
{
    fn parse(&self, input: &'input str) -> ParseResult<'input, O> {
        self.inner
            .parse(input)
            .and_then(|(output, rest)| match rest {
                "" => Ok((output, input)),
                _ => Err(anyhow::format_err!("not end of input")),
            })
    }
}
pub fn end() -> impl for<'a> Parser<'a, &'a str> {
    End { inner: literal("") }
}

#[macro_export]
macro_rules! one_of {
    ($($lit:literal),*) => {
        |input| {
        $(
            match $crate::parsers::literal($lit).parse(input) {
                Ok(inner) => return Ok(inner),
                Err(_) => {},
            };
        )*
            Err(anyhow::format_err!("none of provided options matched"))
        }
    }
}

#[cfg(test)]
mod test {
    use std::time::Duration;

    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_literal() {
        let input = String::from("hello world");
        let (output, rest) = literal("hello").parse(&input).unwrap();

        assert_eq!(output, "hello");
        assert_eq!(rest, " world");
        assert!(literal("goodbye").parse(&input).is_err());
    }

    #[test]
    fn test_numeric() {
        let input = String::from("123");
        let (output, rest) = numeric().parse(&input).unwrap();

        assert_eq!(output, "1");
        assert_eq!(rest, "23");
    }

    #[test]
    fn test_end_method() {
        assert!(end().parse("").is_ok());
        assert!(end().parse("abc").is_err());
    }

    #[test]
    fn test_then_method() {
        let input = String::from("500ms");
        let (output, rest) = uint().then(literal("ms")).parse(&input).unwrap();

        assert_eq!(output.0, 500);
        assert_eq!(output.1, "ms");
        assert!(end().parse(rest).is_ok());
    }

    #[test]
    fn test_map_method() {
        let input = "500ms";

        let (output, _rest) = uint()
            .then(literal("ms"))
            .map(|(int, unit)| match unit {
                "ms" => Duration::from_millis(int),
                _ => unimplemented!("nah man"),
            })
            .end()
            .parse(input)
            .expect("skill issue");

        assert_eq!(output, Duration::from_millis(500));
    }

    #[test]
    fn test_one_of_macro() {
        let inputs = vec!["500ms", "2s", "1000ns", "1000000us"];
        let expected_outputs = vec![
            Duration::from_millis(500),
            Duration::from_secs(2),
            Duration::from_nanos(1000),
            Duration::from_micros(1_000_000),
        ];

        for (input, expected) in inputs.into_iter().zip(expected_outputs) {
            let (output, _rest) = uint()
                .then(one_of!("s", "ms", "ns", "us"))
                .map(|(int, unit)| match unit {
                    "s" => Duration::from_secs(int),
                    "ms" => Duration::from_millis(int),
                    "ns" => Duration::from_nanos(int),
                    "us" => Duration::from_micros(int),
                    _ => unimplemented!("nah man"),
                })
                .end()
                .parse(input)
                .expect("skill issue");

            assert_eq!(output, expected);
        }
    }
}
