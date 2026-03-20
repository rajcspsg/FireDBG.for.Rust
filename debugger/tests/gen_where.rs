mod util;
use util::*;

use anyhow::Result;
use firedbg_rust_debugger::{Bytes, Debugger, Event, EventStream};
use sea_streamer::{Buffer, Consumer, Message, Producer};

/// Normalize function name by stripping generic parameters.
/// Handles cases where function name may use generic placeholders (T)
/// or concrete types (Vec<i32>) depending on Rust version.
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
    let testcase = "gen_where";
    let (producer, consumer) = setup(testcase).await?;

    let debugger_params = debugger_params_from_file(testcase);

    Debugger::run(debugger_params, producer.clone());

    producer.end().await?;

    for i in 0..10 {
        let payload = consumer.next().await?.message().into_bytes();
        let event = EventStream::read_from(Bytes::from(payload));
        println!("#{i} {:?}", event);

        match &event {
            Event::Breakpoint { .. } => (),
            Event::FunctionCall { function_name, .. } => {
                let expected = match i {
                    0 => "gen_where::main",
                    // May appear as <T as PrintInOption> or <Vec<i32> as PrintInOption>
                    1 | 3 | 5 | 7 => "<_ as gen_where::PrintInOption>::print_in_option",
                    _ => panic!("Unexpected i {i}"),
                };
                assert_fn_name(function_name, expected);
            }
            Event::FunctionReturn { function_name, .. } => {
                let expected = match i {
                    2 | 4 | 6 | 8 => "<_ as gen_where::PrintInOption>::print_in_option",
                    9 => "gen_where::main",
                    _ => panic!("Unexpected i {i}"),
                };
                assert_fn_name(function_name, expected);
            }
        }
    }

    Ok(())
}
