# Clack
A Slack API CLI tool

# Goal
Provide a simple command line tool for querying the Slack API with
output that is human readable by default, but support many other
standard formats for ease of machine parsing.

# Usage

``` shell
clack <command> [<options>]
```

## Core commands
List instances of an `object-type` matching `filter`.

``` shell
clack list [<options>] <object-type> [<filter>]
```

``` shell
clack
```

# Implementation Phases
## 1. Determine the CLI usage shape and conventions to follow
Start by using Users and Messages as the first two object types to
support. There should be an CLI.md created that describes the
interface philosophy with examples using our first two supported
objects.

## 2. Scaffolding
Create an empty rust project with a hello-world source file, and a
simple unit test which exercises the hello-world functionality.

Create a Makefile with the following targets:
 - `make clack` - should build the clack binary for the host platform.
 - `make test` - should build and run the unit tests.
 - `make deps` - should install any build dependencies.
 - `make all` - should install deps, build, and test.
 - `make` - should default to `make clack`.

## 3. Implement the functionality described in CLI.md
Now that we have our philosophy and conventions documented, implement
the functionality for our first two object types. Objects should be
retrieved from the Slack API.

If the SLACK_TOKEN env var does not exist, return -1 and output a
useful error message.

## 4. Generate unit tests for all public functions and CLI interfaces
 - Any supported CLI Usage should have unit tests.
 - Any public function should have unit tests.

Any call to the Slack API should be mocked, avoiding any actual
network requests.

## 5. Optimize the output format for human readability
 - Output should be colorized.
 - Output should include: the most useful information contained within
   the object. Give options and ask for clarification here.
 - Objects should include a URL to the slack object when
   applicable. If applicability is not obvious, ask.

## 6. Implement basic message search
### a. The initial implementation of search should support the following:
 - basic text query
 - user(s) - how can we make a user identifier easy for humans to type
   (it doesn't _need_ to be the actual Slack ID of the object)?
   the standard "from", "with", etc. Slack search keywords should be supported.
 - channel - which channel should be searched?
### b. Usage of the `clack search` feature
Propose the Usage interface, then update this section once decided.

### c. Unit tests
Implement unit tests accordingly. Ensure `make test` compiles and tests pass.

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
