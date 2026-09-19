# Behavior contract for the first milestone: attach to a running opencode
# server and hold a prompt session.
#
# These scenarios are an UNAUTOMATED CONTRACT until the cucumber-rs bindings in
# tests/ run against a pinned `opencode serve` and have been observed failing
# against the unmodified skeleton. `just contracts` runs the binding.
@contract
Feature: Attach to an opencode server and interact with a session

  The client is a second, native front-end for the same server the upstream TUI
  talks to. It reads state over the v1 REST surface and stays current over the
  `/global/event` SSE stream, so a user can watch and drive a session without
  the Bun runtime.

  Background:
    Given an opencode server is listening at a known base URL
    And the server exposes the v1 REST and SSE surface

  Scenario: Connecting makes the client ready
    Given the server is reachable
    When the user launches the client against the base URL
    Then the client is connected
    And its state status moves from Loading to Complete
    And the event stream is subscribed
    And the client has not spawned a server process

  Scenario: An unreachable server is reported, not hidden
    Given no server is listening at the base URL
    When the user launches the client against the base URL
    Then the client reports a connection error naming the base URL
    And it exits without leaving the terminal in raw mode

  Scenario: The session list is read from the server
    Given the server has at least one session
    When the client finishes its initial load
    Then the home screen lists the server's sessions
    And each entry shows the session title and identifier

  Scenario: Opening a session renders its transcript
    Given a session exists with at least one user and one assistant message
    When the user opens that session
    Then the session screen lists the messages in server order
    And assistant parts are rendered by their part type
    And tool parts show the tool name and its current status

  Scenario: Submitting a prompt streams the assistant reply
    Given the user is on an open session
    When the user submits the prompt "reply with the single word pong"
    Then the prompt is posted to that session
    And the assistant reply arrives over the event stream
    And the rendered transcript updates without a manual refresh

  Scenario: A permission request is surfaced but not answered
    Given a session requests permission for a tool
    When the permission event arrives
    Then the client renders the request and its options
    And the client does not reply on the permission endpoint

  Scenario: A dropped event stream degrades and recovers
    Given the client has completed its initial load
    When the event stream disconnects
    Then the state status becomes Partial
    And the client retries with a bounded exponential backoff
    When the server becomes reachable again
    Then the event stream is re-subscribed
    And the state status returns to Complete

  Scenario: The directory header is carried and encoded
    Given the user passes a working directory that contains spaces
    When the client requests server state
    Then the directory is sent to the server
    And the value is URL-encoded
