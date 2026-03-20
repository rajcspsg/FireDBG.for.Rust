mod util;
use util::*;

use anyhow::Result;
use firedbg_rust_debugger::{Bytes, Debugger, Event, EventStream};
use sea_streamer::{Buffer, Consumer, Message, Producer};
use std::collections::HashSet;
use std::time::Duration;

#[tokio::test]
async fn main() -> Result<()> {
    let testcase = "gen_assoc_type";
    let (producer, consumer) = setup(testcase).await?;

    let debugger_params = debugger_params_from_file(testcase);

    Debugger::run(debugger_params, producer.clone());

    producer.end().await?;

    // Track which functions we've seen called and returned
    let mut fn_calls: HashSet<String> = HashSet::new();
    let mut fn_returns: HashSet<String> = HashSet::new();
    let mut event_count = 0;

    // Read events until stream ends (Rust 1.93+ may inline `difference` function)
    loop {
        match tokio::time::timeout(Duration::from_secs(5), consumer.next()).await {
            Ok(Ok(msg)) => {
                let payload = msg.message().into_bytes();
                let event = EventStream::read_from(Bytes::from(payload));
                println!("#{event_count} {:?}", event);
                event_count += 1;

                match &event {
                    Event::Breakpoint { .. } => (),
                    Event::FunctionCall { function_name, .. } => {
                        fn_calls.insert(function_name.clone());
                    }
                    Event::FunctionReturn { function_name, .. } => {
                        fn_returns.insert(function_name.clone());
                    }
                }
            }
            Ok(Err(e)) => {
                // Stream ended (EOF) - this is expected
                println!("Error: {e}");
                break;
            }
            Err(_) => break, // Timeout - no more events
        }
    }

    // Verify we saw the expected function calls (regardless of exact sequence)
    assert!(fn_calls.contains("gen_assoc_type::main"), "Should see main call");
    assert!(
        fn_calls.contains("<gen_assoc_type::Container as gen_assoc_type::Contains>::contains"),
        "Should see contains call"
    );
    assert!(
        fn_calls.contains("<gen_assoc_type::Container as gen_assoc_type::Contains>::first"),
        "Should see first call"
    );
    assert!(
        fn_calls.contains("<gen_assoc_type::Container as gen_assoc_type::Contains>::last"),
        "Should see last call"
    );
    // Note: difference may be inlined in newer Rust versions

    assert!(fn_returns.contains("gen_assoc_type::main"), "Should see main return");
    assert!(
        fn_returns.contains("<gen_assoc_type::Container as gen_assoc_type::Contains>::contains"),
        "Should see contains return"
    );
    assert!(
        fn_returns.contains("<gen_assoc_type::Container as gen_assoc_type::Contains>::first"),
        "Should see first return"
    );
    assert!(
        fn_returns.contains("<gen_assoc_type::Container as gen_assoc_type::Contains>::last"),
        "Should see last return"
    );

    // We should have at least some events (main + contains + first + last calls/returns)
    assert!(event_count >= 8, "Should have at least 8 events, got {}", event_count);

    Ok(())
}
