mod util;
use util::*;

use anyhow::Result;
use firedbg_rust_debugger::{Bytes, Debugger, Event, EventStream};
use sea_streamer::{Buffer, Consumer, Message, Producer};

/// Normalize function name by stripping generic parameters.
/// This handles cases where the function name may use generic placeholders (Unit)
/// or concrete types (Inch, Mm) depending on Rust version/optimization.
fn normalize_fn_name(name: &str) -> String {
    let mut result = String::new();
    let mut depth = 0;
    for c in name.chars() {
        if c == '<' {
            depth += 1;
        } else if c == '>' {
            depth -= 1;
        } else if depth == 0 {
            result.push(c);
        }
    }
    result
}

fn assert_fn_name(actual: &str, expected: &str) {
    let actual_normalized = normalize_fn_name(actual);
    let expected_normalized = normalize_fn_name(expected);
    assert_eq!(
        actual_normalized, expected_normalized,
        "Function name mismatch: actual='{}', expected='{}'",
        actual, expected
    );
}

#[tokio::test]
async fn main() -> Result<()> {
    let testcase = "gen_phantom";
    let (producer, consumer) = setup(testcase).await?;

    let debugger_params = debugger_params_from_file(testcase);

    Debugger::run(debugger_params, producer.clone());

    producer.end().await?;

    for i in 0..6 {
        let payload = consumer.next().await?.message().into_bytes();
        let event = EventStream::read_from(Bytes::from(payload));
        println!("#{i} {:?}", event);

        match &event {
            Event::Breakpoint { .. } => (),
            Event::FunctionCall { function_name, .. } => {
                // Normalize to strip generic params - may be Unit, Inch, or Mm depending on version
                let expected = match i {
                    0 => "gen_phantom::main",
                    1 | 3 => "<gen_phantom::Length<_> as core::ops::arith::Add>::add",
                    _ => panic!("Unexpected i {i}"),
                };
                assert_fn_name(function_name, expected);
            }
            Event::FunctionReturn { function_name, .. } => {
                let expected = match i {
                    2 | 4 => "<gen_phantom::Length<_> as core::ops::arith::Add>::add",
                    5 => "gen_phantom::main",
                    _ => panic!("Unexpected i {i}"),
                };
                assert_fn_name(function_name, expected);
            }
        }
    }

    Ok(())
}
