#!/usr/bin/env bash
# End-to-end smoke test. Usage: BASE=http://localhost:8080 ./scripts/smoke.sh
set -euo pipefail
BASE="${BASE:-http://localhost:8080}"
JQ="${JQ:-jq}"

echo "→ health"
curl -fsS "$BASE/api/health" | "$JQ" '{ok, isolation: .sandbox.isolation, verifier}'

echo "→ create sample dataset"
DS=$(curl -fsS -X POST "$BASE/api/samples/orders" | "$JQ" -r '.dataset.id')
echo "  dataset $DS"

echo "→ ask"
RUN=$(curl -fsS -X POST "$BASE/api/datasets/$DS/ask" \
  -H 'content-type: application/json' \
  -d '{"question":"Clean this export. What was monthly net revenue, and which country generated the most revenue?"}' \
  | "$JQ" -r '.run_id')
echo "  run $RUN"

for _ in $(seq 1 90); do
  STATUS=$(curl -fsS "$BASE/api/runs/$RUN" | "$JQ" -r '.run.status')
  [ "$STATUS" = "complete" ] && break
  [ "$STATUS" = "failed" ] && { echo "run failed"; curl -fsS "$BASE/api/runs/$RUN" | "$JQ" '{error,log}'; exit 1; }
  sleep 2
done

echo "→ report"
curl -fsS "$BASE/api/runs/$RUN/report" | "$JQ" '{receipted: .report.receipted, unreceipted: .report.unreceipted, report: "'"$BASE"'/r/'"$RUN"'"}'

echo "→ containment drill"
DRILL=$(curl -fsS -X POST "$BASE/api/drills" | "$JQ" -r '.drill_id')
for _ in $(seq 1 60); do
  DSTATUS=$(curl -fsS "$BASE/api/drills/$DRILL" | "$JQ" -r '.drill.status')
  [ "$DSTATUS" = "complete" ] && break
  [ "$DSTATUS" = "failed" ] && { echo "drill failed"; exit 1; }
  sleep 2
done
curl -fsS "$BASE/api/drills/$DRILL" | "$JQ" '{steps: (.drill.evidence | length), verdicts: [.drill.evidence[].verdict], host: .drill.host}'
