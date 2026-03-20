mod util;
use util::*;

use anyhow::Result;
use firedbg_rust_debugger::{Bytes, Debugger, Event, EventStream};
use sea_streamer::{Buffer, Consumer, Message, Producer};

/// Normalize function name to handle Rust version differences.
/// Rust 1.93+ uses `<Type>::method` format for inherent impls.
fn normalize_fn_name(name: &str) -> String {
    let mut result = name.to_string();

    // Convert `<path::Type>::method` to `path::Type::method`
    if let Some(rest) = result.strip_prefix('<') {
        if let Some(pos) = rest.find(">::") {
            let type_part = &rest[..pos];
            let method_part = &rest[pos + 3..];
            result = format!("{}::{}", type_part, method_part);
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
    let testcase = "gen_new_type";
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
                    0 => "gen_new_type::main",
                    1 => "gen_new_type::Years::to_days",
                    3 => "gen_new_type::old_enough",
                    5 => "gen_new_type::Days::to_years",
                    7 => "gen_new_type::old_enough",
                    _ => panic!("Unexpected i {i}"),
                };
                assert_fn_name(function_name, expected);
            }
            Event::FunctionReturn { function_name, .. } => {
                let expected = match i {
                    2 => "gen_new_type::Years::to_days",
                    4 => "gen_new_type::old_enough",
                    6 => "gen_new_type::Days::to_years",
                    8 => "gen_new_type::old_enough",
                    9 => "gen_new_type::main",
                    _ => panic!("Unexpected i {i}"),
                };
                assert_fn_name(function_name, expected);
            }
        }
    }

    Ok(())
}
