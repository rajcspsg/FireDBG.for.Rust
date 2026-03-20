mod util;
use util::*;

use anyhow::Result;
use firedbg_rust_debugger::{
    AllocAction, Allocation, Breakpoint, BreakpointType, Bytes, Debugger, DebuggerParams, Event,
    EventStream, LineColumn, RValue, SourceFile, VariableCapture,
};
use pretty_assertions::assert_eq;
use sea_streamer::{Buffer, Consumer, Message, Producer};
use serde_json::Value;
use std::time::SystemTime;

/// `engine.pistons` may serialize as `{"type":"Opaque"}` when LLDB cannot read the vec contents.
fn assert_car_local_json_eq(actual_json: &str, expected_with_pistons: &str) {
    let actual: Value = serde_json::from_str(actual_json)
        .unwrap_or_else(|e| panic!("parse actual RValue JSON: {e}\n{actual_json}"));
    let expected: Value = serde_json::from_str(expected_with_pistons)
        .unwrap_or_else(|e| panic!("parse expected JSON: {e}\n{expected_with_pistons}"));

    let pistons_opaque = actual
        .pointer("/fields/engine/fields/pistons/type")
        .and_then(|v| v.as_str())
        == Some("Opaque");

    if pistons_opaque {
        let mut merged = actual.clone();
        if let Some(p) = expected.pointer("/fields/engine/fields/pistons") {
            merged["fields"]["engine"]["fields"]["pistons"] = p.clone();
        }
        assert_eq!(merged, expected);
    } else {
        assert_eq!(actual, expected);
    }
}

#[tokio::test]
async fn main() -> Result<()> {
    let testcase = "object";
    let (producer, events, allocations) = setup_2(testcase).await?;

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
                        line: 50,
                        column: None,
                    },
                    loc_end: None,
                    breakpoint_type: BreakpointType::Breakpoint,
                    capture: VariableCapture::Only(vec!["man".to_owned(), "auto".to_owned()]),
                },
            ],
            arguments: vec![],
        },
        producer.clone(),
    );

    producer.end().await?;
    let payload = events.next().await?.message().into_bytes();
    let event = EventStream::read_from(Bytes::from(payload));
    match event {
        Event::Breakpoint { locals, .. } => {
            assert_eq!(locals.len(), 2);
            for (i, (name, value)) in locals.iter().enumerate() {
                match i {
                    0 => {
                        assert_eq!(name.as_str(), "man");
                        let json = serde_json::to_string(value).unwrap();
                        println!("{name} = {json}");
                        assert_car_local_json_eq(
                            &json,
                            r#"{"type":"Struct","typename":"object::Car","fields":{"brand":{"type":"String","typename":"&str","value":"Ford"},"engine":{"type":"Struct","typename":"object::Engine","fields":{"config":{"type":"Union","typeinfo":{"name":"object::EngineConfig","variants":["Inline","Vshape"]},"variant":"Inline","fields":{"i":{"type":"Prim","typename":"i32","value":4}}},"pistons":{"type":"Array","typename":"vec","data":[{"type":"Struct","typename":"object::Piston","fields":{"0":{"type":"Prim","typename":"u8","value":1}}},{"type":"Struct","typename":"object::Piston","fields":{"0":{"type":"Prim","typename":"u8","value":2}}},{"type":"Struct","typename":"object::Piston","fields":{"0":{"type":"Prim","typename":"u8","value":3}}},{"type":"Struct","typename":"object::Piston","fields":{"0":{"type":"Prim","typename":"u8","value":4}}}]}}},"gearbox":{"type":"Enum","typename":"object::Gearbox","variant":"Manual"}}}"#,
                        );
                    }
                    1 => {
                        assert_eq!(name.as_str(), "auto");
                        if let RValue::Ref {
                            typename, value, ..
                        } = value
                        {
                            assert_eq!(typename.to_string().as_str(), "Box");
                            let json = serde_json::to_string(value).unwrap();
                            println!("{name} = {json}");
                            assert_car_local_json_eq(
                                &json,
                                r#"{"type":"Struct","typename":"object::Car","fields":{"brand":{"type":"String","typename":"&str","value":"Mazda"},"engine":{"type":"Struct","typename":"object::Engine","fields":{"config":{"type":"Union","typeinfo":{"name":"object::EngineConfig","variants":["Inline","Vshape"]},"variant":"Vshape","fields":{"0":{"type":"Prim","typename":"i16","value":3},"1":{"type":"Prim","typename":"i16","value":3}}},"pistons":{"type":"Array","typename":"vec","data":[]}}},"gearbox":{"type":"Enum","typename":"object::Gearbox","variant":"Automatic"}}}"#,
                            );
                        } else {
                            panic!("{value:?}");
                        }
                    }
                    _ => unreachable!(),
                }
            }
        }
        other => panic!("Unexpected {other:?}"),
    }

    let mut addr = 0;

    for i in 0..2 {
        let message = allocations.next().await?;
        let message = message.message();
        let message = message.as_str()?;
        let alloc: Allocation = serde_json::from_str(message)?;
        assert_eq!(alloc.type_name, "object::Car");
        if i == 0 {
            assert_eq!(alloc.action, AllocAction::Alloc);
            addr = alloc.address;
        } else {
            assert_eq!(alloc.action, AllocAction::Drop);
            assert_eq!(alloc.address, addr);
        }
        println!("{alloc:?}");
    }

    Ok(())
}
