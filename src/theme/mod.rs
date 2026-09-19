//! Shared theme definitions. Mirrors upstream `theme/index.ts`; the palette and
//! `theme/assets/*.json` set land with the theme slice.

/// Theme identity and (later) palette. Mirrors the upstream `Theme` type.
#[derive(Clone, Debug)]
pub struct Theme {
    pub name: String,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            name: "opencode".to_string(),
        }
    }
}
