<!--
  SPDX-FileCopyrightText: 2026 Kubuno contributors
  SPDX-License-Identifier: AGPL-3.0-or-later
-->

<div align="center">

<img src=".github/logo.png" alt="Kubuno Assistant logo" width="120">

# Kubuno — Assistant

[![License: AGPL v3](https://img.shields.io/badge/License-AGPL_v3-blue.svg)](LICENSE)
![Rust](https://img.shields.io/badge/Rust-edition_2021-orange.svg)
![React](https://img.shields.io/badge/React-19-61dafb.svg)
![Status](https://img.shields.io/badge/status-alpha-yellow.svg)
![Module](https://img.shields.io/badge/Kubuno-module-4D38DB.svg)

**A self-hosted, multi-model AI assistant for Kubuno — chat with local models, build reusable agents, and let them call tools, all without your conversations ever leaving your server.**

Assistant is a module for [Kubuno](https://github.com/kubuno/core), the self-hosted, libre (AGPLv3) cloud platform — a sovereign alternative to Google Workspace and Microsoft 365. It brings a private, keep-your-data assistant to your instance: run open models on your own hardware, or point it at a configured provider, while the platform decides who may use what.

</div>

---

## ✨ Features

- 🧠 **Multi-model chat** — converse with local models served by your own runtime, or with any configured provider, picking the model per conversation. Each exchange tracks its token usage.
- 🤖 **Agents** — reusable assistant profiles (system prompt, preferred model, avatar, prompt suggestions), including shared *system agents* available to every user. An agent can run an agentic loop and call tools through an MCP client.
- 🛠️ **Tool calling** — when tools are enabled, an agent can chain several tool rounds within a single answer (bounded by an administrator), so it can look things up and act before replying.
- 🗂️ **Conversations, folders & organization** — pin, rename, archive, drag-and-drop reordering, and project folders to group related conversations. Every conversation is addressable by URL (`/assistant/#conversation/<id>`), so direct links and the browser Back button just work.
- 📝 **Rich answers** — assistant replies are rendered as full Markdown (headings, links, styled quotes, code blocks).
- 🔄 **Delta sync (local-first)** — `GET /conversations/delta`, `/folders/delta` and `/agents/delta` expose owner-scoped change feeds (monotonic `change_seq`, tombstones for deletions, pagination), and creation endpoints accept client-minted UUIDs, so an offline-capable client can replay its local changes and pull only what changed since its last cursor.
- 🛎️ **Right-rail mini-panel** — your conversations follow you across the shell, so you can pick one up from any module without leaving where you are.
- 🎛️ **Administrable by policy** — an administrator can forbid remote engines entirely (nothing then leaves the server), restrict the usable models to an allow-list, disable custom agents, cap message and response length, bound the replayed history window, toggle tool calling, and set a retention window after which idle conversations are pruned.
- 👤 **Per-user settings** — each user tunes the assistant to their own preferences.

## 🏗️ Architecture

Like every Kubuno app, Assistant is an **independent process**, not a library linked into the core. It registers with the [core](https://github.com/kubuno/core) at startup; the core then proxies its routes (`/api/v1/assistant/*`), distributes platform events to it, serves its runtime-loaded React frontend bundle and manages its lifecycle.

- **Port** — the backend listens on `127.0.0.1:3107` and is reached only through the core's reverse proxy.
- **Backend** — `src/`: Axum + SQLx over PostgreSQL, confined to the `assistant` schema; migrations in `migrations/`.
- **Frontend** — `frontend/`: a React 19 bundle built to `entry.js` + `entry.css`, consuming `@kubuno/sdk`, `@ui` (`@kubuno/ui`) and `@kubuno/drive`. At runtime those specifiers are `external` and resolved by the host's import map to its single shared instances; the npm packages are used only for building and type-checking.
- **Trust boundary** — proxied requests are authenticated from a signed `X-Kubuno-Auth` token minted by the core (see `kubuno-modauth`), never from plain `X-Kubuno-User-*` headers.

## 📦 Install

The easiest way to self-host a full Kubuno instance (core + every module) is the **all-in-one Docker image** (`ghcr.io/kubuno/kubuno`), which already bundles this module — see **[kubuno/docker](https://github.com/kubuno/docker)** for `docker compose` instructions.

To add the module to an existing instance, install its **`.kbpkg`** — the single, cross-platform package format a Kubuno server unpacks by itself (no `.deb`/`.rpm`/`.exe`/`.pkg`, and no external tools). Each tagged release (`v*`) attaches a Linux `.kbpkg` (built by `build.yml`) and Windows/macOS `.kbpkg` files (built by `dist.yml`) to its [GitHub Release](https://github.com/kubuno/assistant/releases):

```bash
# From the admin console: Modules → Install, then drop the .kbpkg — or, offline, from the CLI:
sudo kubuno modules:install dist/assistant-<version>-<os>-<arch>.kbpkg
sudo systemctl restart kubuno     # the core loads the module on (re)start
```

## 🛠️ Build & development

**Requirements:** Rust ≥ 1.82, Node.js ≥ 24, PostgreSQL 16. No `kubuno/core` checkout is needed — shared Rust crates come from tagged git dependencies, and the `@kubuno/*` frontend libraries from the public npm scope.

```bash
cargo build --release                     # → target/release/kubuno-assistant
cd frontend && npm ci && npm run build     # → dist/{entry.js, entry.css}

bash build_kbpkg.sh                        # → dist/assistant-<version>-<os>-<arch>.kbpkg
bash build_kbpkg.sh --install              # build, install into the module store, restart
```

Once the module has been installed at least once, iterate quickly without repackaging:

```bash
bash ../_tools/deploy_local.sh assistant             # backend + frontend
bash ../_tools/deploy_local.sh assistant --frontend  # frontend only (fastest)
```

## 📦 Tech stack

Rust 2021 · Axum 0.7 · Tokio · SQLx 0.8 (PostgreSQL 16, schema `assistant`) · an MCP client for tool calls — React 19 · TypeScript · Vite · Tailwind CSS v4 · Zustand · React Query, on the shared `@kubuno/sdk`, `@ui` and `@kubuno/drive` surfaces.

## 🤝 Contributing

Contributions are welcome. Please open an issue to discuss any significant change before submitting a pull request.

## 📄 License

[AGPL-3.0-or-later](LICENSE) © Kubuno contributors.
