use anyhow::{Context, Result};
use rmpv::Value;

/// Handle a Python -> Rust RPC request.
///
/// Request:
/// [u16 request_id, string method, array arguments]
///
/// Response:
/// [u16 request_id, i32 code, string message, data]
pub fn dispatch(payload: &[u8]) -> Result<Vec<u8>> {
    let (request_id, method, arguments): (u16, String, Vec<Value>) =
        rmp_serde::from_slice(payload).context("Invalid IPC request")?;

    match method.as_str() {
        "engine.ping" => {
            if !arguments.is_empty() {
                return response_error(request_id, 400, "engine.ping does not accept arguments");
            }

            response_ok(request_id, "pong")
        }

        _ => response_error(request_id, 404, &format!("Unknown engine method: {method}")),
    }
}

fn response_ok(request_id: u16, data: &str) -> Result<Vec<u8>> {
    Ok(rmp_serde::to_vec(&(request_id, 0_i32, "ok", data))?)
}

fn response_error(request_id: u16, code: i32, message: &str) -> Result<Vec<u8>> {
    Ok(rmp_serde::to_vec(&(
        request_id,
        code,
        message,
        Option::<String>::None,
    ))?)
}
