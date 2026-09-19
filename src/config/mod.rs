//! TUI configuration. Mirrors the subset of upstream `config/index.tsx` that M1
//! needs: reading `~/.config/opencode/tui.json` and resolving its keybind
//! overrides. Theme selection and the rest of the schema arrive with the theme
//! slice.

pub mod keybind;

use std::path::PathBuf;

use keybind::Keybinds;

/// Path to the user's TUI config: `$OPENCODE_CONFIG_DIR/tui.json` when set,
/// else `~/.config/opencode/tui.json`, matching upstream's config location.
pub fn config_path() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("OPENCODE_CONFIG_DIR") {
        return Some(PathBuf::from(dir).join("tui.json"));
    }
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join(".config/opencode/tui.json"))
}

/// Load and resolve the user's keybind overrides. A missing or malformed file
/// falls back to the built-in defaults rather than failing startup.
pub fn load_keybinds() -> Keybinds {
    let path = match config_path() {
        Some(path) => path,
        None => return Keybinds::resolve(&serde_json::Map::new()),
    };
    let overrides = match std::fs::read_to_string(&path) {
        Ok(text) => match serde_json::from_str::<serde_json::Value>(&text) {
            Ok(value) => value
                .get("keybinds")
                .and_then(|value| value.as_object())
                .cloned()
                .unwrap_or_default(),
            Err(err) => {
                tracing::warn!(%err, path = %path.display(), "ignoring malformed tui.json");
                serde_json::Map::new()
            }
        },
        Err(_) => serde_json::Map::new(),
    };
    let keybinds = Keybinds::resolve(&overrides);
    for name in &keybinds.unknown {
        tracing::warn!(%name, "ignoring unknown or malformed keybind in tui.json");
    }
    keybinds
}
