#!/usr/bin/env python3
import asyncio
import json
import os
import random
import sys
import time
from datetime import datetime, timezone

from danube import DanubeClientBuilder, DispatchStrategy

EVENT_TYPES = ["user_signup", "user_login", "purchase", "page_view", "api_call"]
USER_IDS = ["user_001", "user_002", "user_003", "user_004", "user_005"]
PRODUCTS = ["laptop", "phone", "tablet", "monitor", "keyboard"]


def build_message(index: int) -> dict:
    event_type = random.choice(EVENT_TYPES)
    user_id = random.choice(USER_IDS)
    product = random.choice(PRODUCTS)
    amount = random.randint(50, 1049)
    event_id = f"evt_{index}_{int(time.time())}_{random.randint(1000, 99999)}"
    timestamp = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")

    if event_type == "purchase":
        data = {
            "product": product,
            "amount": amount,
            "currency": "USD",
        }
    elif event_type == "user_signup":
        data = {
            "email": f"{user_id}@example.com",
            "source": "web",
        }
    elif event_type == "user_login":
        data = {
            "ip_address": f"192.168.1.{random.randint(1, 254)}",
            "device": "desktop",
        }
    elif event_type == "page_view":
        data = {
            "page": f"/products/{product}",
            "duration_ms": random.randint(0, 59999),
        }
    else:
        data = {
            "endpoint": f"/api/v1/{product}",
            "method": "GET",
            "status_code": 200,
            "response_time_ms": random.randint(0, 499),
        }

    return {
        "event_id": event_id,
        "event_type": event_type,
        "timestamp": timestamp,
        "user_id": user_id,
        "data": data,
    }


async def main() -> int:
    service_url = os.environ.get("DANUBE_URL", "http://danube-broker:6650")
    topic = os.environ.get("TOPIC", "/default/events")
    count = int(os.environ.get("COUNT", "10"))
    interval_ms = int(os.environ.get("INTERVAL", "500"))
    schema_subject = os.environ.get("SCHEMA_SUBJECT", "events-v1")
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
            message = raw_message or json.dumps(build_message(index))

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
        print("      docker-compose logs -f surrealdb-sink")
        print("")
        print("   2. Query SurrealDB:")
        print("      docker exec -it surrealdb /surreal sql --endpoint http://localhost:8000 --username root --password root --namespace default --database default")
        print("      Then run: SELECT * FROM events;")
        print("")
        print("   3. Or use HTTP API:")
        print("      curl -X POST http://localhost:8000/sql -H 'Content-Type: application/json' -H 'Accept: application/json' -u root:root -d '{\"query\": \"SELECT * FROM events LIMIT 10;\"}'")
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
