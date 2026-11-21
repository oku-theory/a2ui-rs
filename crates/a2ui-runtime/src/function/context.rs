//! What a function implementation sees of the call it runs in.

/// The call an implementation runs in: its surface, catalog and user activation.
#[derive(Debug)]
pub struct CallContext<'a> {
    user_activation: bool,
    surface_id: &'a str,
    catalog_id: &'a str,
}

impl<'a> CallContext<'a> {
    /// A context for a call in `catalog_id` on `surface_id`.
    #[cfg_attr(not(test), expect(dead_code, reason = "used by Task 2 (t-1712)"))]
    pub(crate) fn new(user_activation: bool, surface_id: &'a str, catalog_id: &'a str) -> Self {
        Self {
            user_activation,
            surface_id,
            catalog_id,
        }
    }

    /// Whether the call runs inside a user activation, such as a click.
    pub fn user_activation(&self) -> bool {
        self.user_activation
    }

    /// The id of the surface the call evaluates on.
    pub fn surface_id(&self) -> &str {
        self.surface_id
    }

    /// The id of the catalog the call resolved in.
    pub fn catalog_id(&self) -> &str {
        self.catalog_id
    }
}
