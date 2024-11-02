mod parsers;
mod ticker;

use anyhow::Context;
use clap::Parser;
use parsers::{uint, Parser as _Parser};
use std::time::Duration;
use tokio::time::interval;

#[derive(Parser)]
struct Args {
    /// URL of the service to barrage
    //addr: String,

    /// JSON payload/template to barrage `addr` with
    #[arg(short, long)]
    data: serde_json::Value,

    /// How often to send requests to `addr` (Ex. "500ms")
    #[arg(long, value_parser = parse_duration)]
    every: Duration,
}

fn duration<'input>() -> impl parsers::Parser<'input, Duration> {
    uint()
        .then(one_of!("s", "ms", "ns", "us"))
        .map(|(int, unit)| match unit {
            "s" => Duration::from_secs(int),
            "ms" => Duration::from_millis(int),
            "ns" => Duration::from_nanos(int),
            "us" => Duration::from_micros(int),
            _ => unreachable!(),
        })
}

fn parse_duration(s: &str) -> Result<Duration, anyhow::Error> {
    duration()
        .end()
        .parse(s)
        .map(|(out, _)| out)
        .context("cannot parse to duration value")
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    let mut interval = interval(args.every);

    loop {
        tokio::select! {
            _ = interval.tick() => {
                println!("{}", args.data);
            },
            _ = tokio::signal::ctrl_c() => {
                break;
            }
        }
    }

    println!("cancelled");
}

#[cfg(test)]
mod test {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_parse_duration() {
        let inputs = vec!["500ms", "1000000us"];

        let expected_outputs = vec![Duration::from_millis(500), Duration::from_micros(1_000_000)];

        for (input, expected) in inputs.into_iter().zip(expected_outputs.into_iter()) {
            let output = parse_duration(input).unwrap();
            assert_eq!(expected, output);
        }
    }
}
