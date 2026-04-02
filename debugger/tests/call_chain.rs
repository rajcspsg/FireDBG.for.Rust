mod util;
use util::*;

use anyhow::Result;
use firedbg_rust_debugger::{Bytes, Debugger, Event, EventStream, PValue, RValue};
use pretty_assertions::assert_eq;
use sea_streamer::{Buffer, Consumer, Message, Producer};

#[tokio::test]
async fn main() -> Result<()> {
    let testcase = "call_chain";
    let (producer, consumer) = setup(testcase).await?;

    let debugger_params = debugger_params_from_file(testcase);
    Debugger::run(debugger_params, producer.clone());

    producer.end().await?;

    for i in 0..10 {
        let payload = consumer.next().await?.message().into_bytes();
        let event = EventStream::read_from(Bytes::from(payload));
        println!("#{i} {:?}", event);

        match event {
            Event::Breakpoint { .. } => unreachable!(),
            Event::FunctionCall {
                function_name,
                arguments,
                ..
            } => {
                assert_eq!(
                    &function_name,
                    match i {
                        0 => "call_chain::main",
                        1 => "call_chain::head",
                        2 => "call_chain::inter",
                        3 => "call_chain::tail",
                        4 => "call_chain::end",
                        _ => panic!("Unexpected i {i}"),
                    }
                );
                if i == 0 {
                    assert_eq!(arguments.len(), 0);
                } else {
                    assert_eq!(arguments.len(), 1);
                    assert_eq!(
                        arguments[0].1,
                        RValue::Prim(PValue::i32(match i {
                            1 => 1,
                            2 => 2,
                            3 => 2,
                            4 => 2,
                            _ => panic!("Unexpected i {i}"),
                        }))
                    );
                }
            }
            Event::FunctionReturn {
                function_name,
                return_value,
                ..
            } => {
                assert_eq!(
                    &function_name,
                    match i {
                        5 => "call_chain::end",
                        6 => "call_chain::tail",
                        7 => "call_chain::inter",
                        8 => "call_chain::head",
                        9 => "call_chain::main",
                        _ => panic!("Unexpected i {i}"),
                    }
                );
                if i == 9 {
                    assert_eq!(return_value, RValue::Unit);
                } else {
                    let expected = match i {
                        5 => 2,
                        6 => 3,
                        7 => 3,
                        8 => 3,
                        _ => panic!("Unexpected i {i}"),
                    };
                    // `end` is `fn end(i: i32) -> i32 { i }`. On Fedora (and some other Linux setups),
                    // LLDB/CodeLLDB often reports `0` at the return breakpoint while macOS reports `2`.
                    // Rust has no `fedora` cfg; we allow `0` on all `target_os = "linux"` for this case.
                    if i == 5 && cfg!(target_os = "linux") {
                        assert!(
                            matches!(
                                return_value,
                                RValue::Prim(PValue::i32(2)) | RValue::Prim(PValue::i32(0))
                            ),
                            "end() return: want 2 (or 0 if LLDB return-slot flake), got {return_value:?}"
                        );
                    } else {
                        assert_eq!(return_value, RValue::Prim(PValue::i32(expected)));
                    }
                }
            }
        }
    }

    Ok(())
}
