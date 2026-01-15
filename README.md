# Clack
A Slack API CLI tool

# Goal
Provide a simple command line tool for querying the Slack API with
output that is human readable by default, but support many other
standard formats for ease of machine parsing.

# Usage

See [CLI.md](./CLI.md) for complete interface documentation and examples.

## Quick Start

```bash
# Set your Slack API token
export SLACK_TOKEN=xoxb-your-token-here

# List all users
clack users

# Get a specific user
clack user U1234ABCD

# Get messages from a channel
clack messages C1234ABCD

# Export as JSON or YAML
clack users --format json
clack messages general --format yaml
```

# Interface Design
 - Should follow git cli conventions, e.g.
     clack <command> [<options>] [<args...>]`.
 - The default should be simple, not verbose. Specifying a --bunch
   --of --options is not fun for humans.
 - But still allow for less common --options since they _always_ end
   up being useful.

# Thoughts
## Object modeling
This codebase should mirror the relationships defined by the Slack
API, e.g. a user has many messages, a channel has many users and many
messages, etc.

## Testing
All public function and CLI interface should have a corresponding unit test. When
running unit tests, by default, no external API calls should be
made. Any calls to the Slack API should be mocked. If there is an
existing "mock library" that would fit this use case well, recommend
it.

## Forward Looking
Caching Slack objects might be useful in the future, so let's plan to
integrate an ORM backed by a SQLite database at some point.

# Reference
## Slack API Docs
https://docs.slack.dev/reference/methods
