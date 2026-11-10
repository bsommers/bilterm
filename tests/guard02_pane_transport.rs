//! GUARD-02: `PaneTransport` makes Local / Remote / None mutually exclusive (T-TRN-01).

use bilterm::pane::{CompositePane, LinkBanner, PaneTransport, RemotePaneBridge};
use bilterm::pty::PtyConfig;

fn headless() -> CompositePane {
    CompositePane::new_headless(1, 25, 80)
}

fn banner(text: &str) -> LinkBanner {
    LinkBanner { text: text.into(), interactive: false }
}

#[test]
fn headless_defaults_to_none() {
    let mut p = headless();
    assert!(matches!(p.transport(), PaneTransport::None));
    assert!(p.is_headless());
    assert!(!p.has_pty());
    assert!(!p.remote_input_tap());
    assert!(p.link_banner().is_none());
    p.write_to_pty(b"dropped").unwrap();
    assert!(p.take_remote_input().is_empty(), "None writes nothing anywhere");
}

#[test]
fn link_then_disconnect_transitions() {
    let mut p = headless();
    p.set_remote_input_tap(true);
    assert!(matches!(p.transport(), PaneTransport::Remote(_)));
    assert!(p.remote_input_tap());
    assert!(p.is_headless(), "remote panes own no local PTY");
    p.write_to_pty(b"ls\r").unwrap();
    // Re-linking keeps the buffered bytes.
    p.set_remote_input_tap(true);
    assert_eq!(p.take_remote_input(), b"ls\r");

    p.write_to_pty(b"pending").unwrap();
    p.set_remote_input_tap(false);
    assert!(matches!(p.transport(), PaneTransport::None));
    assert!(!p.remote_input_tap());
    assert!(p.take_remote_input().is_empty(), "disconnect drops buffered bytes");
    p.write_to_pty(b"after").unwrap();
    assert!(p.take_remote_input().is_empty());
}

#[test]
fn remote_buffer_is_capped() {
    let mut p = headless();
    p.set_remote_input_tap(true);
    // The cap is sized for a large paste arriving as one event (it was 4096, which silently
    // truncated pastes); beyond it bytes are still dropped rather than buffered without bound.
    let cap = bilterm::tmux::link::REMOTE_INPUT_CAP;
    p.write_to_pty(&vec![b'a'; cap + 10_000]).unwrap();
    assert_eq!(p.take_remote_input().len(), cap);
}

#[test]
fn tap_only_in_remote_and_local_is_replaced_by_link() {
    let mut p = CompositePane::spawn(2, PtyConfig::default()).unwrap();
    assert!(matches!(p.transport(), PaneTransport::Local(_)));
    assert!(p.has_pty() && !p.is_headless() && !p.remote_input_tap());
    assert!(p.take_remote_input().is_empty(), "no tap while Local");
    p.write_to_pty(b"\n").unwrap();
    assert!(p.take_remote_input().is_empty(), "Local bytes go to the PTY, not the tap");

    p.set_remote_input_tap(false); // no-op while Local
    assert!(p.transport().is_local());

    p.set_remote_input_tap(true); // local -> remote: PTY is released, never both
    assert!(p.transport().is_remote());
    assert!(!p.has_pty() && p.pty().is_none());
    p.write_to_pty(b"x").unwrap();
    assert_eq!(p.take_remote_input(), b"x");
}

#[test]
fn transport_byte_routing_per_variant() {
    let mut none = PaneTransport::None;
    none.write_bytes(b"x").unwrap();
    assert!(none.remote_bridge_mut().is_none());

    let mut remote = PaneTransport::Remote(RemotePaneBridge::new());
    remote.write_bytes(b"abc").unwrap();
    assert!(remote.local_pty().is_none());
    assert_eq!(remote.remote_bridge().unwrap().pending_len(), 3);
    assert_eq!(remote.remote_bridge_mut().unwrap().take_input(), b"abc");
    assert!(remote.is_remote() && !remote.is_local() && !remote.is_none());
    assert!(!remote.resize(10, 10).unwrap());
}

#[test]
fn link_banner_is_independent_of_transport_and_sanitized() {
    // Orphaned/held panes (transport None) keep their banner; it survives link transitions.
    let mut p = headless();
    p.set_link_banner(Some(banner("orphaned")));
    assert_eq!(p.link_banner().unwrap().text, "orphaned");
    p.set_remote_input_tap(true);
    assert_eq!(p.link_banner().unwrap().text, "orphaned");
    p.set_remote_input_tap(false);
    assert!(p.link_banner().is_some());
    p.set_link_banner(None);
    assert!(p.link_banner().is_none());
    p.set_link_banner(Some(banner("a\x1b[31mb")));
    assert!(!p.link_banner().unwrap().text.contains('\x1b'));
}
