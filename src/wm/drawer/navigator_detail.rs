//! Second-line text of a two-line navigator pane row: a shell-prompt style `user@host  path`.
//!
//! Local panes show the local login and short host name plus the pane's live working directory. Panes
//! attached to a remote destination (`ssh` / `tmux -CC`) show that destination and only the directory the
//! remote shell reported over `OSC 7`, because the local `/proc` cwd of the `ssh` client says nothing
//! about the remote side.

use std::path::Path;
use std::sync::OnceLock;

use crate::pane::CompositePane;
use crate::vt::sanitize_display;

/// Maximum chars of the detail line that reach the layout (same bound as titles, T-50-04).
const DETAIL_MAX_CHARS: usize = 128;

/// `user@host  path` for `pane`, `None` when the pane exposes neither an identity nor a path.
pub(crate) fn pane_detail_line(pane: &CompositePane) -> Option<String> {
    let (identity, path) = match pane.remote_target() {
        Some(target) => {
            let dest = target.split('/').next().unwrap_or(target).trim();
            let identity = (!dest.is_empty()).then(|| dest.to_string());
            (identity, pane.vt().reported_cwd().map(|p| tilde_path(p, None)))
        }
        None => (local_identity(), Some(tilde_path(&pane.resolve_cwd(), home_dir().as_deref()))),
    };
    compose(identity.as_deref(), path.as_deref())
}

/// Joins the identity and path with two spaces; `None` when both are missing or blank.
pub(crate) fn compose(identity: Option<&str>, path: Option<&str>) -> Option<String> {
    let identity = identity.map(str::trim).filter(|s| !s.is_empty());
    let path = path.map(str::trim).filter(|s| !s.is_empty());
    let line = match (identity, path) {
        (Some(i), Some(p)) => format!("{i}  {p}"),
        (Some(i), None) => i.to_string(),
        (None, Some(p)) => p.to_string(),
        (None, None) => return None,
    };
    Some(sanitize_display(&line, DETAIL_MAX_CHARS))
}

/// `path` with a leading `home` replaced by `~` (`home` of `None` leaves it untouched).
pub(crate) fn tilde_path(path: &Path, home: Option<&Path>) -> String {
    if let Some(home) = home.filter(|h| h.as_os_str().len() > 1)
        && let Ok(rest) = path.strip_prefix(home)
    {
        return if rest.as_os_str().is_empty() { "~".to_string() } else { format!("~/{}", rest.display()) };
    }
    path.display().to_string()
}

fn home_dir() -> Option<std::path::PathBuf> {
    std::env::var_os("HOME").map(std::path::PathBuf::from)
}

/// Cached `user@host` of this machine (host trimmed to its first label, like a shell `\h`).
fn local_identity() -> Option<String> {
    static IDENTITY: OnceLock<Option<String>> = OnceLock::new();
    IDENTITY
        .get_or_init(|| {
            let user = std::env::var("USER").or_else(|_| std::env::var("LOGNAME")).ok().filter(|u| !u.is_empty());
            let host = std::env::var("HOSTNAME")
                .ok()
                .or_else(|| std::fs::read_to_string("/proc/sys/kernel/hostname").ok())
                .or_else(|| std::fs::read_to_string("/etc/hostname").ok())
                .map(|h| h.trim().split('.').next().unwrap_or("").to_string())
                .filter(|h| !h.is_empty());
            match (user, host) {
                (Some(u), Some(h)) => Some(format!("{u}@{h}")),
                (Some(u), None) => Some(u),
                (None, Some(h)) => Some(h),
                (None, None) => None,
            }
        })
        .clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn compose_joins_identity_and_path_with_two_spaces() {
        assert_eq!(compose(Some("bill@box"), Some("~/src")).as_deref(), Some("bill@box  ~/src"));
        assert_eq!(compose(Some("bill@box"), None).as_deref(), Some("bill@box"));
        assert_eq!(compose(None, Some("/tmp")).as_deref(), Some("/tmp"));
        assert_eq!(compose(Some("  "), Some("")), None);
        assert_eq!(compose(None, None), None);
    }

    #[test]
    fn compose_strips_control_characters() {
        let line = compose(Some("a@b"), Some("/x\u{1b}[31my")).unwrap();
        assert!(!line.contains('\u{1b}'));
    }

    #[test]
    fn tilde_path_abbreviates_home_only() {
        let home = PathBuf::from("/home/bill");
        assert_eq!(tilde_path(Path::new("/home/bill"), Some(&home)), "~");
        assert_eq!(tilde_path(Path::new("/home/bill/src/x"), Some(&home)), "~/src/x");
        assert_eq!(tilde_path(Path::new("/home/billy/x"), Some(&home)), "/home/billy/x");
        assert_eq!(tilde_path(Path::new("/etc"), None), "/etc");
        assert_eq!(tilde_path(Path::new("/etc"), Some(Path::new("/"))), "/etc");
    }
}
