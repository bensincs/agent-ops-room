#!/bin/bash
# Send a user message to the Agent Ops Room

if [ -z "$1" ]; then
  echo "Usage: ./send-message.sh \"Your message here\""
  exit 1
fi

MESSAGE="$1"
ROOM_ID="${2:-default}"
USER_ID="${3:-alice}"

TIMESTAMP=$(date +%s)
MSG_ID="user_msg_${TIMESTAMP}"

echo "📤 Sending message to room: $ROOM_ID"
echo "   From user: $USER_ID"
echo "   Message: $MESSAGE"
echo ""

mosquitto_pub -h localhost -p 1883 -t "rooms/${ROOM_ID}/public" -m "{
  \"id\": \"${MSG_ID}\",
  \"type\": \"say\",
  \"room_id\": \"${ROOM_ID}\",
  \"from\": {\"kind\": \"user\", \"id\": \"${USER_ID}\"},
  \"ts\": ${TIMESTAMP},
  \"payload\": {\"text\": \"${MESSAGE}\"}
}"

echo "✅ Message sent!"
echo ""
echo "Watch the terminal windows to see the system respond."
