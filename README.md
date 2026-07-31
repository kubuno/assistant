<!--
  SPDX-FileCopyrightText: 2026 Kubuno contributors
  SPDX-License-Identifier: AGPL-3.0-or-later
-->

# Kubuno Assistant

[![License: AGPL v3](https://img.shields.io/badge/License-AGPL_v3-blue.svg)](LICENSE)
![Rust](https://img.shields.io/badge/Rust-edition_2021-orange.svg)
![React](https://img.shields.io/badge/React-19-61dafb.svg)
![Module](https://img.shields.io/badge/Kubuno-module-4D38DB.svg)

**Kubuno Assistant — assistant IA multi-modèles (Ollama local, agents, RAG)**

A module for [Kubuno](https://github.com/kubuno/core), the self-hosted, libre (AGPLv3) cloud platform.

## Features

- **Multi-model chat** — converse with local models served by Ollama (or any configured provider), with per-conversation model selection and token accounting.
- **Agents** — reusable assistant profiles (system prompt, preferred model, avatar, prompt suggestions), including shared system agents available to every user. Agents can run an agentic loop and call tools through an MCP client.
- **Conversations, folders & organization** — pin, rename, archive, drag-and-drop reordering, and project folders to group related conversations. Every conversation is addressable by URL (`/assistant/#conversation/<id>`), so direct links and the browser Back button just work.
- **Rich answers** — assistant replies are rendered as full Markdown (headings, links, styled quotes, code blocks).
- **Delta sync (local-first)** — `GET /conversations/delta`, `/folders/delta` and `/agents/delta` expose owner-scoped change feeds (monotonic `change_seq`, tombstones for deletions, pagination), and creation endpoints accept client-minted UUIDs, so an offline-capable client can replay its local changes and pull only what changed since its last cursor.
- **Per-user settings** — each user tunes the assistant to their own preferences.

## Architecture

A standalone Rust process that registers with the [core](https://github.com/kubuno/core) at startup; the core proxies its routes (`/api/v1/assistant/*`) and serves its runtime-loaded React frontend bundle.

- **Backend** — `src/`: Axum + SQLx (PostgreSQL, schema `assistant`); migrations in `migrations/`.
- **Frontend** — `frontend/`: a React bundle built to `entry.js`, consuming `@kubuno/sdk`, `@kubuno/ui` and `@kubuno/drive` from npm (provided by the host at runtime via the import map).

## Install

This module ships in the **all-in-one [Kubuno](https://github.com/kubuno/core) Docker image** (`ghcr.io/kubuno/kubuno`) — the easiest way to self-host a full Kubuno instance (core + every module). See **[kubuno/docker](https://github.com/kubuno/docker)** for `docker compose` instructions.

**Native packages** are attached to each [GitHub Release](https://github.com/kubuno/assistant/releases) (built by CI on every `v*` tag):

| Platform | Package |
|---|---|
| Debian / Ubuntu | `kubuno-assistant_*.deb` |
| Fedora / RHEL / openSUSE | `kubuno-assistant-*.rpm` |
| Windows | `kubuno-assistant-setup-*-x64.exe` (NSIS installer) |
| macOS | `kubuno-assistant-*.pkg` |

Each package installs the module into an existing Kubuno core installation and restarts the platform service so the core picks it up.

To build this module from source, see below.

## Build

**Requirements:** Rust ≥ 1.82, Node.js ≥ 24, PostgreSQL 16.

```bash
cargo build --release                     # → target/release/kubuno-assistant
cd frontend && npm ci && npm run build     # → dist/{entry.js, entry.css}
bash build_deb.sh                          # → dist/kubuno-assistant_*.deb
```

Other platforms (same auto-detected layout as the `.deb`, so the core discovers the module identically):

```bash
bash build_rpm.sh          # Fedora/RHEL/openSUSE → dist/kubuno-assistant-*.rpm
bash build_windows.sh      # Windows (NSIS)       → dist/kubuno-assistant-setup-*-x64.exe
bash build_macos.sh        # macOS (on a Mac)     → dist/kubuno-assistant-*.pkg
```

> Shared dependencies come from Kubuno — no `kubuno/core` checkout required:
> - **Rust** — shared crates via tagged git dependencies on `kubuno/core`.
> - **Frontend** — `@kubuno/sdk`, `@kubuno/ui`, `@kubuno/drive` from the `@kubuno` npm scope.

## License

[AGPL-3.0-or-later](LICENSE) © Kubuno contributors.
