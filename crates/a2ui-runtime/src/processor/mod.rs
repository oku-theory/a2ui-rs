//! Applies agent messages to the surfaces a renderer holds.
//!
//! [`MessageProcessor::handle`] checks a message against the surfaces, catalogs and
//! [`StateLimits`], then applies it; a refused message changes nothing. The host turns an error
//! into a reply with [`RendererMessage::error`](crate::RendererMessage::error).

pub(crate) mod children;
mod components;
mod composition;
mod limits;
mod reports;
mod resolve;
mod rules;
#[cfg(test)]
pub(crate) mod test_support;
mod topology;

use std::collections::BTreeMap;

use serde_json::Value;

use crate::catalog::CatalogSet;
use crate::error::{Error, ProcessFailure};
use crate::message::{
    AgentMessage, CreateSurface, MessageLimits, UpdateComponents, UpdateDataModel,
};
use crate::surface::Surface;

pub use limits::StateLimits;
pub use rules::ValidationRules;

/// What handling a message produced; later versions add fields.
// MVP: t-1688 (outbound messages), t-1689 (ChangeSet)
#[non_exhaustive]
#[derive(Debug, Default, PartialEq)]
pub struct Outcome {}

/// Keeps a renderer's surfaces and applies the agent's messages to them.
///
/// It owns the [`CatalogSet`] the surfaces' components are checked against. It does no I/O:
/// the host reads messages from its transport and hands them over.
#[derive(Debug)]
pub struct MessageProcessor {
    catalogs: CatalogSet,
    surfaces: BTreeMap<String, Surface>,
    limits: MessageLimits,
    state_limits: StateLimits,
    rules: ValidationRules,
}

impl MessageProcessor {
    /// A processor with no surface, checking against `catalogs` under default limits.
    pub fn new(catalogs: CatalogSet) -> Self {
        Self {
            catalogs,
            surfaces: BTreeMap::new(),
            limits: MessageLimits::default(),
            state_limits: StateLimits::default(),
            rules: ValidationRules::default(),
        }
    }

    /// This processor reading batch messages under `limits`.
    pub fn with_limits(self, limits: MessageLimits) -> Self {
        Self { limits, ..self }
    }

    /// This processor keeping its state within `limits`.
    pub fn with_state_limits(self, limits: StateLimits) -> Self {
        Self {
            state_limits: limits,
            ..self
        }
    }

    /// This processor checking components under `rules` unless a call names others.
    pub fn with_rules(self, rules: ValidationRules) -> Self {
        Self { rules, ..self }
    }

    /// Applies one message under this processor's [`ValidationRules`].
    ///
    /// # Errors
    ///
    /// The first check the message fails; a refused message changes nothing. An error from a
    /// message naming a surface carries its [`Error::surface_id`].
    pub fn handle(&mut self, message: &AgentMessage) -> Result<Outcome, Error> {
        self.handle_with(message, self.rules)
    }

    /// Applies one message, checking its components under `rules`.
    ///
    /// # Errors
    ///
    /// As [`MessageProcessor::handle`].
    pub fn handle_with(
        &mut self,
        message: &AgentMessage,
        rules: ValidationRules,
    ) -> Result<Outcome, Error> {
        match message {
            AgentMessage::CreateSurface(create) => self
                .create_surface(create, rules)
                .map_err(|error| error.with_surface(&create.surface_id)),
            AgentMessage::UpdateComponents(update) => self
                .update_components(update, rules)
                .map_err(|error| error.with_surface(&update.surface_id)),
            AgentMessage::UpdateDataModel(update) => self
                .update_data_model(update)
                .map_err(|error| error.with_surface(&update.surface_id)),
            AgentMessage::DeleteSurface(delete) => {
                self.surfaces.remove(&delete.surface_id);
                Ok(())
            }
            // MVP: t-1688
            AgentMessage::CallRendererFunction(call) => {
                Err(Error::from(ProcessFailure::NotYetSupported {
                    action: "callRendererFunction",
                })
                .with_function_call(&call.function_call_id))
            }
            // MVP: t-1688
            AgentMessage::AgentFunctionResponse(_) => Err(ProcessFailure::NotYetSupported {
                action: "agentFunctionResponse",
            }
            .into()),
        }?;
        Ok(Outcome::default())
    }

    /// Reads and applies a batch in order, stopping at the first error.
    ///
    /// A batch is an array of messages or an object whose only key is `messages`. Each message
    /// is read only once the previous one is applied.
    ///
    /// # Errors
    ///
    /// A validation error when `batch` has neither shape; else the first message's error, with
    /// its [`Error::message_index`]. Earlier messages stay applied; later ones are never read.
    pub fn handle_batch(&mut self, batch: &Value) -> Result<Outcome, Error> {
        self.handle_batch_with(batch, self.rules)
    }

    /// Reads and applies a batch as [`MessageProcessor::handle_batch`], under `rules`.
    ///
    /// # Errors
    ///
    /// As [`MessageProcessor::handle_batch`].
    pub fn handle_batch_with(
        &mut self,
        batch: &Value,
        rules: ValidationRules,
    ) -> Result<Outcome, Error> {
        let limits = self.limits;
        for (index, message) in AgentMessage::batch(batch, &limits)?.enumerate() {
            self.handle_with(&message?, rules)
                .map_err(|error| error.at_message(index))?;
        }
        Ok(Outcome::default())
    }

    /// The surface whose id is `id`, if the processor holds it.
    pub fn surface(&self, id: &str) -> Option<&Surface> {
        self.surfaces.get(id)
    }

    /// The catalogs surfaces are checked against.
    pub fn catalogs(&self) -> &CatalogSet {
        &self.catalogs
    }

    /// Checks a `createSurface` (duplicate, surface count, catalog, components, data model size),
    /// then inserts the surface.
    fn create_surface(
        &mut self,
        create: &CreateSurface,
        rules: ValidationRules,
    ) -> Result<(), Error> {
        let id = &create.surface_id;
        if self.surfaces.contains_key(id) {
            return Err(ProcessFailure::SurfaceExists { id: id.into() }.into());
        }
        let max_surfaces = self.state_limits.max_surfaces();
        if self.surfaces.len() >= max_surfaces {
            return Err(state_limit("Surface", max_surfaces));
        }
        if let Some(catalog) = &create.catalog_id
            && self.catalogs.get(catalog).is_none()
        {
            return Err(ProcessFailure::CatalogNotFound { id: catalog.into() }.into());
        }
        let mut surface = Surface::new(create);
        let staged = components::check(
            &surface,
            &create.components,
            rules,
            &self.catalogs,
            &self.state_limits,
        )?;
        let view = surface.merged(&staged);
        topology::check(&view, &self.catalogs, rules)?;
        composition::check(&view, &staged, &self.catalogs)?;
        surface.commit(staged);
        let max_nodes = self.state_limits.max_data_model_nodes();
        if surface.data_model().weighs_more_than(max_nodes) {
            return Err(state_limit("Data model node", max_nodes));
        }
        self.surfaces.insert(id.clone(), surface);
        Ok(())
    }

    /// Checks every component of `update`, then stores them all.
    fn update_components(
        &mut self,
        update: &UpdateComponents,
        rules: ValidationRules,
    ) -> Result<(), Error> {
        let surface = self.surfaces.get_mut(&update.surface_id).ok_or_else(|| {
            ProcessFailure::SurfaceNotFound {
                id: (&update.surface_id).into(),
            }
        })?;
        let components = &update.components;
        let staged = components::check(
            surface,
            components,
            rules,
            &self.catalogs,
            &self.state_limits,
        )?;
        let view = surface.merged(&staged);
        topology::check(&view, &self.catalogs, rules)?;
        composition::check(&view, &staged, &self.catalogs)?;
        surface.commit(staged);
        Ok(())
    }

    /// Writes the message's value; a scalar or array root is refused, and a write past the node limit is undone and refused.
    fn update_data_model(&mut self, update: &UpdateDataModel) -> Result<(), Error> {
        let max_nodes = self.state_limits.max_data_model_nodes();
        let model = self
            .surfaces
            .get_mut(&update.surface_id)
            .ok_or_else(|| ProcessFailure::SurfaceNotFound {
                id: (&update.surface_id).into(),
            })?
            .data_model_mut();
        if update.path.is_root() && !update.value.is_null() && !update.value.is_object() {
            return Err(ProcessFailure::NonObjectRoot.into());
        }
        if update.value.is_null() {
            // A deletion never adds a node.
            model.set(&update.path, Value::Null)?;
            return Ok(());
        }
        if let Some(undo) = model.set_undoable(&update.path, update.value.clone())?
            && model.weighs_more_than(max_nodes)
        {
            model.undo(undo)?;
            return Err(state_limit("Data model node", max_nodes));
        }
        Ok(())
    }
}

fn state_limit(what: &'static str, limit: usize) -> Error {
    ProcessFailure::StateLimit { what, limit }.into()
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "mod_components_tests.rs"]
mod components_tests;

#[cfg(test)]
#[path = "mod_rollback_tests.rs"]
mod rollback_tests;
