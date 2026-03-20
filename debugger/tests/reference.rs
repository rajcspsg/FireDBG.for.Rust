mod util;
use util::*;

use anyhow::Result;
use firedbg_rust_debugger::{
    Breakpoint, BreakpointType, Bytes, Debugger, DebuggerParams, Event, EventStream, LineColumn,
    SourceFile, VariableCapture,
};
use pretty_assertions::assert_eq;
use sea_streamer::{Buffer, Consumer, Message, Producer};
use serde_json::Value;
use std::time::SystemTime;

/// LLDB may not expand `Train.cargo` (`Vec`); field serializes as `{"type":"Opaque"}`.
fn assert_train_local_json_eq(actual_json: &str, expected_json: &str) {
    let actual: Value = serde_json::from_str(actual_json)
        .unwrap_or_else(|e| panic!("parse actual: {e}\n{actual_json}"));
    let expected: Value = serde_json::from_str(expected_json)
        .unwrap_or_else(|e| panic!("parse expected: {e}\n{expected_json}"));

    let cargo_opaque = actual
        .pointer("/value/fields/cargo/type")
        .and_then(|v| v.as_str())
        == Some("Opaque");

    if cargo_opaque {
        let mut merged = actual.clone();
        if let Some(c) = expected.pointer("/value/fields/cargo") {
            merged["value"]["fields"]["cargo"] = c.clone();
        }
        assert_eq!(merged, expected);
    } else {
        assert_eq!(actual, expected);
    }
}

#[tokio::test]
async fn main() -> Result<()> {
    let testcase = "reference";
    let (producer, consumer) = setup(testcase).await?;

    rustc(&format!("testcases/{testcase}"));

    Debugger::run(
        DebuggerParams {
            binary: format!("testcases/{testcase}.o"),
            files: vec![
                Default::default(),
                SourceFile {
                    id: 1,
                    path: format!("testcases/{testcase}.rs"),
                    crate_name: testcase.into(),
                    modified: SystemTime::UNIX_EPOCH,
                },
            ],
            breakpoints: vec![
                Default::default(),
                Breakpoint {
                    id: 1,
                    file_id: 1,
                    loc: LineColumn {
                        line: 21,
                        column: None,
                    },
                    loc_end: None,
                    breakpoint_type: BreakpointType::Breakpoint,
                    capture: VariableCapture::Only(vec!["train".to_owned()]),
                },
                Breakpoint {
                    id: 2,
                    file_id: 1,
                    loc: LineColumn {
                        line: 25,
                        column: None,
                    },
                    loc_end: None,
                    breakpoint_type: BreakpointType::Breakpoint,
                    capture: VariableCapture::Arguments,
                },
            ],
            arguments: vec![],
        },
        producer.clone(),
    );

    producer.end().await?;
    for i in 0..2 {
        let payload = consumer.next().await?.message().into_bytes();
        let event = EventStream::read_from(Bytes::from(payload));
        match event {
            Event::Breakpoint { mut locals, .. } => match i {
                0 => {
                    assert_eq!(locals.len(), 1);
                    for (j, (name, value)) in locals.iter_mut().enumerate() {
                        match j {
                            0 => {
                                assert_eq!(name.as_str(), "train");
                                value.redact_addr();
                                let json = serde_json::to_string(value).unwrap();
                                println!("{name} = {json}");
                                assert_train_local_json_eq(
                                    &json,
                                    r#"{"type":"Ref","typename":"ref","addr":"<redacted>","value":{"type":"Struct","typename":"reference::Train","fields":{"head":{"type":"Ref","typename":"ref","addr":"<redacted>","value":{"type":"Struct","typename":"reference::Head","fields":{"0":{"type":"Ref","typename":"ref","addr":"<redacted>","value":{"type":"Struct","typename":"reference::Label","fields":{"0":{"type":"String","typename":"&str","value":"Bullet"}}}}}}},"cargo":{"type":"Array","typename":"vec","data":[{"type":"Struct","typename":"reference::Cargo","fields":{"payload":{"type":"Prim","typename":"u8","value":1}}},{"type":"Struct","typename":"reference::Cargo","fields":{"payload":{"type":"Prim","typename":"u8","value":2}}},{"type":"Struct","typename":"reference::Cargo","fields":{"payload":{"type":"Prim","typename":"u8","value":3}}},{"type":"Struct","typename":"reference::Cargo","fields":{"payload":{"type":"Prim","typename":"u8","value":4}}}]},"tail":{"type":"Ref","typename":"ref","addr":"<redacted>","value":{"type":"Struct","typename":"reference::Tail","fields":{"label":{"type":"Ref","typename":"ref","addr":"<redacted>","value":{"type":"Struct","typename":"reference::Label","fields":{"0":{"type":"String","typename":"&str","value":"Bullet"}}}},"end":{"type":"Prim","typename":"i32","value":88888}}}}}}}"#,
                                );
                            }
                            _ => unreachable!(),
                        }
                    }
                }
                1 => {
                    assert_eq!(locals.len(), 2);
                    for (j, (name, value)) in locals.iter_mut().enumerate() {
                        value.redact_addr();
                        let json = serde_json::to_string(value).unwrap();
                        println!("{name} = {json}");
                        match j {
                            0 => {
                                assert_eq!(name.as_str(), "head");
                                assert_eq!(
                                    json,
                                    r#"{"type":"Ref","typename":"ref","addr":"<redacted>","value":{"type":"Struct","typename":"reference::Head","fields":{"0":{"type":"Ref","typename":"ref","addr":"<redacted>","value":{"type":"Struct","typename":"reference::Label","fields":{"0":{"type":"String","typename":"&str","value":"Bullet"}}}}}}}"#
                                );
                            }
                            1 => {
                                assert_eq!(name.as_str(), "tail");
                                assert_eq!(
                                    json,
                                    r#"{"type":"Ref","typename":"ref","addr":"<redacted>","value":{"type":"Struct","typename":"reference::Tail","fields":{"label":{"type":"Ref","typename":"ref","addr":"<redacted>","value":{"type":"Struct","typename":"reference::Label","fields":{"0":{"type":"String","typename":"&str","value":"Bullet"}}}},"end":{"type":"Prim","typename":"i32","value":88888}}}}"#
                                );
                            }
                            _ => unreachable!(),
                        }
                    }
                }
                _ => unreachable!(),
            },
            other => panic!("Unexpected {other:?}"),
        }
    }

    Ok(())
}
