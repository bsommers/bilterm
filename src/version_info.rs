//! Build and runtime version details shown by `bilterm --version`, `:version`, and the status RPC.
//!
//! `VERSION` is the full version including any pre-release suffix (`3.3.1-rc7`); it is set by `build.rs`
//! (`BILTERM_BUILD_VERSION` > `git describe --tags` > Cargo.toml).

/// Full version string, e.g. `3.3.1-rc7` (or `3.3.1-rc7-4-gabc1234-dirty` for a development build).
pub const VERSION: &str = env!("BILTERM_VERSION");
/// Version declared in `Cargo.toml` (no pre-release suffix).
pub const CARGO_VERSION: &str = env!("CARGO_PKG_VERSION");

/// One-line form used by `--version`: `bilterm 3.3.1-rc7`.
pub fn short() -> String {
    format!("bilterm {VERSION}")
}

/// Ordered `(label, value)` rows describing this build and the machine it runs on.
pub fn details() -> Vec<(String, String)> {
    let dirty = env!("BILTERM_GIT_DIRTY") == "true";
    let features = env!("BILTERM_FEATURES");
    let mut rows: Vec<(String, String)> = vec![
        ("VERSION".into(), VERSION.into()),
        ("CARGO VERSION".into(), CARGO_VERSION.into()),
        (
            "COMMIT".into(),
            format!("{}{}", env!("BILTERM_GIT_COMMIT"), if dirty { " (dirty)" } else { "" }),
        ),
        ("COMMIT DATE".into(), env!("BILTERM_COMMIT_DATE").into()),
        ("TARGET".into(), env!("BILTERM_TARGET").into()),
        ("PROFILE".into(), env!("BILTERM_PROFILE").into()),
        ("FEATURES".into(), if features.is_empty() { "(none)".into() } else { features.into() }),
        ("RUSTC".into(), env!("BILTERM_RUSTC").into()),
        (
            "PLATFORM".into(),
            format!("{} / {} ({})", std::env::consts::OS, std::env::consts::ARCH, std::env::consts::FAMILY),
        ),
    ];
    if let Ok(exe) = std::env::current_exe() {
        rows.push(("EXECUTABLE".into(), exe.display().to_string()));
    }
    rows.push(("PID".into(), std::process::id().to_string()));
    for key in ["TERM", "COLORTERM", "TERM_PROGRAM"] {
        if let Ok(v) = std::env::var(key) {
            rows.push((key.into(), v));
        }
    }
    rows
}

/// Plain-text rendering of [`details`] (`bilterm --version --verbose`, RPC, tests).
pub fn details_text() -> String {
    details().iter().map(|(k, v)| format!("{k:<14} {v}\n")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_non_empty_and_starts_with_cargo_version_or_a_tag() {
        assert!(VERSION.starts_with(|c: char| c.is_ascii_digit()));
    }

    #[test]
    fn details_lists_core_rows() {
        let rows = details();
        for label in ["VERSION", "COMMIT", "TARGET", "PLATFORM", "RUSTC"] {
            assert!(rows.iter().any(|(k, _)| k == label), "missing {label}");
        }
        assert!(details_text().contains(VERSION));
    }

    #[test]
    fn short_form_keeps_prerelease_suffix() {
        assert_eq!(short(), format!("bilterm {VERSION}"));
    }
}
