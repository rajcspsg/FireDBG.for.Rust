mod util;
use util::*;

use anyhow::Result;
use firedbg_rust_debugger::{Bytes, Debugger, Event, EventStream};
use sea_streamer::{Buffer, Consumer, Message, Producer};

/// Normalize function name to handle Rust version differences.
/// Rust 1.93+ uses `<Type>::method` format for inherent impls.
/// Also strips generic parameters from type names.
fn normalize_fn_name(name: &str) -> String {
    let mut result = name.to_string();

    // Convert `<path::Type>::method` or `<path::Type<T>>::method` to `path::Type::method`
    if let Some(rest) = result.strip_prefix('<') {
        if let Some(pos) = rest.find(">::") {
            let type_part = &rest[..pos];
            let method_part = &rest[pos + 3..];
            result = format!("{}::{}", type_part, method_part);
        }
    }

    // Strip trailing generic parameters like `::<Type>`
    if let Some(pos) = result.find("::<") {
        result = result[..pos].to_string();
    }

    // Strip generic parameters from type names like `GenVal<T>` or `GenVal<i32>` -> `GenVal`
    // This handles cases where the expected uses `<T>` but actual uses `<i32>`
    let mut normalized = String::new();
    let mut depth = 0;
    let mut chars = result.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '<' {
            // Check if this is a generic parameter (not part of `>::`)
            depth += 1;
        } else if c == '>' {
            depth -= 1;
        } else if depth == 0 {
            normalized.push(c);
        }
    }

    normalized
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
    let testcase = "gen_impl";
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
                let expected = match i {
                    0 => "gen_impl::main",
                    1 => "gen_impl::Val::value",
                    3 => "gen_impl::GenVal<T>::value",
                    _ => panic!("Unexpected i {i}"),
                };
                assert_fn_name(function_name, expected);
            }
            Event::FunctionReturn { function_name, .. } => {
                let expected = match i {
                    2 => "gen_impl::Val::value",
                    4 => "gen_impl::GenVal<T>::value",
                    5 => "gen_impl::main",
                    _ => panic!("Unexpected i {i}"),
                };
                assert_fn_name(function_name, expected);
            }
        }
    }

    Ok(())
}
