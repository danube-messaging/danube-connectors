#!/usr/bin/env python3
import asyncio
import json
import os
import sys
from pathlib import Path

from danube import DanubeClientBuilder, DispatchStrategy


async def main() -> int:
    service_url = os.environ.get("DANUBE_URL", "http://danube-broker:6650")
    topic = os.environ.get("TOPIC", "/default/vectors")
    embeddings_file = Path(os.environ.get("EMBEDDINGS_FILE", "embeddings.jsonl"))
    interval_ms = int(os.environ.get("INTERVAL", "500"))
    schema_subject = os.environ.get("SCHEMA_SUBJECT", "embeddings-v1")
    producer_name = os.environ.get("PRODUCER_NAME", "test_producer")

    lines = [line.rstrip("\n") for line in embeddings_file.read_text().splitlines() if line.strip()]
    total = len(lines)
    success = 0
    failed = 0

    client = await (
        DanubeClientBuilder()
        .service_url(service_url)
        .build()
    )

    producer = (
        client.new_producer()
        .with_topic(topic)
        .with_name(producer_name)
        .with_schema_subject(schema_subject)
        .with_dispatch_strategy(DispatchStrategy.RELIABLE)
        .build()
    )

    await producer.create()

    try:
        for count, message in enumerate(lines, start=1):
            text_short = "N/A"
            try:
                payload = json.loads(message)
                text = payload.get("payload", {}).get("text", "N/A")
                text_short = text[:50]
            except Exception:
                pass

            try:
                await producer.send(message.encode())
                success += 1
                print(f"✅ [{count}/{total}] Sent: {text_short}...")
            except Exception as exc:
                failed += 1
                print(f"❌ [{count}/{total}] Failed: {text_short}...")
                print(exc)

            if count < total and interval_ms > 0:
                await asyncio.sleep(interval_ms / 1000)
    finally:
        await producer.close()

    print("")
    print("=" * 60)
    print("📊 Summary")
    print("=" * 60)
    print(f"Total: {total}")
    print(f"Success: {success}")
    print(f"Failed: {failed}")
    print("=" * 60)

    if success > 0:
        print("")
        print("💡 Next steps:")
        print("   1. Check connector logs:")
        print("      docker-compose logs -f qdrant-sink")
        print("")
        print("   2. View Qdrant dashboard:")
        print("      http://localhost:6333/dashboard")
        print("")
        print("   3. Search vectors:")
        print("      docker-compose --profile tools run --rm vector-search --query 'password reset'")
        print("")

    return 0 if failed == 0 else 1


if __name__ == "__main__":
    try:
        raise SystemExit(asyncio.run(main()))
    except KeyboardInterrupt:
        print("\nInterrupted")
        raise SystemExit(130)
    except Exception as exc:
        print(f"❌ Error: {exc}", file=sys.stderr)
        raise SystemExit(1)
