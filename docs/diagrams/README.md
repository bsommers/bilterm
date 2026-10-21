# Bilterm Visual Architecture & Diagram Catalog

[← Back to Documentation Hub](../README.md) | [← Back to Project README](../../README.md)

This directory contains multi-format architectural diagrams for **Bilterm** across three distinct mediums:

1. **Terminal-Ready ASCII Schematics** ([`ascii_architecture.txt`](ascii_architecture.txt))
2. **Mermaid Diagrams** (`*.mmd`)
3. **Native draw.io Vector XML Diagrams** (`*.drawio`)

---

## Diagram Inventory

| Topic | ASCII | Mermaid (`.mmd`) | draw.io XML (`.drawio`) | Description |
|-------|-------|------------------|-------------------------|-------------|
| **System Architecture** | [`ascii_architecture.txt`](ascii_architecture.txt) | [`architecture.mmd`](architecture.mmd) | [`architecture.drawio`](architecture.drawio) | Subsystems, `WindowManager`, `CompositePane`, `VtParser`, `PtyHandle`, and 3-pass Ratatui compositor |
| **Execution & Input Flow** | [`ascii_architecture.txt`](ascii_architecture.txt) | [`execution_flow.mmd`](execution_flow.mmd) | [`execution_flow.drawio`](execution_flow.drawio) | Keystroke routing, Tab completion, command submission, 64KB PTY batching, and frame rendering |
| **Pane Lifecycle & Storage** | [`ascii_architecture.txt`](ascii_architecture.txt) | [`pane_lifecycle.mmd`](pane_lifecycle.mmd) | [`storage_and_tmux.drawio`](storage_and_tmux.drawio) | 3-signal line-discipline state machine (`CMD BOX`, `RAW PTY`, `ALT SCR`, `!ECHO LOCK`), history persistence, and `tmux` bridge |

---

## How to View Each Format

### 1. ASCII Diagrams (`ascii_architecture.txt`)

View directly in any terminal or pager:

```bash
cat docs/diagrams/ascii_architecture.txt
```

### 2. Mermaid Diagrams (`*.mmd`)

- **GitHub**: Rendered natively inside [`README.md`](../../README.md) and [`docs/architecture.md`](../architecture.md).
- **CLI (`@mermaid-js/mermaid-cli`)**:
  ```bash
  npx @mermaid-js/mermaid-cli -i docs/diagrams/architecture.mmd -o docs/diagrams/architecture.svg
  ```

### 3. Native draw.io XML Diagrams (`*.drawio`)

- **Web**: Open [https://app.diagrams.net](https://app.diagrams.net) and choose **File -> Open from -> Device...**
- **VS Code / Antigravity IDE**: Install the `hediet.vscode-drawio` extension and open any `.drawio` file directly in the editor.
