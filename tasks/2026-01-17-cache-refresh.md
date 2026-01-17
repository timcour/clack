# Goal
Allow for explicit cache refreshes.

# Interface
Add a global optional arg, `--refresh-cache` which will skip the cache
for any API request.

# Requirements
When `--refresh-cach` is specified, the Slack API should be queried
directly, for all requests during the current command lifecycle.  Upon
receiving successful API responses, update the corresponding cache
with the response data.

# Success criteria
 - Unit tests must be added for the new `--refresh-cache` behavior.
 - All new and existing unit tests must pass.
 - Since `--refresh-cache` is a global option, it should be accepted
   at any point in the command, e.g. The following are both valid

```
  clack --refresh-cache conversations info CID
  clack conversations info --refresh-cache CID
```
