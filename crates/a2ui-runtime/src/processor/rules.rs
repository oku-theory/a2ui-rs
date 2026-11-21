//! How strictly a processor checks the components it is sent.

/// Which component-graph faults a processor accepts.
///
/// [`ValidationRules::strict`], the default, accepts none. Orphans, dangling references and a
/// missing root are judged once the graph checks exist; an unknown element is a component type
/// or function its catalog does not define.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ValidationRules {
    allow_orphan_components: bool,
    allow_dangling_references: bool,
    allow_missing_root: bool,
    allow_unknown_elements: bool,
}

impl ValidationRules {
    /// Rules accepting none of the four faults.
    pub fn strict() -> Self {
        Self {
            allow_orphan_components: false,
            allow_dangling_references: false,
            allow_missing_root: false,
            allow_unknown_elements: false,
        }
    }

    /// Rules accepting all four faults.
    pub fn permissive() -> Self {
        Self {
            allow_orphan_components: true,
            allow_dangling_references: true,
            allow_missing_root: true,
            allow_unknown_elements: true,
        }
    }

    /// These rules, accepting components no other component reaches when `allow` holds.
    pub fn allow_orphan_components(self, allow: bool) -> Self {
        Self {
            allow_orphan_components: allow,
            ..self
        }
    }

    /// These rules, accepting references to absent components when `allow` holds.
    pub fn allow_dangling_references(self, allow: bool) -> Self {
        Self {
            allow_dangling_references: allow,
            ..self
        }
    }

    /// These rules, accepting a surface with no `root` component when `allow` holds.
    pub fn allow_missing_root(self, allow: bool) -> Self {
        Self {
            allow_missing_root: allow,
            ..self
        }
    }

    /// These rules, accepting unknown component types and functions when `allow` holds.
    pub fn allow_unknown_elements(self, allow: bool) -> Self {
        Self {
            allow_unknown_elements: allow,
            ..self
        }
    }

    /// Whether components no other component reaches are accepted.
    pub fn allows_orphan_components(&self) -> bool {
        self.allow_orphan_components
    }

    /// Whether references to absent components are accepted.
    pub fn allows_dangling_references(&self) -> bool {
        self.allow_dangling_references
    }

    /// Whether a surface with no `root` component is accepted.
    pub fn allows_missing_root(&self) -> bool {
        self.allow_missing_root
    }

    /// Whether unknown component types and functions are accepted.
    pub fn allows_unknown_elements(&self) -> bool {
        self.allow_unknown_elements
    }
}

impl Default for ValidationRules {
    fn default() -> Self {
        Self::strict()
    }
}

#[cfg(test)]
#[path = "rules_tests.rs"]
mod tests;
