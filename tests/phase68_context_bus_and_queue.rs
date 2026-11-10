use bilterm::wm::context_bus::*;
use bilterm::wm::prompt_queue::*;
use bilterm::wm::WindowManager;

// ---------- ContextBus ----------

#[test]
fn bus_set_get_overwrite() {
    let b = ContextBus::new();
    assert!(b.get_at("k", 0).is_none());
    let r1 = b.set_at("k", "v1", Some(3), None, 10).unwrap();
    let e = b.get_at("k", 10).unwrap();
    assert_eq!((e.value.as_str(), e.sender, e.revision, e.set_at), ("v1", Some(3), r1, 10));
    let r2 = b.set_at("k", "v2", Some(4), None, 20).unwrap();
    assert!(r2 > r1);
    let e = b.get_at("k", 20).unwrap();
    assert_eq!((e.value.as_str(), e.sender, e.revision), ("v2", Some(4), r2));
    assert_eq!(b.list_at(None, 20).len(), 1);
}

#[test]
fn bus_delete_and_tombstone() {
    let b = ContextBus::new();
    let r1 = b.set_at("a", "1", None, None, 0).unwrap();
    assert!(b.delete_at("a", Some(9), 1));
    assert!(!b.delete_at("a", None, 2));
    assert!(b.get_at("a", 2).is_none());
    let ch = b.watch_since_at(0, 2);
    assert_eq!(ch.len(), 2);
    assert_eq!(ch[0].value.as_deref(), Some("1"));
    assert_eq!(ch[1].value, None);
    assert_eq!(ch[1].sender, Some(9));
    assert!(ch[1].revision > r1);
    assert_eq!(b.revision(), ch[1].revision);
}

#[test]
fn bus_list_prefix_sorted() {
    let b = ContextBus::new();
    for k in ["b/2", "a/1", "b/1", "c"] {
        b.set_at(k, "x", None, None, 0).unwrap();
    }
    let all: Vec<_> = b.list_at(None, 0).into_iter().map(|e| e.key).collect();
    assert_eq!(all, ["a/1", "b/1", "b/2", "c"]);
    let bs: Vec<_> = b.list_at(Some("b/"), 0).into_iter().map(|e| e.key).collect();
    assert_eq!(bs, ["b/1", "b/2"]);
    assert!(b.list_at(Some("zz"), 0).is_empty());
}

#[test]
fn bus_watch_since_ordering() {
    let b = ContextBus::new();
    let r1 = b.set_at("a", "1", None, None, 0).unwrap();
    let r2 = b.set_at("b", "2", Some(1), None, 0).unwrap();
    let r3 = b.set_at("a", "3", None, None, 0).unwrap();
    assert!(r1 < r2 && r2 < r3);
    let ch = b.watch_since_at(r1, 0);
    assert_eq!(ch.iter().map(|c| c.revision).collect::<Vec<_>>(), [r2, r3]);
    assert!(b.watch_since_at(r3, 0).is_empty());
    assert_eq!(b.watch_since_at(0, 0).len(), 3);
}

#[test]
fn bus_ttl_default_and_per_entry() {
    let b = ContextBus::with_default_ttl(100);
    b.set_at("d", "v", None, None, 0).unwrap();
    b.set_at("short", "v", None, Some(10), 0).unwrap();
    b.set_at("long", "v", None, Some(1000), 0).unwrap();
    assert!(b.get_at("short", 9).is_some());
    assert!(b.get_at("short", 10).is_none());
    assert!(b.get_at("d", 99).is_some());
    assert!(b.get_at("d", 100).is_none());
    assert!(b.get_at("long", 500).is_some());
    // expiry is observable as tombstones
    let tomb: Vec<_> = b.watch_since_at(0, 100).into_iter().filter(|c| c.value.is_none()).collect();
    assert_eq!(tomb.len(), 2);
}

#[test]
fn bus_no_ttl_never_expires() {
    let b = ContextBus::new();
    b.set_at("k", "v", None, None, 0).unwrap();
    assert!(b.get_at("k", u64::MAX).is_some());
}

#[test]
fn bus_limits() {
    let b = ContextBus::new();
    let big = "x".repeat(MAX_VALUE_BYTES + 1);
    assert!(matches!(b.set_at("k", &big, None, None, 0), Err(ContextBusError::ValueTooLarge { .. })));
    let ok = "x".repeat(MAX_VALUE_BYTES);
    assert!(b.set_at("k", &ok, None, None, 0).is_ok());
    for i in 1..MAX_ENTRIES {
        b.set_at(&format!("k{i}"), "v", None, None, 0).unwrap();
    }
    assert_eq!(b.list_at(None, 0).len(), MAX_ENTRIES);
    assert_eq!(
        b.set_at("overflow", "v", None, None, 0),
        Err(ContextBusError::TooManyEntries { max: MAX_ENTRIES })
    );
    // overwrite at capacity is fine; deleting frees a slot
    assert!(b.set_at("k1", "new", None, None, 0).is_ok());
    assert!(b.delete_at("k2", None, 0));
    assert!(b.set_at("overflow", "v", None, None, 0).is_ok());
}

#[test]
fn bus_full_frees_slot_by_expiry() {
    let b = ContextBus::new();
    b.set_at("ttl", "v", None, Some(5), 0).unwrap();
    for i in 1..MAX_ENTRIES {
        b.set_at(&format!("k{i}"), "v", None, None, 0).unwrap();
    }
    assert!(b.set_at("new", "v", None, None, 0).is_err());
    assert!(b.set_at("new", "v", None, None, 5).is_ok());
}

#[test]
fn bus_invalid_keys() {
    let b = ContextBus::new();
    assert_eq!(b.set_at("", "v", None, None, 0), Err(ContextBusError::EmptyKey));
    assert!(matches!(
        b.set_at(&"k".repeat(MAX_KEY_BYTES + 1), "v", None, None, 0),
        Err(ContextBusError::KeyTooLong { .. })
    ));
    assert_eq!(b.set_at("a\nb", "v", None, None, 0), Err(ContextBusError::InvalidKeyChar));
    assert_eq!(b.set_at("a\u{1b}b", "v", None, None, 0), Err(ContextBusError::InvalidKeyChar));
    assert!(b.set_at(&"k".repeat(MAX_KEY_BYTES), "v", None, None, 0).is_ok());
    assert!(b.set_at("with space/ünï", "v", None, None, 0).is_ok());
    assert_eq!(b.revision(), 2, "rejected writes do not bump revision");
}

#[test]
fn bus_log_is_bounded() {
    let b = ContextBus::new();
    for i in 0..(MAX_LOG + 10) {
        b.set_at("k", &i.to_string(), None, None, 0).unwrap();
    }
    let all = b.watch_since_at(0, 0);
    assert_eq!(all.len(), MAX_LOG);
    assert_eq!(all.last().unwrap().revision, (MAX_LOG + 10) as u64);
}

#[test]
fn bus_clone_shares_state_and_threads() {
    let b = ContextBus::new();
    let mut hs = vec![];
    for t in 0..8usize {
        let b = b.clone();
        hs.push(std::thread::spawn(move || {
            (0..50)
                .map(|i| b.set(&format!("t{t}/{i}"), "v", Some(t)).unwrap())
                .collect::<Vec<u64>>()
        }));
    }
    let mut revs: Vec<u64> = hs.into_iter().flat_map(|h| h.join().unwrap()).collect();
    revs.sort_unstable();
    revs.dedup();
    assert_eq!(revs.len(), 400, "revisions are unique");
    assert_eq!(b.list(None).len(), 400);
    assert_eq!(b.revision(), 400);
    assert_eq!(b.get("t3/7").unwrap().sender, Some(3));
}

// ---------- PromptQueue ----------

#[test]
fn queue_fifo_peek_len() {
    let mut q = PromptQueue::new();
    assert!(q.is_empty(1));
    assert_eq!(q.pop_front(1), None);
    q.enqueue(1, "a").unwrap();
    q.enqueue(1, "b").unwrap();
    assert_eq!(q.len(1), 2);
    assert_eq!(q.peek(1), Some("a"));
    assert_eq!(q.pop_front(1).as_deref(), Some("a"));
    assert_eq!(q.pop_front(1).as_deref(), Some("b"));
    assert_eq!(q.pop_front(1), None);
}

#[test]
fn queue_reorder_remove() {
    let mut q = PromptQueue::new();
    for p in ["a", "b", "c", "d"] {
        q.enqueue(1, p).unwrap();
    }
    q.reorder(1, 3, 0).unwrap();
    assert_eq!(q.items(1), ["d", "a", "b", "c"]);
    q.reorder(1, 0, 2).unwrap();
    assert_eq!(q.items(1), ["a", "b", "d", "c"]);
    q.reorder(1, 1, 1).unwrap();
    assert_eq!(q.items(1), ["a", "b", "d", "c"]);
    assert_eq!(q.reorder(1, 4, 0), Err(QueueError::IndexOutOfRange { index: 4, len: 4 }));
    assert_eq!(q.reorder(1, 0, 9), Err(QueueError::IndexOutOfRange { index: 9, len: 4 }));
    assert_eq!(q.remove(1, 2).as_deref(), Some("d"));
    assert_eq!(q.remove(1, 9), None);
    assert_eq!(q.items(1), ["a", "b", "c"]);
}

#[test]
fn queue_pause_resume_on_idle() {
    let mut q = PromptQueue::new();
    q.enqueue(1, "a").unwrap();
    q.enqueue(1, "b").unwrap();
    assert_eq!(q.on_idle(1).as_deref(), Some("a"));
    q.pause(1);
    assert!(q.is_paused(1));
    assert_eq!(q.on_idle(1), None);
    assert_eq!(q.len(1), 1);
    assert_eq!(q.peek(1), Some("b"));
    q.resume(1);
    assert!(!q.is_paused(1));
    assert_eq!(q.on_idle(1).as_deref(), Some("b"));
    assert_eq!(q.on_idle(1), None);
    assert_eq!(q.on_idle(42), None);
}

#[test]
fn queue_bounds_and_isolation() {
    let mut q = PromptQueue::new();
    for i in 0..MAX_QUEUE_LEN {
        q.enqueue(1, format!("p{i}")).unwrap();
    }
    assert_eq!(q.enqueue(1, "x"), Err(QueueError::Full { max: MAX_QUEUE_LEN }));
    q.enqueue(2, "other").unwrap();
    q.pause(1);
    assert_eq!(q.on_idle(2).as_deref(), Some("other"));
    assert_eq!(q.len(1), MAX_QUEUE_LEN);
    q.remove_pane(1);
    assert_eq!(q.len(1), 0);
    assert!(!q.is_paused(1));
}

// ---------- BlockedReason ----------

fn c(lines: &[&str]) -> Option<BlockedReason> {
    classify_blocked(lines)
}

#[test]
fn classify_positive_each_kind() {
    assert_eq!(c(&["Overwrite file? [y/N]"]), Some(BlockedReason::YesNoConfirm));
    assert_eq!(c(&["Proceed (y/n)"]), Some(BlockedReason::YesNoConfirm));
    assert_eq!(c(&["Continue?"]), Some(BlockedReason::YesNoConfirm));
    assert_eq!(c(&["Do you want to proceed?"]), Some(BlockedReason::ToolApproval));
    assert_eq!(c(&["Allow tool Bash to run?"]), Some(BlockedReason::ToolApproval));
    assert_eq!(c(&["Please approve this action"]), Some(BlockedReason::ToolApproval));
    assert_eq!(c(&["Do you want to make this edit to foo.rs?"]), Some(BlockedReason::FileWritePermission));
    assert_eq!(c(&["About to write to file x.txt"]), Some(BlockedReason::FileWritePermission));
    assert_eq!(c(&["Create file src/new.rs?"]), Some(BlockedReason::FileWritePermission));
    assert_eq!(c(&["Allow MCP server github to run?"]), Some(BlockedReason::McpPermission));
    assert_eq!(c(&["MCP permission required"]), Some(BlockedReason::McpPermission));
    assert_eq!(c(&["[sudo] password for bill:"]), Some(BlockedReason::SudoPassword));
    assert_eq!(c(&["Password:"]), Some(BlockedReason::SudoPassword));
}

#[test]
fn classify_case_insensitive() {
    assert_eq!(c(&["CONTINUE?"]), Some(BlockedReason::YesNoConfirm));
    assert_eq!(c(&["[Y/N]"]), Some(BlockedReason::YesNoConfirm));
    assert_eq!(c(&["do YOU want TO proceed"]), Some(BlockedReason::ToolApproval));
}

#[test]
fn classify_negatives() {
    assert_eq!(c(&[]), None);
    assert_eq!(c(&["", "   "]), None);
    assert_eq!(c(&["compiling foo", "Finished dev", "$ "]), None);
    assert_eq!(c(&["create file without question"]), None);
    assert_eq!(c(&["mcp server started"]), None);
}

#[test]
fn classify_old_prompt_scrolled_out_of_tail() {
    let mut lines = vec!["Do you want to proceed?"];
    lines.extend(["out1", "out2", "out3", "out4", "out5"]);
    assert_eq!(c(&lines), None);
    // blank lines do not count toward the tail window
    let mut lines2 = vec!["Do you want to proceed?"];
    lines2.extend(["out1", "", "out2", "", "out3", "out4"]);
    assert_eq!(c(&lines2), Some(BlockedReason::ToolApproval));
}

#[test]
fn classify_ansi_wrapped() {
    assert_eq!(c(&["\x1b[1;33mContinue? [y/N]\x1b[0m"]), Some(BlockedReason::YesNoConfirm));
    assert_eq!(c(&["\x1b[31m[sudo] password for\x1b[0m bill: "]), Some(BlockedReason::SudoPassword));
    assert_eq!(c(&["\x1b]0;title\x07Do you want to proceed?\r"]), Some(BlockedReason::ToolApproval));
    assert_eq!(c(&["Do \x1b[1myou want to proceed\x1b[0m?"]), Some(BlockedReason::ToolApproval));
    assert_eq!(strip_ansi("\x1b[2Kab\x1b]8;;http://x\x1b\\c"), "abc");
}

#[test]
fn classify_precedence() {
    let all = ["Continue? [y/N]", "Do you want to proceed?", "write to file x", "Allow MCP tool?", "[sudo] password for u:"];
    assert_eq!(c(&all), Some(BlockedReason::SudoPassword));
    assert_eq!(c(&all[..4]), Some(BlockedReason::McpPermission));
    assert_eq!(c(&all[..3]), Some(BlockedReason::FileWritePermission));
    assert_eq!(c(&all[..2]), Some(BlockedReason::ToolApproval));
    assert_eq!(c(&all[..1]), Some(BlockedReason::YesNoConfirm));
}

#[test]
fn payloads() {
    use BlockedReason::*;
    assert_eq!(approve_payload(&YesNoConfirm).unwrap(), b"y\r");
    assert_eq!(deny_payload(&YesNoConfirm).unwrap(), b"n\r");
    for r in [ToolApproval, FileWritePermission, McpPermission] {
        assert_eq!(approve_payload(&r).unwrap(), b"1\r");
        assert_eq!(deny_payload(&r).unwrap(), vec![0x1b]);
    }
    assert_eq!(approve_payload(&SudoPassword), None, "never synthesize passwords");
    assert_eq!(deny_payload(&SudoPassword), Some(vec![0x03]));
}

// ---------- WindowManager accessors ----------

#[test]
fn window_manager_accessors() {
    let mut wm = WindowManager::new_headless(16, 80);
    let handle = wm.context_bus().clone();
    wm.context_bus_mut().set("plan", "v1", Some(1)).unwrap();
    assert_eq!(handle.get("plan").unwrap().value, "v1");
    wm.prompt_queue_mut().enqueue(1, "hello").unwrap();
    assert_eq!(wm.prompt_queue().peek(1), Some("hello"));
    wm.prompt_queue_mut().pause(1);
    assert_eq!(wm.prompt_queue_mut().on_idle(1), None);
    wm.prompt_queue_mut().resume(1);
    assert_eq!(wm.prompt_queue_mut().on_idle(1).as_deref(), Some("hello"));
}
