use anyhow::Context;

pub type ParseResult<'input, O> = anyhow::Result<(&'input str, O)>;

pub trait Parser<'input, O>: Sized {
    fn parse(&self, input: &'input str) -> ParseResult<'input, O>;

    fn then<P, O2>(self, then: P) -> impl Parser<'input, (O, O2)>
    where
        P: Parser<'input, O2>,
    {
        move |input| {
            let (rest, first) = self.parse(input).context("first parser unsuccessful")?;
            let (rest, then) = then.parse(rest).context("second parser unsuccessful")?;
            Ok((rest, (first, then)))
        }
    }

    fn map<F, O2>(self, transform: F) -> impl Parser<'input, O2>
    where
        F: Fn(O) -> O2,
    {
        move |input| {
            self.parse(input)
                .map(|(rest, output)| (rest, transform(output)))
        }
    }

    fn end(self) -> impl Parser<'input, O> {
        self.then(end()).map(|(out, _)| out)
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

impl<'input> Parser<'input, &'input str> for &'static str {
    fn parse(&self, input: &'input str) -> ParseResult<'input, &'input str> {
        if let Some(rest) = input.strip_prefix(self) {
            let found = &input[..self.len()];
            Ok((rest, found))
        } else {
            Err(anyhow::format_err!(
                "expected literal `{}` not found in input",
                self
            ))
        }
    }
}

pub fn literal(expected: &'static str) -> impl for<'a> Parser<'a, &'a str> {
    expected
}

pub fn match_char_where<'input, F>(pred: F) -> impl Parser<'input, &'input str>
where
    F: Fn(char) -> bool,
{
    move |input: &'input str| match input.chars().next() {
        Some(c) if pred(c) => {
            let found = &input[..c.len_utf8()];
            let rest = &input[c.len_utf8()..];
            Ok((rest, found))
        }
        Some(c) => Err(anyhow::format_err!("`{}` does not satisfy predicate", c)),
        None => Err(anyhow::format_err!("unexpected end of input")),
    }
}
pub fn numeric<'input>() -> impl Parser<'input, &'input str> {
    match_char_where(|c| c.is_numeric())
}

fn one_or_more<'input, P, O>(parser: P) -> impl Parser<'input, &'input str>
where
    P: Parser<'input, O>,
{
    move |input| {
        let mut consumed = 0;
        let mut remaining = input;

        while let Ok((rest, _)) = parser.parse(remaining) {
            consumed += remaining.len() - rest.len();
            remaining = rest;
        }

        if consumed == 0 {
            Err(anyhow::format_err!(
                "parser did not find any values it could consume"
            ))
        } else {
            Ok((remaining, &input[..consumed]))
        }
    }
}

pub fn uint<'input>() -> impl Parser<'input, u64> {
    one_or_more(numeric()).map(|out| {
        out.parse()
            .expect("output from numeric should parse to int without issue")
    })
}

pub fn end<'input>() -> impl Parser<'input, ()> {
    |input| match input {
        "" => Ok((input, ())),
        _ => Err(anyhow::format_err!("not end of input")),
    }
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

#[macro_export]
macro_rules! map_one_of {
    ($($lit:literal => $to:expr$(,)?)*) => {
        $crate::one_of!($($lit),*).map(|out| match out {
            $(
                $lit => $to,
            )*
            _ => unreachable!(),
        })
    }
}

#[cfg(test)]
mod test {
    use std::time::Duration;

    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_literal() {
        let input = "hello world";
        let (rest, output) = literal("hello").parse(input).unwrap();

        assert_eq!(output, "hello");
        assert_eq!(rest, " world");
        assert!(literal("goodbye").parse(input).is_err());
    }

    #[test]
    fn test_numeric() {
        let input = "123";
        let (rest, output) = numeric().parse(input).unwrap();

        assert_eq!(output, "1");
        assert_eq!(rest, "23");
    }

    #[test]
    fn test_end_method() {
        let input = "abc";

        let (rest, output) = literal("abc").end().parse(input).unwrap();
        assert_eq!(output, "abc");
        assert_eq!(rest, "");
    }

    #[test]
    fn test_then_method() {
        let input = "500ms";
        let (rest, output) = uint().then("ms").parse(input).unwrap();

        assert_eq!(output.0, 500);
        assert_eq!(output.1, "ms");
        assert!(end().parse(rest).is_ok());
    }

    #[test]
    fn test_map_method() {
        let input = "500ms";

        let (_rest, output) = uint()
            .then("ms")
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
            let (_rest, output) = uint()
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

    #[test]
    fn test_map_one_of_macro() {
        let inputs = vec!["500ms", "2s", "1000ns", "1000000us"];
        let expected_outputs = vec![
            Duration::from_millis(500),
            Duration::from_secs(2),
            Duration::from_nanos(1000),
            Duration::from_micros(1_000_000),
        ];

        for (input, expected) in inputs.into_iter().zip(expected_outputs) {
            let (_rest, output) = uint()
                .then(map_one_of!(
                    "s" => Duration::from_secs,
                    "ms" => Duration::from_millis,
                    "ns" => Duration::from_nanos,
                    "us" => Duration::from_micros,
                ))
                .map(|(amt, duration_fn)| duration_fn(amt))
                .end()
                .parse(input)
                .expect("skill issue");

            assert_eq!(output, expected);
        }
    }
}
