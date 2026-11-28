//! Self-contained, zero-CDN interactive Shared Artifact Studio HTML/CSS/JS web app (`EDIT-07`).
//!
//! Designed specifically to work cleanly inside headless Chrome CDP screencasts (zero native
//! `<select>` OS popups, explicit click targets, explicit Save/Cancel on inline block editors)
//! as well as standard desktop browsers.

/// Serializes a Rust string as a JS string literal safe to embed inside an HTML `<script>` block.
fn js_string_literal(s: &str) -> String {
    let json = serde_json::to_string(s).unwrap_or_else(|_| "\"\"".to_string());
    json.replace('<', "\\u003c")
        .replace('>', "\\u003e")
        .replace('&', "\\u0026")
        .replace('\u{2028}', "\\u2028")
        .replace('\u{2029}', "\\u2029")
}

/// Returns the interactive Shared Artifact Studio HTML page bound to `base_path` (e.g. `/<token>`).
pub fn studio_app_html(base_path: &str, initial_artifact: &str) -> String {
    let base_js = js_string_literal(base_path);
    let init_js = js_string_literal(initial_artifact);
    let head = r###"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>Bilterm Artifact Studio</title>
<style>
  :root {
    --bg: #0b0f17;
    --panel: #111827;
    --panel-2: #161f30;
    --border: #243047;
    --text: #e6edf7;
    --muted: #8b9bb4;
    --accent: #38bdf8;
    --accent-2: #22d3ee;
    --green: #34d399;
    --amber: #fbbf24;
    --red: #f87171;
  }
  * { box-sizing: border-box; }
  html, body { height: 100%; margin: 0; background: var(--bg); color: var(--text); font: 14px/1.55 -apple-system, BlinkMacSystemFont, "SF Pro Text", "Segoe UI", sans-serif; }
  .shell { display: grid; grid-template-columns: 190px 1fr; height: 100vh; overflow: hidden; }
  .mobile-docs { display: none; padding: 6px 12px; gap: 6px; background: var(--panel); border-bottom: 1px solid var(--border); overflow-x: auto; align-items: center; }
  @media (max-width: 520px) {
    .shell { grid-template-columns: 1fr; }
    .side { display: none; }
    .mobile-docs { display: flex; }
  }
  .side { background: var(--panel); border-right: 1px solid var(--border); display: flex; flex-direction: column; padding: 12px 10px; gap: 10px; overflow: hidden; }
  .brand { display: flex; align-items: center; justify-content: space-between; font-weight: 700; font-size: 12px; letter-spacing: .05em; color: var(--accent-2); text-transform: uppercase; padding: 2px 4px; }
  .live-pill { font-size: 10px; color: var(--green); background: rgba(52,211,153,.12); border: 1px solid rgba(52,211,153,.3); border-radius: 999px; padding: 1px 7px; font-weight: 600; }
  .doc-list { flex: 1; overflow-y: auto; display: flex; flex-direction: column; gap: 4px; }
  .doc-row { display: flex; align-items: center; gap: 3px; }
  .doc-item { flex: 1; min-width: 0; display: flex; align-items: center; justify-content: space-between; padding: 7px 9px; border-radius: 8px; border: 1px solid transparent; background: transparent; color: var(--text); cursor: pointer; text-align: left; font: inherit; }
  .doc-item:hover { background: rgba(56,189,248,.08); }
  .doc-item.active { background: rgba(56,189,248,.14); border-color: rgba(56,189,248,.4); }
  .doc-name { font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .doc-badge { font-size: 11px; color: var(--muted); background: var(--panel-2); border-radius: 6px; padding: 1px 6px; margin-left: 6px; flex-shrink: 0; }
  .doc-del { background: transparent; border: 0; color: var(--muted); cursor: pointer; padding: 4px 6px; border-radius: 6px; font-size: 13px; }
  .doc-del:hover { color: var(--red); background: rgba(248,113,113,.12); }
  .new-form { display: flex; gap: 5px; }
  .new-form input, .quick-input, .inline-ta { background: var(--bg); color: var(--text); border: 1px solid var(--border); border-radius: 7px; padding: 7px 9px; font: 13px/1.45 inherit; outline: none; }
  .new-form input { flex: 1; min-width: 0; font-size: 12px; padding: 6px 8px; }
  .new-form input:focus, .quick-input:focus, .inline-ta:focus { border-color: var(--accent); }
  .btn { background: var(--panel-2); color: var(--text); border: 1px solid var(--border); border-radius: 7px; padding: 6px 10px; cursor: pointer; font: 600 12px inherit; transition: .12s; white-space: nowrap; }
  .btn:hover { border-color: var(--accent); color: #fff; }
  .btn-primary { background: linear-gradient(135deg, #0ea5e9, #06b6d4); border-color: transparent; color: #04131c; }
  .btn-primary:hover { filter: brightness(1.08); color: #04131c; }
  .btn-active { background: rgba(251,191,36,.18); border-color: var(--amber); color: var(--amber); }
  .btn-sm { padding: 4px 9px; font-size: 11.5px; }
  .main { display: flex; flex-direction: column; height: 100vh; overflow: hidden; }
  .topbar { display: flex; align-items: center; justify-content: space-between; gap: 8px; padding: 10px 16px; background: var(--panel); border-bottom: 1px solid var(--border); flex-wrap: wrap; }
  .title-block { display: flex; align-items: center; gap: 8px; min-width: 0; }
  .title { font-size: 15px; font-weight: 700; margin: 0; }
  .meta { font-size: 12px; color: var(--muted); }
  .tabs { display: flex; gap: 3px; background: var(--bg); padding: 3px; border-radius: 8px; border: 1px solid var(--border); }
  .tab { background: transparent; color: var(--muted); border: 0; border-radius: 6px; padding: 4px 10px; cursor: pointer; font: 600 12px inherit; }
  .tab.active { background: var(--panel-2); color: var(--accent-2); }
  .progress-wrap { height: 3px; background: var(--panel-2); width: 100%; }
  .progress-fill { height: 100%; background: linear-gradient(90deg, var(--accent), var(--green)); width: 0%; transition: width .2s ease; }
  .content { flex: 1; overflow-y: auto; padding: 16px 20px; }
  .card { max-width: 820px; margin: 0 auto; }
  .hint { font-size: 11.5px; color: var(--muted); margin-bottom: 10px; }
  .blk { position: relative; display: flex; align-items: flex-start; gap: 8px; border-radius: 8px; padding: 6px 9px; margin: 4px 0; border: 1px solid transparent; transition: .1s; }
  .blk:hover { background: rgba(56,189,248,.06); border-color: rgba(56,189,248,.24); }
  .blk.editing { background: var(--panel); border-color: var(--accent); padding: 8px 10px; }
  .blk-body { flex: 1; min-width: 0; cursor: text; }
  .blk-del { opacity: 0; background: transparent; border: 0; color: var(--muted); cursor: pointer; font-size: 14px; line-height: 1; padding: 2px 6px; border-radius: 5px; flex-shrink: 0; }
  .blk:hover .blk-del { opacity: 1; }
  .blk-del:hover { color: var(--red); background: rgba(248,113,113,.12); }
  .md h1 { font-size: 21px; margin: 0; color: #f8fafc; }
  .md h2 { font-size: 16px; margin: 2px 0; color: var(--accent-2); }
  .md h3 { font-size: 14.5px; margin: 2px 0; color: var(--accent); }
  .md p { margin: 0; color: #d5dfee; white-space: pre-wrap; }
  .md pre { background: #090d14; border: 1px solid var(--border); border-radius: 8px; padding: 10px 12px; overflow-x: auto; font: 12.5px/1.5 ui-monospace, SFMono-Regular, Menlo, monospace; color: #f8d486; margin: 6px 0; }
  .md code { background: var(--panel-2); color: #fbbf24; padding: 1px 5px; border-radius: 5px; font: 12.5px ui-monospace, SFMono-Regular, Menlo, monospace; }
  .md pre code { background: none; padding: 0; }
  .md blockquote { border-left: 3px solid var(--green); margin: 0; padding: 3px 10px; color: var(--muted); background: rgba(52,211,153,.05); border-radius: 0 6px 6px 0; }
  .task-blk { background: var(--panel); border-color: var(--border); padding: 7px 10px; }
  .task-blk.done { opacity: .65; }
  .task-blk.done .task-text { text-decoration: line-through; color: var(--muted); }
  .task-blk input[type=checkbox] { margin-top: 3px; accent-color: var(--green); width: 15px; height: 15px; cursor: pointer; flex-shrink: 0; }
  .bullet-dot { color: var(--accent); font-weight: 700; margin-top: 1px; }
  .inline-box { display: flex; flex-direction: column; gap: 6px; width: 100%; }
  .inline-ta { width: 100%; min-height: 44px; resize: vertical; background: #090d14; }
  .inline-bar { display: flex; gap: 6px; align-items: center; }
  .composer { margin-top: 18px; padding-top: 14px; border-top: 1px solid var(--border); display: flex; flex-direction: column; gap: 8px; }
  .kind-pills { display: flex; gap: 5px; flex-wrap: wrap; }
  .kind-pill { background: var(--panel); color: var(--muted); border: 1px solid var(--border); border-radius: 999px; padding: 4px 10px; font: 600 11.5px inherit; cursor: pointer; }
  .kind-pill:hover { color: var(--text); border-color: var(--accent); }
  .kind-pill.active { background: rgba(56,189,248,.16); color: var(--accent-2); border-color: var(--accent); }
  .quick-row { display: flex; gap: 6px; }
  .quick-input { flex: 1; min-width: 0; }
  .editor-wrap { display: flex; flex-direction: column; height: 100%; gap: 10px; max-width: 920px; margin: 0 auto; }
  .editor-wrap textarea { flex: 1; min-height: 300px; background: #090d14; color: var(--text); border: 1px solid var(--border); border-radius: 10px; padding: 14px; font: 13px/1.6 ui-monospace, SFMono-Regular, Menlo, monospace; resize: none; outline: none; }
  .editor-wrap textarea:focus { border-color: var(--accent); }
  .sb-bar { display: flex; align-items: center; justify-content: space-between; gap: 8px; margin-bottom: 10px; flex-wrap: wrap; }
  .sb-stage { position: relative; width: 100%; height: calc(100vh - 175px); border: 1px solid var(--border); border-radius: 10px; overflow: hidden; background: #fff; }
  .sb-stage iframe { width: 100%; height: 100%; border: 0; display: block; }
  .pin-overlay { position: absolute; inset: 0; cursor: crosshair; background: rgba(56,189,248,.04); display: none; }
  .pin-overlay.on { display: block; }
  .pin-dot { position: absolute; transform: translate(-50%,-50%); width: 22px; height: 22px; border-radius: 999px; background: var(--amber); color: #111; font: 700 11px/22px inherit; text-align: center; box-shadow: 0 2px 8px rgba(0,0,0,.45); pointer-events: none; }
  .pin-prompt { display: none; gap: 6px; margin-top: 8px; }
  .pin-prompt.on { display: flex; }
  .pin-prompt input { flex: 1; background: var(--panel); color: var(--text); border: 1px solid var(--amber); border-radius: 7px; padding: 7px 10px; font: 13px inherit; }
  .toast { position: fixed; right: 14px; bottom: 14px; background: var(--panel-2); border: 1px solid var(--accent); color: var(--text); padding: 7px 13px; border-radius: 8px; font-size: 12px; opacity: 0; pointer-events: none; transition: opacity .18s; z-index: 20; }
  .toast.show { opacity: 1; }
</style>
</head>
<body>
<div class="shell">
  <aside class="side">
    <div class="brand"><span>Artifacts</span><span class="live-pill">● LIVE</span></div>
    <div class="doc-list" id="docList"></div>
    <form class="new-form" id="newForm">
      <input id="newName" placeholder="new-doc (optional)" maxlength="48">
      <button class="btn btn-primary" type="submit" id="newBtn">+ New</button>
    </form>
  </aside>
  <main class="main">
    <div class="mobile-docs" id="mobileDocs"></div>
    <header class="topbar">
      <div class="title-block">
        <h1 class="title" id="docTitle"></h1>
        <span class="meta" id="docMeta">rev 0</span>
      </div>
      <div class="tabs">
        <button class="tab active" data-tab="board">Board</button>
        <button class="tab" data-tab="edit">Markdown</button>
        <button class="tab" data-tab="sandbox">HTML / JS</button>
      </div>
      <button class="btn btn-primary" id="broadcastBtn">Broadcast</button>
    </header>
    <div class="progress-wrap"><div class="progress-fill" id="progressFill"></div></div>
    <section class="content" id="content"></section>
  </main>
</div>
<div class="toast" id="toast"></div>
<script>
"###;
    let body = r###"let tab = "board";
let addKind = "task";
let docs = [];
let lastRevByName = {};
let lastActiveSeq = 0;
let editingLine = null;
let editorDirty = false;
let pinMode = false;
let pendingPin = null;
let toastTimer = null;

const KINDS = [
  { id: "task", label: "Task", prefix: "- [ ] ", ph: "Add a checklist task..." },
  { id: "note", label: "Paragraph", prefix: "", ph: "Write an instruction or paragraph..." },
  { id: "bullet", label: "Bullet", prefix: "- ", ph: "Add a bullet point..." },
  { id: "heading", label: "Heading", prefix: "## ", ph: "Add a section heading..." },
  { id: "quote", label: "Callout", prefix: "> ", ph: "Add a highlighted callout..." }
];

function showToast(msg) {
  const t = document.getElementById("toast");
  t.textContent = msg;
  t.classList.add("show");
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => t.classList.remove("show"), 2600);
}

function esc(s) {
  return String(s).replace(/&/g,"&amp;").replace(/</g,"&lt;").replace(/>/g,"&gt;").replace(/"/g,"&quot;");
}

function inlineMd(s) {
  return esc(s)
    .replace(/`([^`]+)`/g, "<code>$1</code>")
    .replace(/\*\*([^*]+)\*\*/g, "<strong>$1</strong>")
    .replace(/\*([^*]+)\*/g, "<em>$1</em>");
}

function currentDoc() {
  return docs.find(d => d.name === activeName) || docs[0] || { name: activeName, text: "", revision: 0, done: 0, total: 0 };
}

function extractHtmlSnippet(text) {
  const trimmed = text.trim();
  if (/^<!doctype\s+html|^<html|^<svg/i.test(trimmed)) return trimmed;
  const m = text.match(/```(?:html|svg)\s*\n([\s\S]*?)```/i);
  return m ? m[1] : null;
}

function parsePins(text) {
  const pins = [];
  const re = /\[UI #(\d+) @ (\d+)%,(\d+)%\]\s*(.*)/g;
  let m;
  while ((m = re.exec(text)) !== null) {
    pins.push({ n: Number(m[1]), x: Number(m[2]), y: Number(m[3]), note: m[4] });
  }
  return pins;
}

async function api(path, body) {
  try {
    const opts = body !== undefined
      ? { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify(body) }
      : { method: "GET" };
    const res = await fetch(BASE + path, opts);
    const data = await res.json();
    if (!res.ok && !data.error) data.error = `HTTP ${res.status}`;
    return data;
  } catch (err) {
    return { ok: false, error: String(err) };
  }
}

async function saveText(name, text, quiet, expectedRev) {
  const known = (docs.find(d => d.name === name) || {}).revision;
  const exp = typeof expectedRev === "number" ? expectedRev : (typeof lastRevByName[name] === "number" ? lastRevByName[name] : known);
  const payload = typeof exp === "number" ? { name, text, expected_revision: exp } : { name, text };
  const r = await api("/api/save", payload);
  if (!r.ok) {
    if (r.conflict) {
      showToast(`Conflict: '${name}' updated concurrently (rev ${r.current_revision}) — refreshed`);
      editingLine = null;
      editorDirty = false;
      await poll(true);
    } else {
      showToast(r.error || "Save failed");
    }
    return false;
  }
  editorDirty = false;
  if (!quiet) showToast(`Synced '${name}' (rev ${r.revision})`);
  await poll(true);
  return true;
}

async function createNewDoc(rawName) {
  const r = await api("/api/create", { name: rawName || "" });
  if (r.ok && r.name) {
    activeName = r.name;
    editingLine = null;
    editorDirty = false;
    await poll(true);
    showToast(`Created '${r.name}'`);
  } else {
    showToast(r.error || "Could not create document");
  }
}

async function deleteDoc(name) {
  if (docs.length <= 1) {
    showToast("Keep at least one artifact");
    return;
  }
  const r = await api("/api/delete", { name });
  if (r.ok) {
    if (activeName === name) activeName = "";
    editingLine = null;
    await poll(true);
    showToast(`Deleted '${name}'`);
  } else {
    showToast(r.error || "Delete failed");
  }
}

function mutateLines(fn, quiet = true) {
  const doc = currentDoc();
  const lines = doc.text.split("\n");
  fn(lines);
  doc.text = lines.join("\n");
  editingLine = null;
  renderContent();
  saveText(doc.name, doc.text, quiet);
}

function toggleLineCheckbox(lineIdx) {
  mutateLines(lines => {
    if (lineIdx < 0 || lineIdx >= lines.length) return;
    lines[lineIdx] = lines[lineIdx].replace(/^(\s*(?:[-*+]|\d+\.)\s+\[)([ xX])(\])/, (_, a, mark, b) => a + (mark === " " ? "x" : " ") + b);
  }, true);
}

function deleteLine(lineIdx) {
  mutateLines(lines => {
    if (lineIdx >= 0 && lineIdx < lines.length) lines.splice(lineIdx, 1);
  }, false);
}

function startInlineEdit(lineIdx, prefix, bodyText) {
  if (editingLine === lineIdx) return;
  editingLine = lineIdx;
  const row = document.querySelector(`[data-blk-row="${lineIdx}"]`);
  const holder = document.querySelector(`[data-edit-body="${lineIdx}"]`);
  if (!holder) return;
  if (row) row.classList.add("editing");

  const wrap = document.createElement("div");
  wrap.className = "inline-box";
  wrap.addEventListener("click", ev => ev.stopPropagation());
  wrap.addEventListener("mousedown", ev => ev.stopPropagation());

  const ta = document.createElement("textarea");
  ta.className = "inline-ta";
  ta.value = bodyText;
  ta.rows = Math.max(2, Math.min(6, Math.ceil((bodyText.length + 1) / 56)));

  const bar = document.createElement("div");
  bar.className = "inline-bar";
  const saveBtn = document.createElement("button");
  saveBtn.type = "button";
  saveBtn.className = "btn btn-primary btn-sm";
  saveBtn.textContent = "Save";
  const cancelBtn = document.createElement("button");
  cancelBtn.type = "button";
  cancelBtn.className = "btn btn-sm";
  cancelBtn.textContent = "Cancel";
  const tip = document.createElement("span");
  tip.className = "meta";
  tip.textContent = "Enter or Save · Esc cancel";
  bar.append(saveBtn, cancelBtn, tip);
  wrap.append(ta, bar);

  holder.innerHTML = "";
  holder.appendChild(wrap);
  ta.focus();
  ta.setSelectionRange(ta.value.length, ta.value.length);

  const commit = () => {
    const val = ta.value.replace(/\r?\n/g, " ");
    mutateLines(lines => {
      if (lineIdx >= 0 && lineIdx < lines.length) lines[lineIdx] = prefix + val;
    }, false);
  };
  const cancel = () => {
    editingLine = null;
    renderContent();
  };
  saveBtn.onclick = ev => { ev.stopPropagation(); commit(); };
  cancelBtn.onclick = ev => { ev.stopPropagation(); cancel(); };
  ta.addEventListener("keydown", ev => {
    if (ev.key === "Escape") { ev.preventDefault(); cancel(); }
    else if (ev.key === "Enter" && !ev.shiftKey) { ev.preventDefault(); commit(); }
  });
}

function renderSidebar() {
  const el = document.getElementById("docList");
  const mob = document.getElementById("mobileDocs");
  el.innerHTML = "";
  mob.innerHTML = "";
  const curName = currentDoc().name;
  for (const d of docs) {
    const badge = d.total > 0 ? `${d.done}/${d.total}` : `r${d.revision}`;
    const row = document.createElement("div");
    row.className = "doc-row";
    const btn = document.createElement("button");
    btn.type = "button";
    btn.className = "doc-item" + (d.name === curName ? " active" : "");
    btn.innerHTML = `<span class="doc-name">${esc(d.name)}</span><span class="doc-badge">${badge}</span>`;
    btn.onclick = () => { activeName = d.name; editingLine = null; editorDirty = false; renderAll(); };
    row.appendChild(btn);
    if (docs.length > 1) {
      const del = document.createElement("button");
      del.type = "button";
      del.className = "doc-del";
      del.textContent = "×";
      del.title = "Delete document";
      del.onclick = ev => { ev.stopPropagation(); deleteDoc(d.name); };
      row.appendChild(del);
    }
    el.appendChild(row);

    const pill = document.createElement("button");
    pill.type = "button";
    pill.className = "kind-pill" + (d.name === curName ? " active" : "");
    pill.textContent = d.name;
    pill.onclick = () => { activeName = d.name; editingLine = null; editorDirty = false; renderAll(); };
    mob.appendChild(pill);
  }
  const mobNew = document.createElement("button");
  mobNew.type = "button";
  mobNew.className = "btn btn-primary btn-sm";
  mobNew.textContent = "+ New";
  mobNew.onclick = () => createNewDoc("");
  mob.appendChild(mobNew);
}

function renderBoard(doc) {
  const lines = doc.text.split("\n");
  let inFence = false;
  let fenceBuf = [];
  let html = '<div class="card md"><div class="hint">Double-click any title, instruction, paragraph, bullet, or task to edit inline</div>';
  const editableMeta = [];

  lines.forEach((line, idx) => {
    if (/^\s*(?:```|~~~)/.test(line)) {
      if (inFence) {
        html += `<pre><code>${esc(fenceBuf.join("\n"))}</code></pre>`;
        fenceBuf = [];
      }
      inFence = !inFence;
      return;
    }
    if (inFence) { fenceBuf.push(line); return; }
    if (line.trim() === "") return;

    const delBtn = `<button type="button" class="blk-del" data-del="${idx}" title="Delete">&times;</button>`;
    const h = line.match(/^(\s*#{1,3}\s+)(.*)$/);
    if (h) {
      const lvl = h[1].trim().length;
      editableMeta.push({ idx, prefix: h[1], text: h[2] });
      html += `<div class="blk" data-blk-row="${idx}"><div class="blk-body" data-edit-body="${idx}"><h${lvl}>${inlineMd(h[2])}</h${lvl}></div>${delBtn}</div>`;
      return;
    }
    const cb = line.match(/^(\s*(?:[-*+]|\d+\.)\s+\[([ xX])\]\s*)(.*)$/);
    if (cb) {
      const done = cb[2] !== " ";
      editableMeta.push({ idx, prefix: cb[1], text: cb[3] });
      html += `<div class="blk task-blk ${done ? "done" : ""}" data-blk-row="${idx}"><input type="checkbox" data-chk="${idx}" ${done ? "checked" : ""}><div class="blk-body task-text" data-edit-body="${idx}">${inlineMd(cb[3])}</div>${delBtn}</div>`;
      return;
    }
    const bul = line.match(/^(\s*[-*+]\s+)(.*)$/);
    if (bul) {
      editableMeta.push({ idx, prefix: bul[1], text: bul[2] });
      html += `<div class="blk" data-blk-row="${idx}"><span class="bullet-dot">•</span><div class="blk-body" data-edit-body="${idx}">${inlineMd(bul[2])}</div>${delBtn}</div>`;
      return;
    }
    const bq = line.match(/^(\s*>\s?)(.*)$/);
    if (bq) {
      editableMeta.push({ idx, prefix: bq[1], text: bq[2] });
      html += `<div class="blk" data-blk-row="${idx}"><div class="blk-body" data-edit-body="${idx}"><blockquote>${inlineMd(bq[2])}</blockquote></div>${delBtn}</div>`;
      return;
    }
    editableMeta.push({ idx, prefix: "", text: line });
    html += `<div class="blk" data-blk-row="${idx}"><div class="blk-body" data-edit-body="${idx}"><p>${inlineMd(line)}</p></div>${delBtn}</div>`;
  });
  if (inFence && fenceBuf.length) html += `<pre><code>${esc(fenceBuf.join("\n"))}</code></pre>`;

  const curKind = KINDS.find(k => k.id === addKind) || KINDS[0];
  const pillsHtml = KINDS.map(k =>
    `<button type="button" class="kind-pill ${k.id === curKind.id ? "active" : ""}" data-kind="${k.id}">${k.label}</button>`
  ).join("");
  html += `<form class="composer" id="quickAdd">
    <div class="kind-pills">${pillsHtml}</div>
    <div class="quick-row">
      <input class="quick-input" id="quickText" placeholder="${esc(curKind.ph)}">
      <button class="btn btn-primary" type="submit">+ Add ${curKind.label}</button>
    </div>
  </form></div>`;

  const box = document.getElementById("content");
  box.innerHTML = html;

  box.querySelectorAll("[data-chk]").forEach(cb => {
    cb.addEventListener("change", () => toggleLineCheckbox(Number(cb.dataset.chk)));
  });
  box.querySelectorAll("[data-del]").forEach(btn => {
    btn.addEventListener("click", ev => { ev.stopPropagation(); deleteLine(Number(btn.dataset.del)); });
  });
  for (const m of editableMeta) {
    const bodyEl = box.querySelector(`[data-edit-body="${m.idx}"]`);
    if (bodyEl) {
      bodyEl.addEventListener("dblclick", ev => {
        ev.stopPropagation();
        startInlineEdit(m.idx, m.prefix, m.text);
      });
    }
  }
  box.querySelectorAll("[data-kind]").forEach(pill => {
    pill.addEventListener("click", () => {
      addKind = pill.dataset.kind;
      const savedDraft = document.getElementById("quickText")?.value || "";
      renderBoard(currentDoc());
      const inp = document.getElementById("quickText");
      if (inp) { inp.value = savedDraft; inp.focus(); }
    });
  });
  document.getElementById("quickAdd").addEventListener("submit", ev => {
    ev.preventDefault();
    const inp = document.getElementById("quickText");
    const v = inp.value.trim();
    if (!v) { inp.focus(); return; }
    const liveDoc = currentDoc();
    inp.value = "";
    const k = KINDS.find(x => x.id === addKind) || KINDS[0];
    const sep = liveDoc.text.endsWith("\n") || liveDoc.text === "" ? "" : "\n";
    liveDoc.text = liveDoc.text + sep + k.prefix + v;
    renderContent();
    saveText(liveDoc.name, liveDoc.text, false);
  });
}

function renderEdit(doc) {
  const box = document.getElementById("content");
  box.innerHTML = `<div class="editor-wrap"><textarea id="mdArea"></textarea><div style="display:flex;justify-content:space-between;align-items:center"><span class="meta">Synced to ContextBus &amp; MCP (bilterm://context/artifact:${esc(doc.name)})</span><button type="button" class="btn btn-primary" id="saveEditBtn">Save &amp; Sync</button></div></div>`;
  const ta = document.getElementById("mdArea");
  ta.value = doc.text;
  ta.addEventListener("input", () => { editorDirty = true; });
  ta.addEventListener("keydown", ev => {
    if ((ev.metaKey || ev.ctrlKey) && ev.key.toLowerCase() === "s") {
      ev.preventDefault();
      saveText(currentDoc().name, ta.value, false);
    }
  });
  document.getElementById("saveEditBtn").onclick = () => saveText(currentDoc().name, ta.value, false);
}

function insertSampleUi() {
  const liveDoc = currentDoc();
  const snippet = `\n\n## UI Prototype\n\`\`\`html\n<!doctype html><html><body style="margin:0;min-height:100vh;display:grid;place-items:center;background:#0f172a;color:#e2e8f0;font-family:system-ui"><div style="background:#1e293b;border:1px solid #334155;border-radius:14px;padding:24px 28px;max-width:340px;box-shadow:0 12px 32px rgba(0,0,0,.35)"><h2 style="margin:0 0 8px;color:#38bdf8">Agent Card Preview</h2><p style="margin:0 0 16px;color:#94a3b8;font-size:14px">Toggle Pin Feedback above and click anywhere on this UI to drop review notes for your agents.</p><button onclick="this.textContent='Clicked!'" style="background:#38bdf8;color:#082f49;border:0;border-radius:8px;padding:8px 14px;font-weight:700;cursor:pointer">Primary Action</button></div></body></html>\n\`\`\``;
  liveDoc.text = liveDoc.text + snippet;
  saveText(liveDoc.name, liveDoc.text, false);
}

function injectIframeCsp(html) {
  const meta = `<meta http-equiv="Content-Security-Policy" content="default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; img-src data:; connect-src 'none'">`;
  const headRe = /<head(?:\s[^>]*)?>/i;
  if (headRe.test(html)) {
    return html.replace(headRe, m => m + meta);
  }
  return meta + html;
}

function renderSandbox(doc) {
  const htmlSrc = extractHtmlSnippet(doc.text);
  const pins = parsePins(doc.text);
  const box = document.getElementById("content");
  if (!htmlSrc) {
    box.innerHTML = `<div class="card md"><h2>HTML / JS Live Preview</h2><p>Add a <code>\`\`\`html</code> fenced block in this artifact (or click below for a starter UI card) to preview interactive HTML/JS and drop visual review pins for your agents.</p><button type="button" class="btn btn-primary" id="sampleUiBtn">Insert Starter UI Prototype</button></div>`;
    document.getElementById("sampleUiBtn").onclick = () => insertSampleUi();
    return;
  }
  const pinDots = pins.map(p => `<div class="pin-dot" style="left:${p.x}%;top:${p.y}%" title="${esc(p.note)}">${p.n}</div>`).join("");
  box.innerHTML = `
    <div class="sb-bar">
      <span class="meta">Live HTML/JS preview · ${pins.length} review pin${pins.length === 1 ? "" : "s"}</span>
      <button type="button" class="btn ${pinMode ? "btn-active" : ""}" id="pinToggleBtn">${pinMode ? "● Pin Mode ON (click UI)" : "Pin Feedback"}</button>
    </div>
    <div class="sb-stage" id="sbStage">
      <iframe sandbox="allow-scripts" id="sbFrame"></iframe>
      <div class="pin-overlay ${pinMode ? "on" : ""}" id="pinOverlay">${pinDots}</div>
    </div>
    <form class="pin-prompt ${pendingPin ? "on" : ""}" id="pinForm">
      <input id="pinNoteInput" placeholder="Describe the UI change at this pin...">
      <button class="btn btn-primary" type="submit">Add Pin to Checklist</button>
      <button class="btn" type="button" id="pinCancelBtn">Cancel</button>
    </form>`;
  document.getElementById("sbFrame").srcdoc = injectIframeCsp(htmlSrc);
  document.getElementById("pinToggleBtn").onclick = () => {
    pinMode = !pinMode;
    pendingPin = null;
    renderSandbox(currentDoc());
  };
  document.getElementById("pinOverlay").addEventListener("click", ev => {
    const r = ev.currentTarget.getBoundingClientRect();
    if (!r.width || !r.height) return;
    const x = Math.round(((ev.clientX - r.left) / r.width) * 100);
    const y = Math.round(((ev.clientY - r.top) / r.height) * 100);
    pendingPin = { x, y, n: parsePins(currentDoc().text).length + 1 };
    const f = document.getElementById("pinForm");
    f.classList.add("on");
    document.getElementById("pinNoteInput").focus();
  });
  document.getElementById("pinCancelBtn").onclick = () => {
    pendingPin = null;
    document.getElementById("pinForm").classList.remove("on");
  };
  document.getElementById("pinForm").addEventListener("submit", ev => {
    ev.preventDefault();
    const inp = document.getElementById("pinNoteInput");
    const note = inp.value.trim();
    if (!note || !pendingPin) return;
    const liveDoc = currentDoc();
    const sep = liveDoc.text.endsWith("\n") || liveDoc.text === "" ? "" : "\n";
    liveDoc.text = liveDoc.text + sep + `- [ ] [UI #${pendingPin.n} @ ${pendingPin.x}%,${pendingPin.y}%] ${note}`;
    pendingPin = null;
    saveText(liveDoc.name, liveDoc.text, false);
  });
}

function renderContent() {
  const doc = currentDoc();
  document.getElementById("docTitle").textContent = doc.name;
  const pct = doc.total > 0 ? Math.round((doc.done / doc.total) * 100) : 0;
  document.getElementById("docMeta").textContent = doc.total > 0 ? `rev ${doc.revision} · ${doc.done}/${doc.total} (${pct}%)` : `rev ${doc.revision}`;
  document.getElementById("progressFill").style.width = pct + "%";
  if (tab === "board") renderBoard(doc);
  else if (tab === "edit") renderEdit(doc);
  else renderSandbox(doc);
}

function renderAll() { renderSidebar(); renderContent(); }

document.querySelectorAll(".tab").forEach(b => {
  b.onclick = () => {
    tab = b.dataset.tab;
    editingLine = null;
    document.querySelectorAll(".tab").forEach(x => x.classList.toggle("active", x === b));
    renderContent();
  };
});

document.getElementById("newForm").addEventListener("submit", async ev => {
  ev.preventDefault();
  const inp = document.getElementById("newName");
  const raw = inp.value.trim();
  inp.value = "";
  await createNewDoc(raw);
});

document.getElementById("broadcastBtn").onclick = async () => {
  const doc = currentDoc();
  const r = await api("/api/broadcast", { name: doc.name, note: `updated via Artifact Studio (rev ${doc.revision})` });
  showToast(r.ok ? `Broadcasted '${doc.name}' to agents` : (r.error || "Broadcast failed"));
};

async function poll(force) {
  const data = await api("/api/state");
  if (!data.ok) return;
  docs = data.docs || [];
  let switched = false;
  if (typeof data.active_seq === "number" && data.active_seq > lastActiveSeq) {
    lastActiveSeq = data.active_seq;
    if (data.active && docs.some(d => d.name === data.active) && activeName !== data.active) {
      activeName = data.active;
      editingLine = null;
      switched = true;
    }
  }
  if (!docs.some(d => d.name === activeName) && docs.length) activeName = docs[0].name;
  const cur = currentDoc();
  const changed = force || switched || lastRevByName[cur.name] !== cur.revision;
  for (const d of docs) lastRevByName[d.name] = d.revision;
  renderSidebar();
  if (changed && editingLine === null && !(tab === "edit" && editorDirty)) renderContent();
}

poll(true);
setInterval(() => poll(false), 1200);
</script>
</body>
</html>"###;
    format!("{head}const BASE = {base_js};\nlet activeName = {init_js};\n{body}")
}

#[cfg(test)]
mod tests {
    use super::studio_app_html;

    fn extract_inject_fn(html: &str) -> String {
        let start = html.find("function injectIframeCsp(").expect("injectIframeCsp present");
        let rest = &html[start..];
        let end = rest.find("\n}\n").expect("function end") + 3;
        rest[..end].to_string()
    }

    #[test]
    fn inject_iframe_csp_head_regex_requires_head_boundary() {
        let f = extract_inject_fn(&studio_app_html("/t", "plan"));
        // Source-level: the regex must require whitespace, `>` after `<head`, so `<header>` never matches.
        assert!(f.contains(r"/<head(?:\s[^>]*)?>/i"), "{f}");
    }

    #[test]
    fn inject_iframe_csp_behaviour_under_node() {
        let Ok(ver) = std::process::Command::new("node").arg("--version").output() else {
            eprintln!("node missing; skipping");
            return;
        };
        if !ver.status.success() {
            return;
        }
        let f = extract_inject_fn(&studio_app_html("/t", "plan"));
        let script = format!(
            "{f}\nconst a = injectIframeCsp('<header>x</header><p>y</p>');\n\
             if (!a.startsWith('<meta http-equiv=\"Content-Security-Policy\"')) throw new Error('header: meta not prepended: ' + a);\n\
             if (a.indexOf('<header>') < a.indexOf('<meta')) throw new Error('meta inside/after header');\n\
             const b = injectIframeCsp('<html><head lang=\"en\"><title>t</title></head><body></body></html>');\n\
             if (!b.includes('<head lang=\"en\"><meta http-equiv')) throw new Error('head not injected: ' + b);\n\
             const c = injectIframeCsp('<HEAD><title>t</title></HEAD>');\n\
             if (!c.startsWith('<HEAD><meta')) throw new Error('uppercase: ' + c);\n"
        );
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("t.js");
        std::fs::write(&p, script).unwrap();
        let out = std::process::Command::new("node").arg(&p).output().unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    }
}
