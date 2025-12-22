#!/bin/bash
# Monitor all messages in an Agent Ops Room

ROOM_ID="${1:-default}"

echo "👀 Monitoring room: $ROOM_ID"
echo "================================"
echo ""
echo "Listening to:"
echo "  - rooms/${ROOM_ID}/public (user-visible chat)"
echo "  - rooms/${ROOM_ID}/control (system events)"
echo ""

mosquitto_sub -h localhost -p 1883 -t "rooms/${ROOM_ID}/public" -t "rooms/${ROOM_ID}/control" -F "@Y-@m-@dT@H:@M:@S %t %p" | while read line; do
  echo "$line" | jq -R -r '. as $raw | try (fromjson | "[\(.type)] \(.from.id): \(.payload | tostring)") catch $raw'
done
