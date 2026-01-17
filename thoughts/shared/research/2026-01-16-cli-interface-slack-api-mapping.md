---
date: 2026-01-17T01:58:34+0000
researcher: Claude
git_commit: 278d546496fd8cb05c4c96532a5958dc70c32dac
branch: main
repository: clack
topic: "CLI Interface Restructuring to Match Slack API Method Naming"
tags: [research, codebase, cli, slack-api, refactoring]
status: complete
last_updated: 2026-01-16
last_updated_by: Claude
---

# Research: CLI Interface Restructuring to Match Slack API Method Naming

**Date**: 2026-01-17T01:58:34+0000
**Researcher**: Claude
**Git Commit**: 278d546496fd8cb05c4c96532a5958dc70c32dac
**Branch**: main
**Repository**: clack

## Research Question

How should the Clack CLI interface be restructured to map closely to the Slack API method naming convention? The goal is to transform the current command structure to follow the pattern:

```
Slack API: [<context-type>. ...]<object>.<action>
Clack CLI: clack [<context-type> ...] <object> <action> [<param> ...]
```

## Summary

The Clack codebase currently implements **10 distinct Slack API methods** across 5 API domains (auth, users, conversations, search). The current CLI uses a simplified naming convention (plural/singular pattern) that differs significantly from the Slack API naming scheme.

**Key Findings:**
- Current CLI uses 7 primary commands with 4 search subcommands
- Framework: clap v4.5 with derive macros (highly structured, easy to refactor)
- All API calls route through a centralized `SlackClient` in `src/api/client.rs`
- Parameters are passed as query tuples to match Slack API query parameters
- **10 Slack API methods** are currently implemented and would need CLI restructuring

## Detailed Findings

### Current CLI Command Structure

**Location**: `src/cli.rs:3-86`

The current CLI follows a Git-style command pattern:

```
clack <command> [<options>] [<args...>]
```

**Global Options** (apply to all commands):
- `--format <format>` - Output format: human/json/yaml (default: "human")
- `--no-color` - Disable colorized output
- `--verbose` / `-v` - Enable API logging

### Mapping: Current CLI Commands → Slack API Methods

| Current CLI Command | Slack API Method | Current Implementation |
|---------------------|------------------|------------------------|
| `clack users [--limit] [--include-deleted]` | `users.list` | `src/api/users.rs:17` |
| `clack user <user_id>` | `users.info` | `src/api/users.rs:73` |
| `clack channels [--include-archived] [--limit]` | `conversations.list` | `src/api/channels.rs:61` |
| `clack messages <channel> [--limit] [--latest] [--oldest]` | `conversations.history` | `src/api/messages.rs:29` |
| `clack thread <channel> <message_ts>` | `conversations.replies` | `src/api/messages.rs:68` |
| `clack search messages <query> [options]` | `search.messages` | `src/api/search.rs:16` |
| `clack search files <query> [options]` | `search.files` | `src/api/search.rs:39` |
| `clack search all <query> [options]` | `search.all` | `src/api/search.rs:62` |
| `clack search channels <query>` | *(local filtering, no direct API)* | `src/api/channels.rs:226` |
| `clack auth test` | `auth.test` | `src/api/auth.rs:7` |

### Proposed CLI Command Structure (Matching Slack API)

Based on the Slack API naming convention, here's the proposed transformation:

| Slack API Method | Proposed Clack Command | Parameters |
|------------------|------------------------|------------|
| `auth.test` | `clack auth test` | *(no change - already matches)* |
| `users.list` | `clack users list` | `[--limit <n>] [--include-deleted]` |
| `users.info` | `clack users info <user_id>` | `<user_id>` |
| `users.profile.get` | `clack users profile get <user_id>` | `<user_id>` *(not currently implemented)* |
| `conversations.list` | `clack conversations list` | `[--limit <n>] [--include-archived]` |
| `conversations.info` | `clack conversations info <channel_id>` | `<channel_id>` *(currently embedded in resolution)* |
| `conversations.history` | `clack conversations history <channel>` | `<channel> [--limit <n>] [--latest <ts>] [--oldest <ts>]` |
| `conversations.replies` | `clack conversations replies <channel> <message_ts>` | `<channel> <message_ts>` |
| `search.messages` | `clack search messages <query>` | `<query> [--from <user>] [--channel <ch>] [--after <date>] [--before <date>] [--limit <n>]` *(no change)* |
| `search.files` | `clack search files <query>` | `<query> [--from <user>] [--channel <ch>] [--after <date>] [--before <date>] [--limit <n>]` *(no change)* |
| `search.all` | `clack search all <query>` | `<query> [--channel <ch>] [--limit <n>]` *(no change)* |

### Key Structural Changes Required

**1. Users Domain** (`src/cli.rs:26-40`)

*Current:*
```rust
Commands::Users { limit, include_deleted }  // Plural = list
Commands::User { user_id }                  // Singular = get
```

*Proposed:*
```rust
Commands::Users(UsersCommands)

enum UsersCommands {
    List { limit, include_deleted },
    Info { user_id },
}
```

**2. Conversations Domain** (`src/cli.rs:42-75`)

*Current:*
```rust
Commands::Messages { channel, limit, latest, oldest }
Commands::Thread { channel, message_ts }
Commands::Channels { include_archived, limit }
```

*Proposed:*
```rust
Commands::Conversations(ConversationsCommands)

enum ConversationsCommands {
    List { limit, include_archived },
    Info { channel_id },
    History { channel, limit, latest, oldest },
    Replies { channel, message_ts },
}
```

**3. Search Domain** - *Already matches!*

The search commands already follow the pattern:
```rust
Commands::Search(SearchType)

enum SearchType {
    Messages { query, ... },
    Files { query, ... },
    All { query, ... },
    Channels { query, ... },
}
```

**4. Auth Domain** - *Already matches!*

```rust
Commands::Auth(AuthType)

enum AuthType {
    Test,
}
```

## Code References

### CLI Definition & Parsing
- `src/cli.rs:3-86` - Main CLI structure with clap derive macros
- `src/cli.rs:170-476` - 19 CLI parsing tests (will need updates)
- `src/main.rs:22-247` - Command routing and execution (will need refactoring)

### API Implementations
- `src/api/auth.rs:7` - auth.test implementation
- `src/api/users.rs:17` - users.list implementation
- `src/api/users.rs:73` - users.info implementation
- `src/api/channels.rs:61` - conversations.list implementation (with pagination)
- `src/api/channels.rs:207` - conversations.info implementation
- `src/api/messages.rs:29` - conversations.history implementation
- `src/api/messages.rs:68` - conversations.replies implementation
- `src/api/search.rs:16` - search.messages implementation
- `src/api/search.rs:39` - search.files implementation
- `src/api/search.rs:62` - search.all implementation

### HTTP Client Layer
- `src/api/client.rs:68-74` - Generic get method for Slack API calls
- `src/api/client.rs:76-210` - Retry logic with rate limiting

### Parameter Handling
- `src/api/search.rs:75-101` - Query builder for search filters
- `src/api/channels.rs:6-105` - Channel ID resolution and validation

## Architecture Insights

### Pattern Consistency

**Current Design Philosophy** (from `CLI.md:1-24`):
1. Git-style command structure
2. Plural for lists, singular for get operations
3. Simplicity by default (minimal flags for common cases)
4. Human-first, machine-friendly output

**Slack API Pattern**:
The Slack API uses a three-level hierarchy:
```
[context.]object.action

Examples:
- users.list (no context, just object.action)
- users.profile.get (user context + profile object + get action)
- conversations.history (no context, conversations object + history action)
```

### Refactoring Implications

**Low Risk Areas**:
1. **Search commands** - Already match the pattern, no changes needed
2. **Auth commands** - Already match the pattern, no changes needed
3. **API layer** - No changes needed, already well-structured

**Medium Risk Areas**:
1. **CLI enum structure** - Requires adding subcommand layers for users and conversations
2. **Main routing logic** - Pattern matching in `main.rs` needs updating
3. **CLI tests** - All 19 tests need syntax updates to match new command structure

**High Risk Areas**:
1. **Documentation** - `CLI.md` and `README.md` need comprehensive updates
2. **User migration** - Breaking change for existing users/scripts
3. **Backwards compatibility** - Consider alias support for transition period

### Implementation Strategy

**Recommended Approach**:
1. Add new command structure alongside existing commands (with deprecation warnings)
2. Update API layer to support both patterns temporarily
3. Provide migration guide for users
4. Remove old commands in a major version bump

**Clap Features to Leverage**:
- `#[command(alias = "old-command")]` - Support old command names
- `#[arg(hide = true)]` - Hide deprecated options
- Custom help text for migration guidance

### Testing Strategy

**Test Coverage to Maintain**:
- All 19 CLI parsing tests need updates (`src/cli.rs:170-476`)
- All API integration tests remain unchanged (search, users, channels, messages, auth modules)
- Add new tests for subcommand parsing (users list, users info, conversations list, etc.)

## Open Questions

1. **Backwards Compatibility**: Should we support both old and new command syntax during a transition period?
2. **Conversations vs Channels**: The Slack API uses "conversations" but users often think in terms of "channels". Should we support both as aliases?
3. **Subcommand Defaults**: For `clack users`, should it default to `users list` or require explicit `users list`?
4. **Parameter Naming**: Should parameters match Slack API exactly (e.g., `--ts` instead of `--message-ts`)?
5. **Additional API Methods**: Should we implement currently missing methods like `users.profile.get`, `conversations.info` (explicit command), etc.?

## Related Research

- Current CLI documentation: `CLI.md`
- Task specification: `tasks/2026-01-16-cli-interface.md`
- Slack API reference: https://docs.slack.dev/reference/methods/

## Next Steps

1. Update `tasks/2026-01-16-cli-interface.md` with detailed proposed command structure
2. Create detailed examples for each proposed command transformation
3. Decide on backwards compatibility strategy
4. Plan implementation phases
5. Update CLI tests to match new structure
6. Update documentation
