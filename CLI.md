# Clack CLI Interface Philosophy

## Design Principles

### 1. Git-Style Command Structure
Clack follows git CLI conventions with the pattern:
```
clack <command> [<options>] [<args...>]
```

### 2. Simplicity by Default
- The default behavior should be simple and not verbose
- Common use cases shouldn't require many flags
- Less common options are available but not required

### 3. Human-First, Machine-Friendly
- Default output is human-readable and colorized
- Machine-readable formats available via `--format` flag
- Output respects NO_COLOR environment variable

### 4. Intuitive Resource Access
- Plural commands list resources: `clack users`
- Singular commands get specific resources: `clack user <id>`
- Natural language-like flow: `clack messages <channel>`

## Command Reference

### Users

#### List all users
```bash
clack users
```

Lists all users in the workspace with colorized, human-readable output showing:
- Display name
- Real name
- Email (if available)
- Status emoji and text
- User ID (for reference)

**Options:**
- `--format <format>` - Output format: `human` (default), `json`, `yaml`
- `--limit <n>` - Limit number of results (default: all)
- `--include-deleted` - Include deleted/deactivated users

**Examples:**
```bash
# List all users (human-readable, colorized)
clack users

# Export users as JSON
clack users --format json

# Get first 10 users in YAML format
clack users --limit 10 --format yaml
```

#### Get a specific user
```bash
clack user <user_id>
```

Displays detailed information about a single user:
- Display name and real name
- Email address
- Status and custom status text
- Timezone
- Profile picture URLs
- Whether they're a bot, admin, owner, etc.
- Link to their Slack profile

**Arguments:**
- `<user_id>` - Slack user ID (e.g., U1234ABCD)

**Options:**
- `--format <format>` - Output format: `human` (default), `json`, `yaml`

**Examples:**
```bash
# Get user info (human-readable)
clack user U1234ABCD

# Get user info as JSON
clack user U1234ABCD --format json
```

### Messages

#### List messages in a channel
```bash
clack messages <channel>
```

Lists messages from a channel with human-readable output showing:
- Timestamp
- User name
- Message text
- Reactions (emoji and count)
- Thread indicators (if part of a thread)
- Link to message in Slack

**Arguments:**
- `<channel>` - Channel ID (e.g., C1234ABCD) or channel name (e.g., #general)

**Options:**
- `--format <format>` - Output format: `human` (default), `json`, `yaml`
- `--limit <n>` - Number of messages to retrieve (default: 100)
- `--latest <timestamp>` - End of time range (default: now)
- `--oldest <timestamp>` - Start of time range
- `--include-threads` - Include all thread replies inline

**Examples:**
```bash
# Get last 100 messages from a channel
clack messages C1234ABCD

# Get last 50 messages as JSON
clack messages general --limit 50 --format json

# Get messages from a specific time range
clack messages C1234ABCD --oldest 1609459200 --latest 1609545600
```

## Global Options

These options work with any command:

- `--help`, `-h` - Display help information
- `--version`, `-V` - Display version information
- `--no-color` - Disable colorized output
- `--verbose`, `-v` - Enable verbose logging
- `--quiet`, `-q` - Suppress non-essential output

## Authentication

Clack requires a Slack API token to authenticate requests. Set the `SLACK_TOKEN` environment variable:

```bash
export SLACK_TOKEN=xoxb-your-token-here
```

Token types:
- **Bot tokens** (start with `xoxb-`) - Recommended for most use cases
- **User tokens** (start with `xoxp-`) - Can access user-specific data

If `SLACK_TOKEN` is not set, clack will exit with code -1 and display:
```
Error: SLACK_TOKEN environment variable not set

Please set your Slack API token:
  export SLACK_TOKEN=xoxb-your-token-here

To create a token, visit: https://api.slack.com/authentication/token-types
```

## Required Scopes

Your Slack token must have appropriate OAuth scopes:

**For users commands:**
- `users:read` - Required for basic user info
- `users:read.email` - Required to access email addresses

**For messages commands:**
- `channels:history` - For public channel messages
- `groups:history` - For private channel messages
- `im:history` - For direct messages
- `mpim:history` - For group direct messages

**Important:** Note the distinction between `:read` and `:history` scopes:
- `:read` scopes (e.g., `channels:read`) only allow reading metadata like channel names, topics, and member lists
- `:history` scopes are required to read actual message content

If you have `channels:read` but get a "missing scope" error, you need to add `channels:history`.

## Output Formats

### Human Format (Default)
Colorized, formatted output designed for terminal viewing:
- Uses colors to highlight important information
- Aligned columns for easy scanning
- Includes visual separators
- Respects `NO_COLOR` environment variable

### JSON Format
Pretty-printed JSON output:
```bash
clack users --format json
```

### YAML Format
Human-friendly YAML output:
```bash
clack users --format yaml
```

## Error Handling

Clack provides clear error messages:

- **Missing authentication**: Exit code -1
- **Invalid arguments**: Exit code 2 (follows git convention)
- **API errors**: Exit code 1 with descriptive error message
- **Network errors**: Exit code 1 with connection details

All errors are written to stderr, keeping stdout clean for piping.

## Future Commands (Not Yet Implemented)

These commands are planned for future releases:

```bash
clack channels          # List channels
clack channel <id>      # Get channel info
clack search <query>    # Search messages (Phase 6)
```
