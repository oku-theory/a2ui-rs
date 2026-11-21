//! Fuzzes `AgentMessage::parse` with arbitrary text, then the error message built from a refusal.
//!
//! The text is read under the default limits. A refusal becomes the renderer's `error` message,
//! which must build and render to JSON; a panic or a hang is the finding.

#![no_main]

use a2ui_runtime::{AgentMessage, MessageLimits, RendererMessage};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    if let Err(error) = AgentMessage::parse(text, &MessageLimits::default()) {
        let _ = RendererMessage::error(&error).to_json();
    }
});
