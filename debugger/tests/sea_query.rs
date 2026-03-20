mod util;
use util::*;

use anyhow::Result;
use firedbg_rust_debugger::{Bytes, Debugger, Event, EventStream};
use pretty_assertions::assert_eq;
use sea_streamer::{Buffer, Consumer, Message, Producer};

#[tokio::test]
async fn main() -> Result<()> {
    let testcase = "sea_query";
    let (producer, consumer) = setup(testcase).await?;

    let debugger_params = debugger_params_from_file(testcase);

    // println!("{:#?}", debugger_params.breakpoints);

    Debugger::run(debugger_params, producer.clone());

    producer.end().await?;

    // Inlining varies by Rust/LLVM: was ~46, then ~38 (1.81+), now often ~32 for SeaRc::new / trait shims.
    let mut event_count = 0;
    let mut last_function_name = String::new();
    
    loop {
        let message = match consumer.next().await {
            Ok(msg) => msg,
            Err(_) => break, // Stream ended
        };
        let payload = message.message().into_bytes();
        let event = EventStream::read_from(Bytes::from(payload));
        
        match event {
            Event::FunctionCall {
                function_name,
                mut arguments,
                ..
            } => {
                arguments.iter_mut().for_each(|(_, val)| val.redact_addr());
                let json = serde_json::to_string(&arguments).unwrap();
                match event_count {
                    0 => assert_eq!(function_name, "sea_query::main"),
                    1 => assert_eq!(function_name, "sea_query::Query::select"),
                    2 => assert_eq!(function_name, "sea_query::SelectStatement::new"),
                    _ => (),
                }
                match event_count {
                    0..=2 => assert_eq!(json, r#"[]"#),
                    _ => (),
                }
                println!("[{event_count}] {function_name}() -> {arguments:?}");
                last_function_name = function_name.clone();
            }
            Event::FunctionReturn {
                function_name,
                mut return_value,
                ..
            } => {
                return_value.redact_addr();
                let json = serde_json::to_string(&return_value).unwrap();
                match event_count {
                    3 => assert_eq!(function_name, "sea_query::SelectStatement::new"),
                    4 => assert_eq!(function_name, "sea_query::Query::select"),
                    _ => (),
                }
                match event_count {
                    3 => assert_eq!(
                        json,
                        r#"{"type":"Struct","typename":"sea_query::SelectStatement","fields":{"selects":{"type":"Array","typename":"vec","data":[]},"from":{"type":"Array","typename":"vec","data":[]}}}"#
                    ),
                    4 => assert_eq!(
                        json,
                        r#"{"type":"Struct","typename":"sea_query::SelectStatement","fields":{"selects":{"type":"Array","typename":"vec","data":[]},"from":{"type":"Array","typename":"vec","data":[]}}}"#
                    ),
                    _ => (),
                }
                // Check that main returns last
                if function_name == "sea_query::main" {
                    assert_eq!(json, r#"{"type":"Unit"}"#);
                }
                println!("[{event_count}] {function_name}() -> {return_value}");
                last_function_name = function_name.clone();
            }
            e => panic!("Unexpected {e:?}"),
        }
        event_count += 1;
    }
    
    // Verify we got a reasonable number of events and main returned
    assert!(
        event_count >= 30,
        "Expected at least 30 events (inlining lowers the count), got {event_count}"
    );
    assert_eq!(last_function_name, "sea_query::main", "Last function should be main");

    Ok(())
}
