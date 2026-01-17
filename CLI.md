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

### 4. API-Aligned Structure
- Commands mirror Slack API naming: `users.list` → `clack users list`
- Hierarchical subcommands group related operations
- Consistent pattern across all resource types

## Command Reference

### Users

#### List all users
```bash
clack users list
```

Lists all users in the workspace with colorized, human-readable output showing:
- Display name
- Real name
- Email (if available)
- Status emoji and text
- User ID (for reference)

**Options:**
- `--format <format>` - Output format: `human` (default), `json`, `yaml`
- `--limit <n>` - Limit number of results (default: 200)
- `--include-deleted` - Include deleted/deactivated users

**Examples:**
```bash
# List all users (human-readable, colorized)
clack users list

# Export users as JSON
clack users list --format json

# Get first 10 users in YAML format
clack users list --limit 10 --format yaml
```

#### Get a specific user
```bash
clack users info <user_id>
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
clack users info U1234ABCD

# Get user info as JSON
clack users info U1234ABCD --format json
```

### Conversations

#### List messages in a channel
```bash
clack conversations history <channel>
```

Lists messages from a channel with human-readable output showing:
- Timestamp
- User name
- Message text
- Reactions (emoji and count)
- Thread indicators (if part of a thread)
- Link to message in Slack

**Arguments:**
- `<channel>` - Channel ID (C1234ABCD), name with # (#general), or name without # (general)

**Options:**
- `--format <format>` - Output format: `human` (default), `json`, `yaml`
- `--limit <n>` - Number of messages to retrieve (default: 200)
- `--latest <timestamp>` - End of time range (default: now)
- `--oldest <timestamp>` - Start of time range

**Examples:**
```bash
# Get last 100 messages from a channel by ID
clack conversations history C1234ABCD

# Get messages using channel name
clack conversations history general

# Get messages using # prefix
clack conversations history #general

# Get last 50 messages as JSON
clack conversations history general --limit 50 --format json

# Get messages from a specific time range
clack conversations history C1234ABCD --oldest 1609459200 --latest 1609545600
```

**Performance Note:**
When using channel names (like `general` or `#firmware-team`) instead of channel IDs, the tool must first resolve the name to an ID by searching through all channels. This adds extra API calls before fetching messages:
- Using channel ID (`C1234ABCD`): 1 API call (instant)
- Using channel name (`firmware-team`): Multiple API calls to find the channel, then 1 call for messages

For better performance, especially in large workspaces:
1. Use channel IDs directly when known
2. Use `clack conversations list --format json` to get all channel IDs once and cache them
3. The tool stops searching as soon as it finds the channel (optimized)

The `--limit` parameter only affects the number of messages retrieved, not the channel lookup.

#### Get a conversation thread
```bash
clack conversations replies <channel> <message_ts>
```

Retrieves a conversation thread including the root message and all replies. Threads in Slack are conversations that branch off from a message.

**Arguments:**
- `<channel>` - Channel ID (C1234ABCD), name with # (#general), or name without # (general)
- `<message_ts>` - Message timestamp/ID (e.g., `1234567890.123456`)

**Options:**
- `--format <format>` - Output format: `human` (default), `json`, `yaml`

**Examples:**
```bash
# Get a thread using channel ID
clack conversations replies C1234ABCD 1234567890.123456

# Get a thread using channel name
clack conversations replies general 1234567890.123456

# Get a thread using # prefix
clack conversations replies #general 1234567890.123456

# Export thread as JSON
clack conversations replies C1234ABCD 1234567890.123456 --format json

# Get thread with colorization disabled
clack conversations replies general 1234567890.123456 --no-color
```

**Finding Message Timestamps:**
When using the `conversations history` command, each message displays its timestamp and a URL. You can use this timestamp with the `conversations replies` command:
```bash
# First, get messages from a channel
clack conversations history general

# Then use a message timestamp to get its thread
clack conversations replies general 1234567890.123456
```

**Performance Note:**
Like the `conversations history` command, using channel names requires resolving the name to an ID first. For better performance in large workspaces, use channel IDs directly (e.g., `clack conversations replies C1234ABCD 1234567890.123456`).

**Required Scopes:**
- `channels:history` - For threads in public channels
- `groups:history` - For threads in private channels
- `im:history` - For threads in direct messages
- `mpim:history` - For threads in group direct messages

## Global Options

These options work with any command:

- `--help`, `-h` - Display help information
- `--version`, `-V` - Display version information
- `--no-color` - Disable colorized output
- `--verbose`, `-v` - Enable verbose API logging (shows request URLs, parameters, response status, duration, and size)
- `--debug-response` - Show raw HTTP response bodies for debugging API response parsing issues
- `--format <format>` - Output format: `human` (default), `json`, `yaml`

### Verbose Mode

When `--verbose` is enabled, every API request will log detailed information to stderr:

**Request logging:**
```
→ GET https://slack.com/api/users.list
  Query: limit=200
```

**Response logging:**
```
← 200 (245ms, 15234 bytes)
```

This is useful for:
- Debugging API issues
- Understanding which API calls are being made
- Monitoring rate limits and performance
- Troubleshooting authentication or scope problems

### Debug Response Mode

When `--debug-response` is enabled, the raw HTTP response body from each API call will be printed to stderr. This is extremely useful when:
- Troubleshooting deserialization errors
- Understanding the exact structure of API responses
- Debugging unexpected data formats
- Reporting bugs or issues with API response parsing

**Example:**
```bash
# Show raw response to debug parsing errors
clack conversations history C123456 --debug-response
```

The output will show:
```
═══════════════════════════════════════════════════════════════
DEBUG: Response body from conversations.history:
═══════════════════════════════════════════════════════════════
{"ok":true,"messages":[...],"has_more":false}
═══════════════════════════════════════════════════════════════
```

**Example:**
```bash
# Normal output
clack users list --limit 5

# With verbose logging
clack users list --limit 5 --verbose
```

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
clack users list --format json
```

### YAML Format
Human-friendly YAML output:
```bash
clack users list --format yaml
```

## Error Handling

Clack provides clear error messages:

- **Missing authentication**: Exit code -1
- **Invalid arguments**: Exit code 2 (follows git convention)
- **API errors**: Exit code 1 with descriptive error message
- **Network errors**: Exit code 1 with connection details

All errors are written to stderr, keeping stdout clean for piping.

#### List all channels
```bash
clack conversations list
```

Lists all channels that the bot has access to, including both public and private channels (if the bot is a member).

**Options:**
- `--include-archived` - Include archived channels in the list
- `--limit <n>` - Maximum number of channels per page (default: 200, max: 1000)
- `--format <format>` - Output format: `human` (default), `json`, `yaml`

**Examples:**
```bash
# List all active channels
clack conversations list

# Include archived channels
clack conversations list --include-archived

# Export as JSON
clack conversations list --format json

# Find a specific channel
clack conversations list | grep firmware
```

#### Get channel information
```bash
clack conversations info <channel>
```

Gets detailed information about a specific channel.

**Arguments:**
- `<channel>` - Channel ID (C1234ABCD), name with # (#general), or name without # (general)

**Options:**
- `--format <format>` - Output format: `human` (default), `json`, `yaml`

**Examples:**
```bash
# Get channel info by ID
clack conversations info C1234ABCD

# Get channel info by name
clack conversations info general

# Export as JSON
clack conversations info #engineering --format json
```

**Output includes:**
- Channel name with # prefix
- Channel ID
- Privacy status (🔒 for private channels)
- Archived status (📦 for archived)
- Topic and member count

**Pagination:**
This command automatically fetches ALL channels using pagination, so you'll see every channel the bot has access to, even if you have hundreds of channels.

**Rate Limiting:**
If Slack's rate limits are hit, the tool will automatically retry with exponential backoff (up to 3 retries). You'll see a message like:
```
Rate limited. Waiting 1 second(s) before retry 1/3...
```

For large workspaces with many channels, the initial channel name resolution may take a few seconds to paginate through all channels.

**Required Scopes:**
- `channels:read` - For public channels
- `groups:read` - For private channels

### Search

The `search` command allows you to search through Slack messages, files, or both. Searches use Slack's search modifiers and support various filters.

#### Search messages
```bash
clack search messages <query>
```

Searches for messages matching the query across all channels the bot has access to.

**Arguments:**
- `<query>` - Search query text

**Options:**
- `--from <user>` - Filter by message author (user ID, @username, or display name)
- `--channel <channel>` - Filter by channel (channel ID, #name, or name)
- `--after <date>` - Filter messages after date (YYYY-MM-DD or Unix timestamp)
- `--before <date>` - Filter messages before date (YYYY-MM-DD or Unix timestamp)
- `--limit <n>` - Maximum number of results (default: 200)
- `--format <format>` - Output format: `human` (default), `json`, `yaml`

**Examples:**
```bash
# Simple text search
clack search messages "deployment failed"

# Search for messages from a specific user
clack search messages "approved" --from alice

# Search in a specific channel
clack search messages "standup notes" --channel engineering

# Search with date range
clack search messages "budget" --after 2024-01-01 --before 2024-12-31

# Combine multiple filters
clack search messages "release" --from bob --channel releases --after 2024-06-01

# Export results as JSON
clack search messages "error" --format json
```

#### Search files
```bash
clack search files <query>
```

Searches for files matching the query across all channels the bot has access to.

**Arguments:**
- `<query>` - Search query text (can include wildcards like `*.pdf`)

**Options:**
- `--from <user>` - Filter by file uploader (user ID, @username, or display name)
- `--channel <channel>` - Filter by channel where file was shared (channel ID, #name, or name)
- `--after <date>` - Filter files after date (YYYY-MM-DD or Unix timestamp)
- `--before <date>` - Filter files before date (YYYY-MM-DD or Unix timestamp)
- `--limit <n>` - Maximum number of results (default: 200)
- `--format <format>` - Output format: `human` (default), `json`, `yaml`

**Examples:**
```bash
# Search for PDF files
clack search files "*.pdf"

# Search for files from a specific user
clack search files "presentation" --from alice

# Search for files in a specific channel
clack search files "diagram" --channel engineering

# Search with date range
clack search files "report" --after 2024-01-01

# Combine filters
clack search files "*.xlsx" --from bob --channel finance --after 2024-06-01
```

#### Search all (messages and files)
```bash
clack search all <query>
```

Searches both messages and files simultaneously.

**Arguments:**
- `<query>` - Search query text

**Options:**
- `--channel <channel>` - Filter by channel (channel ID, #name, or name)
- `--limit <n>` - Maximum number of results (default: 200)
- `--format <format>` - Output format: `human` (default), `json`, `yaml`

**Examples:**
```bash
# Search everything
clack search all "quarterly review"

# Search in specific channel
clack search all "budget 2024" --channel finance

# Export combined results
clack search all "project alpha" --format json
```

#### Search channels by name
```bash
clack search channels <query>
```

Searches for channels by name (case-insensitive substring matching). This is useful for quickly finding channels when you remember part of the name but not the full name.

**Arguments:**
- `<query>` - Channel name substring to search for (case-insensitive)

**Options:**
- `--include-archived` - Include archived channels in search results
- `--format <format>` - Output format: `human` (default), `json`, `yaml`

**Examples:**
```bash
# Search for channels with "eng" in the name
clack search channels eng
# Results: #engineering, #engineering-ops, #backend-eng, etc.

# Search for channels with "dab" in the name
clack search channels dab

# Include archived channels
clack search channels old-project --include-archived

# Export results as JSON
clack search channels team --format json
```

**Output includes:**
- Channel name with # prefix
- Channel ID
- Privacy status (🔒 for private channels)
- Archived status (📦 for archived)
- Topic and purpose
- Member count

**Note:** This command searches locally through all channels the bot has access to. It performs case-insensitive substring matching, so searching for "eng" will match "engineering", "backend-eng", "EngOps", etc.

**Search Query Syntax:**

The search commands use Slack's search modifier syntax. Filters are automatically combined with your query:
- `--from alice` becomes `from:alice` in the search query
- `--channel engineering` becomes `in:engineering`
- `--after 2024-01-01` becomes `after:2024-01-01`
- `--before 2024-12-31` becomes `before:2024-12-31`

You can also use Slack's native search modifiers directly in your query string:
```bash
clack search messages "deploy from:alice in:engineering after:2024-01-01"
```

**Required Scopes:**
- `search:read` - Required for all search commands

## Command Summary

```
clack users list              # List all users
clack users info <user_id>    # Get user information

clack conversations list                        # List all channels
clack conversations info <channel>              # Get channel information
clack conversations history <channel>           # Get message history
clack conversations replies <channel> <msg_ts>  # Get thread replies

clack search messages <query>  # Search messages
clack search files <query>     # Search files
clack search all <query>       # Search messages and files
clack search channels <query>  # Search channels by name

clack auth test               # Test authentication
```
