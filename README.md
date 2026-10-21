# Closer

Closer is a desktop app for B2B lead research and qualification. Give it a list of companies and it researches each one with an AI agent, finds the right people to talk to, and scores every lead against your ideal customer profile, so you spend your time on the deals worth closing.

## What it does

- **Company research**: pulls together each company's business model, products and services, size, funding and recent news
- **People discovery**: finds relevant decision makers at each company, with their roles and profiles
- **Lead scoring**: scores every lead from 0 to 100 against your criteria (industry, size, location, growth signals, urgency), with the reasoning behind each score
- **Conversation prep**: drafts talking points for reaching out to each person
- **Live progress**: watch the research happen as the agent works through each lead

Research runs through the Claude Code CLI (Claude models) or the Codex CLI (ChatGPT models), signed in with your own Claude or ChatGPT account. Closer installs and updates these for you; there is nothing to set up by hand.

## Install

Download the installer for your computer from the [latest release](https://github.com/tekvyy/closers/releases/latest):

- **Windows**: the file ending in `windows_x64-setup.exe`. No administrator rights needed.
- **macOS**: the `.dmg` for your Mac (`aarch64` for Apple Silicon, `x64` for Intel). Closer isn't code-signed yet, so before opening it the first time run `xattr -cr /Applications/Closer.app` in Terminal.

On first launch Closer offers to install the AI agent CLI and walks you through signing in.

## Updates

Closer updates itself. It checks for a new version at startup and every few hours, downloads it in the background, and shows a **Restart** button when it's ready. Restarting waits until running research jobs have finished.

## Development

Requires [Bun](https://bun.sh) and [Rust](https://rustup.rs).

```bash
bun install
bun run tauri:dev     # app with hot reload
bun run tauri:build   # production build
```

Before committing, run `tsc -b`, `bun run lint` and `bun run format:check`, and `cargo test` in `src-tauri`.

## Releasing

```bash
scripts/release.sh 0.3.1
```

This bumps the version, commits, tags and pushes. The tag starts the Release workflow, which builds for macOS and Windows, signs the update bundles, publishes the GitHub release and its `latest.json`. Installed copies pick the update up on their next check.

The workflow needs two repository secrets: `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`. Keep a backup of that key: installed copies only accept updates signed with it.

## License

MIT, see [LICENSE](LICENSE).
