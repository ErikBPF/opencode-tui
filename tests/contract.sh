#!/usr/bin/env bash
# Binding for features/attach-and-prompt.feature.
#
# The scenarios are an unautomated contract until the cucumber-rs steps in
# tests/ run against a pinned `opencode serve` and are observed failing against
# the skeleton. Until then this asserts the contract exists, names a Feature,
# and has scenarios, so it cannot be silently dropped from the gate.
set -euo pipefail

feature="features/attach-and-prompt.feature"
[ -f "$feature" ] || { echo "missing $feature"; exit 1; }
grep -q '^Feature:' "$feature" || { echo "no Feature: in $feature"; exit 1; }
grep -q 'Scenario:' "$feature" || { echo "no Scenario: in $feature"; exit 1; }

echo "contract present: $feature ($(grep -c 'Scenario:' "$feature") scenarios, binding pending)"
