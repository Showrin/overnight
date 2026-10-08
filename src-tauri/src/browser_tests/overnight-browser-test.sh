#!/usr/bin/env bash
# Installed by Overnight. Hands a browser test doc to the Overnight app on the
# host, where a Claude in Chrome agent runs it, then waits for the report.
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage:
  overnight-browser-test run <doc.md|-> [--branch NAME] [--port PORT] [--timeout MINUTES]
      Submit a test doc and wait for the report (printed as Markdown).
  overnight-browser-test submit <doc.md|-> [--branch NAME] [--port PORT]
      Submit without waiting; prints the test id.
  overnight-browser-test wait <id> [--timeout MINUTES]
      Wait for an already-submitted test and print its report.
  overnight-browser-test status <id>
      Print a test's current status as JSON.
  overnight-browser-test check
      Check that Overnight is reachable and Claude in Chrome is enabled.

Exit codes: 0 report delivered, 2 test failed/cancelled/timed out, 1 usage or connection error.
USAGE
}

die() { echo "overnight-browser-test: $*" >&2; exit 1; }

: "${OVERNIGHT_BROWSER_URL:?Claude in Chrome isn't enabled for this sandbox in Overnight (OVERNIGHT_BROWSER_URL is unset)}"
: "${OVERNIGHT_BROWSER_TOKEN:?Claude in Chrome isn't enabled for this sandbox in Overnight (OVERNIGHT_BROWSER_TOKEN is unset)}"
command -v curl >/dev/null || die "curl is required"
command -v jq >/dev/null || die "jq is required"

api() {
  local method=$1 path=$2 body=${3:-}
  local args=(-sS -X "$method" -H "Authorization: Bearer $OVERNIGHT_BROWSER_TOKEN" -w '\n%{http_code}')
  if [ -n "$body" ]; then args+=(-H 'Content-Type: application/json' --data-binary @-); fi
  local out
  if [ -n "$body" ]; then
    out=$(printf '%s' "$body" | curl "${args[@]}" "$OVERNIGHT_BROWSER_URL$path") || die "couldn't reach Overnight at $OVERNIGHT_BROWSER_URL — is the app running?"
  else
    out=$(curl "${args[@]}" "$OVERNIGHT_BROWSER_URL$path") || die "couldn't reach Overnight at $OVERNIGHT_BROWSER_URL — is the app running?"
  fi
  local code=${out##*$'\n'}
  local json=${out%$'\n'*}
  if [ "$code" -ge 400 ]; then
    die "$(printf '%s' "$json" | jq -r '.error // empty' 2>/dev/null || true) (HTTP $code)"
  fi
  printf '%s' "$json"
}

submit() {
  local doc_file=$1 branch=$2 port=$3
  local doc
  if [ "$doc_file" = "-" ]; then doc=$(cat); else [ -f "$doc_file" ] || die "no such file: $doc_file"; doc=$(cat "$doc_file"); fi
  [ -n "$branch" ] || branch=$(git rev-parse --abbrev-ref HEAD 2>/dev/null || true)
  local body
  body=$(jq -n --arg doc "$doc" --arg branch "$branch" --arg port "$port" \
    '{doc: $doc} + (if $branch != "" then {branch: $branch} else {} end) + (if $port != "" then {port: ($port | tonumber)} else {} end)')
  api POST /v1/browser-tests "$body" | jq -r '.id'
}

wait_for() {
  local id=$1 timeout_min=$2
  local deadline=$(( $(date +%s) + timeout_min * 60 )) last=""
  while :; do
    local json status
    json=$(api GET "/v1/browser-tests/$id")
    status=$(printf '%s' "$json" | jq -r '.status')
    if [ "$status" != "$last" ]; then
      case $status in
        awaiting_host) echo "Waiting for the user to start the app on the host and press Start in Overnight..." >&2 ;;
        queued) echo "Queued for the host browser..." >&2 ;;
        running) echo "The host agent is testing in Chrome..." >&2 ;;
      esac
      last=$status
    fi
    case $status in
      done) printf '%s\n' "$json" | jq -r '.report'; return 0 ;;
      failed) echo "Browser test failed: $(printf '%s' "$json" | jq -r '.error // "unknown error"')" >&2; return 2 ;;
      cancelled) echo "Browser test was cancelled in Overnight." >&2; return 2 ;;
    esac
    if [ "$(date +%s)" -ge "$deadline" ]; then
      echo "Timed out after ${timeout_min}m; test $id is still $status. Resume with: overnight-browser-test wait $id" >&2
      return 2
    fi
    sleep 10
  done
}

cmd=${1:-}; shift || true
branch="" port="${OVERNIGHT_BROWSER_PORT:-}" timeout_min=60 positional=()
while [ $# -gt 0 ]; do
  case $1 in
    --branch) branch=${2:?}; shift 2 ;;
    --port) port=${2:?}; shift 2 ;;
    --timeout) timeout_min=${2:?}; shift 2 ;;
    -h|--help) usage; exit 0 ;;
    *) positional+=("$1"); shift ;;
  esac
done

case $cmd in
  run)
    [ ${#positional[@]} -eq 1 ] || { usage >&2; exit 1; }
    id=$(submit "${positional[0]}" "$branch" "$port")
    echo "Submitted browser test $id" >&2
    wait_for "$id" "$timeout_min" ;;
  submit)
    [ ${#positional[@]} -eq 1 ] || { usage >&2; exit 1; }
    submit "${positional[0]}" "$branch" "$port" ;;
  wait)
    [ ${#positional[@]} -eq 1 ] || { usage >&2; exit 1; }
    wait_for "${positional[0]}" "$timeout_min" ;;
  status)
    [ ${#positional[@]} -eq 1 ] || { usage >&2; exit 1; }
    api GET "/v1/browser-tests/${positional[0]}" | jq . ;;
  check)
    api GET /v1/health | jq . ;;
  -h|--help|help) usage ;;
  *) usage >&2; exit 1 ;;
esac
