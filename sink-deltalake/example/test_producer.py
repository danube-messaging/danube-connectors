#!/usr/bin/env python3
import asyncio
import json
import os
import random
import sys
from datetime import datetime, timezone

from danube import DanubeClientBuilder, DispatchStrategy

EVENT_TYPES = ["purchase", "user_signup", "user_login", "page_view", "api_call"]
USER_IDS = ["user_001", "user_002", "user_003", "user_004", "user_005"]
PRODUCTS = ["laptop", "phone", "tablet", "monitor", "keyboard"]


def build_message() -> dict:
    event_type = random.choice(EVENT_TYPES)
    user_id = random.choice(USER_IDS)
    timestamp = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")

    payload = {
        "event_type": event_type,
        "user_id": user_id,
        "timestamp": timestamp,
    }

    if event_type == "purchase":
        payload.update(
            {
                "product": random.choice(PRODUCTS),
                "amount": random.randint(50, 1049),
                "currency": "USD",
            }
        )

    return payload


async def main() -> int:
    service_url = os.environ.get("DANUBE_URL", "http://danube-broker:6650")
    topic = os.environ.get("TOPIC", "/default/events")
    count = int(os.environ.get("COUNT", "10"))
    interval_ms = int(os.environ.get("INTERVAL", "500"))
    schema_subject = os.environ.get("SCHEMA_SUBJECT", "events-schema-v1")
    producer_name = os.environ.get("PRODUCER_NAME", "test_producer")
    raw_message = os.environ.get("RAW_MESSAGE", "").strip()

    client = await DanubeClientBuilder().service_url(service_url).build()

    producer = (
        client.new_producer()
        .with_topic(topic)
        .with_name(producer_name)
        .with_schema_subject(schema_subject)
        .with_dispatch_strategy(DispatchStrategy.RELIABLE)
        .build()
    )

    await producer.create()

    success = 0
    failed = 0

    try:
        for index in range(1, count + 1):
            message = raw_message or json.dumps(build_message())

            event_type = "custom"
            user_id = "n/a"
            try:
                payload = json.loads(message)
                event_type = payload.get("event_type", event_type)
                user_id = payload.get("user_id", user_id)
            except Exception:
                pass

            try:
                await producer.send(message.encode())
                success += 1
                print(f"✅ [{index}/{count}] Sent: {event_type} ({user_id})")
            except Exception as exc:
                failed += 1
                print(f"❌ [{index}/{count}] Failed: {event_type} ({user_id})")
                print(exc)

            if index < count and interval_ms > 0:
                await asyncio.sleep(interval_ms / 1000)
    finally:
        await producer.close()

    print("")
    print("=" * 60)
    print("📊 Summary")
    print("=" * 60)
    print(f"Total: {count}")
    print(f"Success: {success}")
    print(f"Failed: {failed}")
    print("=" * 60)

    if success > 0:
        print("")
        print("💡 Next steps:")
        print("   1. Check connector logs:")
        print("      docker-compose logs -f deltalake-sink")
        print("")
        print("   2. Access MinIO Console:")
        print("      http://localhost:9001 (minioadmin/minioadmin)")
        print("      Browse to bucket: delta-tables/events")
        print("")
        print("   3. Query Delta table with Python:")
        print("      pip install deltalake pandas")
        print("      python3 -c 'import deltalake as dl; dt = dl.DeltaTable(\"s3://delta-tables/events\", storage_options={\"AWS_ACCESS_KEY_ID\": \"minioadmin\", \"AWS_SECRET_ACCESS_KEY\": \"minioadmin\", \"AWS_ENDPOINT_URL\": \"http://localhost:9000\", \"AWS_REGION\": \"us-east-1\", \"AWS_ALLOW_HTTP\": \"true\"}); print(dt.to_pandas())'")
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
