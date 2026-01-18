# Clack
A Slack API CLI tool focused on readable, human-first output with JSON/YAML export.

## What it does
- Query Slack for users, channels, messages, files, pins, reactions, and threads.
- Format output for humans by default, with `--format json` or `--format yaml` for automation.
- Cache API responses locally to speed up repeated queries.

## Quick Start
```bash
# Set your Slack API token
export SLACK_TOKEN=xoxb-your-token-here

# List all users
clack users list

# Get a specific user
clack users info U1234ABCD

# Get messages from a channel
clack conversations history C1234ABCD

# Export as JSON or YAML
clack users list --format json
clack conversations history general --format yaml
```

See `CLI.md` for the full command reference and examples.

## Configuration
- `SLACK_TOKEN` (required): Slack bot token with appropriate scopes for the endpoints you call.
- `--refresh-cache`: bypass the cache and query Slack directly.
- `--debug-response`: print raw HTTP responses for debugging.
- `--no-color`: disable colorized output.

## Scopes required
The Slack app needs scopes for the API methods Clack calls. Exact names depend on classic vs granular scopes, but these are the typical minimums:

| Feature | API methods | Scopes (classic) | Scopes (granular) |
| --- | --- | --- | --- |
| Auth bootstrap | `auth.test` | N/A | N/A |
| Users | `users.list`, `users.info`, `users.profile.get` | `users:read` | `users:read` |
| Conversations list/info/members | `conversations.list`, `conversations.info`, `conversations.members` | `channels:read`, `groups:read`, `im:read`, `mpim:read` | `conversations:read` |
| Conversations history/replies | `conversations.history`, `conversations.replies` | `channels:history`, `groups:history`, `im:history`, `mpim:history` | `conversations:history` |
| Chat post | `chat.postMessage` | `chat:write` | `chat:write` |
| Reactions | `reactions.add`, `reactions.remove` | `reactions:write` | `reactions:write` |
| Pins list | `pins.list` | `pins:read` | `pins:read` |
| Pins add/remove | `pins.add`, `pins.remove` | `pins:write` | `pins:write` |
| Files | `files.list`, `files.info` | `files:read` | `files:read` |
| Search | `search.messages`, `search.files`, `search.all` | `search:read` | `search:read` |

Note: Access to private channels requires the app to be a member of the channel.

## Development
Prerequisites:
- Rust stable toolchain.
- On Linux, OpenSSL headers may be required (e.g. `libssl-dev` and `pkg-config`).

Common tasks:
```bash
make build
make test
```

Install (macOS):
```bash
sudo make install
```

Uninstall:
```bash
sudo make uninstall
```

Local run:
```bash
cargo run -- conversations list
```

## Project layout
- `src/api`: Slack API client and endpoint wrappers.
- `src/cli.rs`: CLI definitions and flags.
- `src/cache`: SQLite-backed cache layer and migrations.
- `src/models`: API response/request models.
- `src/output`: Human-readable formatters.
- `migrations`: Diesel migrations for the cache DB.

## Caching
Clack stores cached Slack objects in a SQLite database under the OS cache directory
(`~/.cache/clack/cache.db` on Linux). WAL mode is enabled for write performance.
Use `--refresh-cache` to force live API reads.

## CI and releases
- GitHub Actions runs `make build` and `make test` on every push.
- Releases on `main` use semantic-release to bump versions and create GitHub Releases.
- Release assets are built per-platform and attached to the GitHub Release.

## Reference
- Slack API docs: https://docs.slack.dev/reference/methods
