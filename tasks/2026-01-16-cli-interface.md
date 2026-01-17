# Goal
Rework the current CLI interface to map closely to the corresponding
[API methods documented online
here](https://docs.slack.dev/reference/methods/). The Slack API
methods seem to conform to:

```
[<context-type>. ...]<object>.<action>
```

Thus, Clack should conform to:

```
clack [<context-type> ...] <object> <action> [<param> ...]
```

## Example:
### Get User API

```
users.profile.get
```

With Clack:

```
clack users profile get
```

### Get messages in channel

```
conversations.history?latest=TIMESTAMP&limit=15
```

With Clack:
```
conversations history --latest TIMESTAMP --limit 15
```

Each Clack command should support the same parameters as are accepted
by the corresponding Slack API.

# Steps
After codebase research is done, update this document with the newly
proposed Clack command structure convention in detail. For each API,
the corresponding Clack command usage should be defined along with two
example uses.

Let's start with only the Slack API requests that are currently
implemented in this codebase.

---

# Research Complete - Proposed Command Structure

## Summary

The Clack codebase currently implements **10 distinct Slack API methods**. The research findings are documented in `thoughts/shared/research/2026-01-16-cli-interface-slack-api-mapping.md`.

**Current Implementation**: 7 primary commands + 4 search subcommands
**Proposed Structure**: Align with Slack API naming: `clack <object> <action> [params]`

---

## Proposed Commands (for currently implemented APIs)

### 1. auth.test

**Slack API**: `auth.test`
**Current**: `clack auth test` ✓ (already matches)
**Proposed**: `clack auth test`

**Usage**:
```
clack auth test
```

**Examples**:
```bash
# Test authentication and display workspace info
clack auth test

# Test authentication with JSON output
clack auth test --format json
```

---

### 2. users.list

**Slack API**: `users.list`
**Current**: `clack users [--limit N] [--include-deleted]`
**Proposed**: `clack users list [--limit N] [--include-deleted]`

**Usage**:
```
clack users list [OPTIONS]

Options:
  --limit <n>          Maximum number of users to retrieve (default: 200)
  --include-deleted    Include deleted/deactivated users
```

**Examples**:
```bash
# List all active users (default limit: 200)
clack users list

# List first 50 users including deleted accounts
clack users list --limit 50 --include-deleted
```

---

### 3. users.info

**Slack API**: `users.info`
**Current**: `clack user <USER_ID>`
**Proposed**: `clack users info <USER_ID>`

**Usage**:
```
clack users info <USER_ID>

Arguments:
  <USER_ID>    User ID (e.g., U1234ABCD)
```

**Examples**:
```bash
# Get information for a specific user
clack users info U1234ABCD

# Get user info as JSON
clack users info U1234ABCD --format json
```

---

### 4. conversations.list

**Slack API**: `conversations.list`
**Current**: `clack channels [--include-archived] [--limit N]`
**Proposed**: `clack conversations list [--include-archived] [--limit N]`

**Usage**:
```
clack conversations list [OPTIONS]

Options:
  --include-archived    Include archived channels
  --limit <n>           Maximum number of channels per page (default: 200, max: 1000)
```

**Examples**:
```bash
# List all active channels
clack conversations list

# List all channels including archived ones
clack conversations list --include-archived --limit 500
```

**Alternative**: Consider `clack channels list` as an alias since "channels" is more familiar to users.

---

### 5. conversations.info

**Slack API**: `conversations.info`
**Current**: *(embedded in channel resolution, not a standalone command)*
**Proposed**: `clack conversations info <CHANNEL_ID>`

**Usage**:
```
clack conversations info <CHANNEL_ID>

Arguments:
  <CHANNEL_ID>    Channel ID (e.g., C1234ABCD) or channel name (e.g., #general)
```

**Examples**:
```bash
# Get information about a specific channel by ID
clack conversations info C1234ABCD

# Get channel info by name
clack conversations info general
```

**Note**: This would be a new standalone command. Currently, conversations.info is only called internally during channel resolution.

---

### 6. conversations.history

**Slack API**: `conversations.history`
**Current**: `clack messages <CHANNEL> [--limit N] [--latest TS] [--oldest TS]`
**Proposed**: `clack conversations history <CHANNEL> [--limit N] [--latest TS] [--oldest TS]`

**Usage**:
```
clack conversations history <CHANNEL> [OPTIONS]

Arguments:
  <CHANNEL>         Channel ID or name (e.g., C1234ABCD, #general, or general)

Options:
  --limit <n>       Number of messages to retrieve (default: 200)
  --latest <ts>     End of time range (Unix timestamp)
  --oldest <ts>     Start of time range (Unix timestamp)
```

**Examples**:
```bash
# Get recent messages from a channel
clack conversations history general --limit 50

# Get messages from a specific time range
clack conversations history C1234ABCD --latest 1705536000 --oldest 1705449600
```

**Alternative**: Consider `clack messages history` as a shorter alias.

---

### 7. conversations.replies

**Slack API**: `conversations.replies`
**Current**: `clack thread <CHANNEL> <MESSAGE_TS>`
**Proposed**: `clack conversations replies <CHANNEL> <MESSAGE_TS>`

**Usage**:
```
clack conversations replies <CHANNEL> <MESSAGE_TS>

Arguments:
  <CHANNEL>       Channel ID or name
  <MESSAGE_TS>    Message timestamp (e.g., 1234567890.123456)
```

**Examples**:
```bash
# Get all replies in a thread
clack conversations replies general 1705536123.456789

# Get thread replies with JSON output
clack conversations replies C1234ABCD 1705536123.456789 --format json
```

**Alternative**: Consider `clack thread replies` as a shorter alias.

---

### 8. search.messages

**Slack API**: `search.messages`
**Current**: `clack search messages <QUERY> [OPTIONS]` ✓ (already matches)
**Proposed**: `clack search messages <QUERY> [OPTIONS]`

**Usage**:
```
clack search messages <QUERY> [OPTIONS]

Arguments:
  <QUERY>           Search query text

Options:
  --from <user>     Filter by user (user ID, @username, or display name)
  --channel <ch>    Filter by channel (channel ID or name)
  --in <ch>         Alias for --channel
  --after <date>    Messages after date (YYYY-MM-DD or Unix timestamp)
  --before <date>   Messages before date (YYYY-MM-DD or Unix timestamp)
  --limit <n>       Maximum results (default: 200)
```

**Examples**:
```bash
# Search for messages containing "deploy"
clack search messages "deploy"

# Search for messages from a specific user in a channel
clack search messages "production incident" --from alice --channel engineering --after 2024-01-01
```

---

### 9. search.files

**Slack API**: `search.files`
**Current**: `clack search files <QUERY> [OPTIONS]` ✓ (already matches)
**Proposed**: `clack search files <QUERY> [OPTIONS]`

**Usage**:
```
clack search files <QUERY> [OPTIONS]

Arguments:
  <QUERY>           File search query (supports wildcards like *.pdf)

Options:
  --from <user>     Filter by uploader
  --channel <ch>    Filter by channel
  --in <ch>         Alias for --channel
  --after <date>    Files uploaded after date
  --before <date>   Files uploaded before date
  --limit <n>       Maximum results (default: 200)
```

**Examples**:
```bash
# Search for PDF files
clack search files "*.pdf"

# Search for files uploaded by a user in a specific timeframe
clack search files "presentation" --from bob --channel marketing --after 2024-01-01 --before 2024-01-31
```

---

### 10. search.all

**Slack API**: `search.all`
**Current**: `clack search all <QUERY> [OPTIONS]` ✓ (already matches)
**Proposed**: `clack search all <QUERY> [OPTIONS]`

**Usage**:
```
clack search all <QUERY> [OPTIONS]

Arguments:
  <QUERY>           Search query text

Options:
  --channel <ch>    Filter by channel
  --in <ch>         Alias for --channel
  --limit <n>       Maximum results (default: 200)
```

**Examples**:
```bash
# Search for both messages and files
clack search all "quarterly report"

# Search in a specific channel
clack search all "budget 2024" --channel finance
```

---

## Global Options

All commands support these global options:

```
--format <format>     Output format: human (default), json, or yaml
--no-color            Disable colorized output
--verbose, -v         Enable verbose API logging
```

---

## Migration Path

### Breaking Changes

The following commands would need to change:

| Old Command | New Command | Impact |
|-------------|-------------|--------|
| `clack users` | `clack users list` | HIGH - Very common command |
| `clack user <id>` | `clack users info <id>` | MEDIUM - Less common |
| `clack channels` | `clack conversations list` | HIGH - Common command |
| `clack messages <ch>` | `clack conversations history <ch>` | HIGH - Very common |
| `clack thread <ch> <ts>` | `clack conversations replies <ch> <ts>` | MEDIUM - Less common |

### Backwards Compatibility Strategy

**Option 1: Alias Support (Recommended)**
- Keep old commands as aliases during transition period
- Add deprecation warnings to old commands
- Remove in next major version (v2.0)

```rust
#[command(alias = "users")]
List { ... }

#[command(alias = "user")]
Info { ... }
```

**Option 2: Dual Support**
- Support both syntaxes indefinitely
- Use help text to promote new syntax
- More maintenance burden long-term

**Option 3: Hard Break**
- Remove old commands immediately
- Provide migration guide
- Fastest to implement, hardest for users

---

## Implementation Notes

### Code Changes Required

1. **CLI Structure** (`src/cli.rs`):
   - Wrap `Users` and `User` into `Users(UsersCommands)` enum
   - Wrap `Messages`, `Thread`, `Channels` into `Conversations(ConversationsCommands)` enum
   - Add `Info` subcommand to `ConversationsCommands`

2. **Main Router** (`src/main.rs`):
   - Update pattern matching for new nested enum structure
   - Route `users list` → `api::users::list_users()`
   - Route `users info` → `api::users::get_user()`
   - Route `conversations list` → `api::channels::list_channels()`
   - Route `conversations history` → `api::messages::list_messages()`
   - Route `conversations replies` → `api::messages::get_thread()`

3. **Tests** (`src/cli.rs:170-476`):
   - Update all 19 CLI parsing tests
   - Add tests for new subcommand structure

4. **API Layer**:
   - No changes needed (already well-structured)

5. **Documentation**:
   - Update `CLI.md` with new command structure
   - Update `README.md` examples
   - Add migration guide

---

## Open Questions for Discussion

1. **Default subcommand**: Should `clack users` default to `clack users list`?
2. **Alias names**: Should we support `clack channels` as alias for `clack conversations`?
3. **Deprecation timeline**: How long should we support old syntax?
4. **New commands**: Should we add explicit `conversations info` command?
5. **Parameter names**: Should we match Slack API exactly (e.g., `--ts` vs `--message-ts`)?
