#!/usr/bin/env bash
# Forwards a Claude Code hook payload (stdin JSON) to the local Faro broker.
# Defensive: 1s timeout, backgrounded, ALWAYS exit 0 so it can never block
# or break a Claude Code session.
BROKER_URL="${FARO_BROKER_URL:-http://127.0.0.1:8765/event}"
payload="$(cat)"
curl -s -m 1 -X POST "$BROKER_URL" \
  -H 'Content-Type: application/json' \
  -d "$payload" >/dev/null 2>&1 &
exit 0
