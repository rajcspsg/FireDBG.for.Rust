mod util;
use util::*;

use anyhow::Result;
use firedbg_rust_debugger::{Bytes, Debugger, Event, EventStream};
use sea_streamer::{Buffer, Consumer, Message, Producer};

/// Normalize function name by stripping trailing generic parameters like `::<Type>`.
fn normalize_fn_name(name: &str) -> String {
    if let Some(pos) = name.find("::<") {
        name[..pos].to_string()
    } else {
        name.to_string()
    }
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
    let testcase = "gen_multi_bound";
    let (producer, consumer) = setup(testcase).await?;

    let debugger_params = debugger_params_from_file(testcase);

    Debugger::run(debugger_params, producer.clone());

    producer.end().await?;

    for i in 0..12 {
        let payload = consumer.next().await?.message().into_bytes();
        let event = EventStream::read_from(Bytes::from(payload));
        println!("#{i} {:?}", event);

        match &event {
            Event::Breakpoint { .. } => (),
            Event::FunctionCall { function_name, .. } => {
                let expected = match i {
                    0 => "gen_multi_bound::main",
                    1 => "gen_multi_bound::compare_prints",
                    3 => "gen_multi_bound::compare_prints",
                    5 => "gen_multi_bound::compare_prints",
                    7 => "gen_multi_bound::compare_types",
                    9 => "gen_multi_bound::compare_types",
                    _ => panic!("Unexpected i {i}"),
                };
                assert_fn_name(function_name, expected);
            }
            Event::FunctionReturn { function_name, .. } => {
                let expected = match i {
                    2 => "gen_multi_bound::compare_prints",
                    4 => "gen_multi_bound::compare_prints",
                    6 => "gen_multi_bound::compare_prints",
                    8 => "gen_multi_bound::compare_types",
                    10 => "gen_multi_bound::compare_types",
                    11 => "gen_multi_bound::main",
                    _ => panic!("Unexpected i {i}"),
                };
                assert_fn_name(function_name, expected);
            }
        }
    }

    Ok(())
}
