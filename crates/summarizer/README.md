# Summarizer

The Summarizer component generates and maintains conversation summaries for context management in Agent Ops Room.

## Purpose

- Prevents unbounded context growth in long-running rooms
- Uses incremental summarization: new summary = condense(previous summary + new messages)
- Publishes periodic summaries to `rooms/{roomId}/summary` topic

## How It Works

1. Subscribes to `rooms/{roomId}/public` topic
2. Tracks message count
3. Every N messages (default: 50), generates a summary using LLM
4. New summaries incorporate previous summary + new messages
5. Publishes `SummaryPayload` with:
   - `summary_text`: Condensed conversation context
   - `covers_until_ts`: Timestamp of latest message included
   - `message_count`: Number of messages summarized
   - `generated_at`: When summary was created

## Agent Context Assembly

Agents use: `[Latest Summary] + [ALL messages where ts > covers_until_ts]`

This ensures **zero message loss** - every message is either in the summary or after `covers_until_ts`.

## Usage

```bash
cargo run --bin summarizer -- \
  --room-id default \
  --openai-api-key "your-key" \
  --openai-base-url "https://your-endpoint" \
  --summary-interval 50
```

## Environment Variables

- `AOR_ROOM_ID`: Room to monitor (default: "default")
- `AOR_OPENAI_API_KEY`: API key for LLM
- `AOR_OPENAI_MODEL`: Model name (default: "gpt-oss-120b")
- `AOR_OPENAI_BASE_URL`: API endpoint (default: OpenAI)
- `AOR_SUMMARY_INTERVAL`: Messages before summarizing (default: 50)
- `AOR_MQTT_HOST`: MQTT broker host (default: "localhost")
- `AOR_MQTT_PORT`: MQTT broker port (default: 1883)

## Architecture

See `ARCHITECTURE.md` and `SPEC.md` for details on the incremental summarization strategy and context management approach.
