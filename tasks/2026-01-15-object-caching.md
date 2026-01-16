# Gist
Many Slack API objects infrequently change. Let's cache them locally!

Below are some requirements and considerations. Give feedback when
there may be a better option (library, architectural decision,
implementation detail, etc.).

After reading the requirements, suggestions, and considerations,
suggest adding to any of the sections that seems missing. Then once I
approve, propose a plan that takes all of it into account. Explain the
trade-offs, feedback and concrete implementation proposal.

# Requirements
 1. Any [Slack object](https://docs.slack.dev/reference/objects) that
    is received in an API call's response should be cached locally.
 2. For DRY structuring to leverage an existing query language, the
    objects should be stored in a relational database (suggestion:
    Probably the latest SQLite).
 3. Associated with every cached object should be the timestamp when
    it was last refreshed, and whether it has been deleted upstream
    (do not delete messages that were deleted upstream).


# Suggestions
 - SQLite is a good candidate for the relational database because of
   ease of install, portability and existing SQL implementation.
 - plural table names are nice.
 - Use an ORM unless there are good reasons to avoid.

# Considerations
  - naming consistency with the slack api object and field names
