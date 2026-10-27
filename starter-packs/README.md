# Bilterm Starter Packs

Ready-to-run configuration presets (`config.toml`), declarative automation playbooks (`playbook.toml`), and launch scripts for Bilterm's most common engineering, DevOps, multi-agent AI, and retro workstation workflows.

[← Back to Project README](../README.md) | [User Guide →](../docs/user-guide/README.md)

---

## Catalog

| Starter Pack | Directory | Primary Use Case | Quick Launch Command |
| :--- | :--- | :--- | :--- |
| **01. Multi-Agent Coding Swarm** | [`01-multi-agent-swarm/`](01-multi-agent-swarm/) | 4-pane autonomous AI agent swarm (Architect, Implementer, Verifier, Security) with MCP stdio + JSON-RPC control | `bilterm --gui -c starter-packs/01-multi-agent-swarm/config.toml` |
| **02. Remote Dev & SSH Fleet** | [`02-remote-dev-and-fleet/`](02-remote-dev-and-fleet/) | Distributed SSH + `tmux -CC` fleet management, synchronized `:broadcast` patching, and slide-out fleet drawer | `bilterm --gui -c starter-packs/02-remote-dev-and-fleet/config.toml` |
| **03. Robot Scripting & Headless QA** | [`03-robot-scripting-and-qa/`](03-robot-scripting-and-qa/) | Deterministic CI/CD terminal testing, regex output assertions, `.cast` recording, and `.ppm` visual regression | `bilterm run starter-packs/03-robot-scripting-and-qa/playbook.toml` |
| **04. Sysadmin Incident War Room** | [`04-sysadmin-incident-war-room/`](04-sysadmin-incident-war-room/) | 3-way split war room (`120%` zoomed root console, `80%` compact `nmon` telemetry, `100%` live log stream, `:lock on` safety) | `bilterm --gui -c starter-packs/04-sysadmin-incident-war-room/config.toml` |
| **05. Retro CRT Workstations** | [`05-retro-crt-workstation/`](05-retro-crt-workstation/) | Authentic IBM 3278 Green, IBM 3279 Amber, Borland Turbo Vision Blue, and Cyberpunk Neon workstation presets | `bilterm --gui -c starter-packs/05-retro-crt-workstation/ibm-3278-green.toml` |

---

## Verifying All Starter Pack Playbooks

Every playbook in `starter-packs/` is deterministic and can be executed headlessly in under 50ms:

```bash
bilterm run starter-packs/01-multi-agent-swarm/playbook.toml
bilterm run starter-packs/02-remote-dev-and-fleet/playbook.toml
bilterm run starter-packs/03-robot-scripting-and-qa/playbook.toml
bilterm run starter-packs/04-sysadmin-incident-war-room/playbook.toml
```
