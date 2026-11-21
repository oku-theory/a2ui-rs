//! Reads each action's body into its typed form, once the schema has accepted the message.

use serde_json::{Map, Value};

use super::envelope::Action;
use super::{
    AgentFunctionResponse, AgentMessage, CallRendererFunction, CreateSurface, DeleteSurface,
    UpdateComponents, UpdateDataModel,
};
use crate::error::{Error, MessageFieldSnafu};
use crate::pointer::Pointer;
use crate::quoted::reason;

/// Reads `body`, the value of `action`'s key in a message the schema accepted.
pub(crate) fn read(action: Action, body: &Value) -> Result<AgentMessage, Error> {
    let body = object(action, body)?;
    Ok(match action {
        Action::CreateSurface => AgentMessage::CreateSurface(create_surface(body)?),
        Action::UpdateComponents => AgentMessage::UpdateComponents(UpdateComponents {
            surface_id: string(body, "surfaceId")?,
            components: components(body)?,
        }),
        Action::UpdateDataModel => AgentMessage::UpdateDataModel(update_data_model(body)?),
        Action::DeleteSurface => AgentMessage::DeleteSurface(DeleteSurface {
            surface_id: string(body, "surfaceId")?,
        }),
        Action::CallRendererFunction => {
            AgentMessage::CallRendererFunction(call_renderer_function(body)?)
        }
        Action::AgentFunctionResponse => {
            AgentMessage::AgentFunctionResponse(AgentFunctionResponse { body: body.clone() })
        }
    })
}

/// A field the schema guaranteed but the value lacks is a bug in this layer: it is named.
fn missing(name: &'static str) -> Error {
    let reason = reason!["Validated message lacks a usable '", name, "'"];
    MessageFieldSnafu { reason }.build().into()
}

fn object(action: Action, body: &Value) -> Result<&Map<String, Value>, Error> {
    body.as_object().ok_or_else(|| missing(action.key()))
}

fn string(body: &Map<String, Value>, name: &'static str) -> Result<String, Error> {
    body.get(name)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| missing(name))
}

fn optional_string(body: &Map<String, Value>, name: &'static str) -> Result<Option<String>, Error> {
    body.get(name)
        .map(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| missing(name))
        })
        .transpose()
}

fn optional_object(
    body: &Map<String, Value>,
    name: &'static str,
) -> Result<Option<Map<String, Value>>, Error> {
    body.get(name)
        .map(|value| value.as_object().cloned().ok_or_else(|| missing(name)))
        .transpose()
}

fn components(body: &Map<String, Value>) -> Result<Vec<Value>, Error> {
    body.get("components")
        .map(|value| {
            value
                .as_array()
                .cloned()
                .ok_or_else(|| missing("components"))
        })
        .transpose()
        .map(Option::unwrap_or_default)
}

fn create_surface(body: &Map<String, Value>) -> Result<CreateSurface, Error> {
    let send_data_model = match body.get("sendDataModel") {
        None => false,
        Some(flag) => flag.as_bool().ok_or_else(|| missing("sendDataModel"))?,
    };
    Ok(CreateSurface {
        surface_id: string(body, "surfaceId")?,
        catalog_id: optional_string(body, "catalogId")?,
        send_data_model,
        components: components(body)?,
        data_model: optional_object(body, "dataModel")?,
        metadata: optional_object(body, "metadata")?,
    })
}

fn update_data_model(body: &Map<String, Value>) -> Result<UpdateDataModel, Error> {
    let surface_id = string(body, "surfaceId")?;
    let path = match optional_string(body, "path")? {
        None => Pointer::root(),
        Some(text) => Pointer::parse(&text)
            .map_err(|err| err.with_path("/path".to_owned()).with_surface(&surface_id))?,
    };
    let value = body.get("value").cloned().ok_or_else(|| missing("value"))?;
    Ok(UpdateDataModel {
        surface_id,
        path,
        value,
    })
}

fn call_renderer_function(body: &Map<String, Value>) -> Result<CallRendererFunction, Error> {
    Ok(CallRendererFunction {
        function_call_id: string(body, "functionCallId")?,
        surface_id: optional_string(body, "surfaceId")?,
        call_function: optional_object(body, "callFunction")?
            .ok_or_else(|| missing("callFunction"))?,
    })
}

#[cfg(test)]
#[path = "typed_tests.rs"]
mod tests;
