#!/usr/bin/env python3
"""
Generate sample embeddings for testing the Qdrant Sink Connector

This script generates embeddings and saves them to a JSON file.
Use test_producer.sh to send them to Danube.

Install dependencies:
    pip install sentence-transformers
"""

import json
import argparse
from typing import List, Dict, Any

try:
    from sentence_transformers import SentenceTransformer
    TRANSFORMERS_AVAILABLE = True
except ImportError:
    TRANSFORMERS_AVAILABLE = False
    print("⚠️  sentence-transformers not installed")
    print("   For real embeddings: pip install sentence-transformers")
    print("   Falling back to random vectors")

# Sample data
SAMPLES = [
    {"text": "How do I reset my password?", "category": "account", "user_id": "user-001"},
    {"text": "My account is locked", "category": "account", "user_id": "user-002"},
    {"text": "Cannot access my dashboard", "category": "technical", "user_id": "user-003"},
    {"text": "Billing invoice question", "category": "billing", "user_id": "user-004"},
    {"text": "How to upgrade my plan?", "category": "billing", "user_id": "user-005"},
    {"text": "API rate limit exceeded", "category": "technical", "user_id": "user-006"},
    {"text": "Integration not working", "category": "technical", "user_id": "user-007"},
    {"text": "Need help with setup", "category": "support", "user_id": "user-008"},
    {"text": "Feature request for export", "category": "feature", "user_id": "user-009"},
    {"text": "Data export failed", "category": "technical", "user_id": "user-010"},
]


def generate_embeddings(count: int, model_name: str, output_file: str):
    """Generate embeddings and save to file"""
    
    print("=" * 60)
    print("🎯 Embedding Generator for Qdrant Sink Connector")
    print("=" * 60)
    
    # Load model or use random
    if TRANSFORMERS_AVAILABLE:
        print(f"📊 Loading model: {model_name}")
        model = SentenceTransformer(model_name)
        dimension = model.get_sentence_embedding_dimension()
        print(f"✅ Model loaded (dimension: {dimension})")
    else:
        import random
        dimension = 384
        print(f"⚠️  Using random {dimension}-dimensional vectors")
    
    # Generate embeddings
    messages = []
    print(f"\n🔄 Generating {count} embeddings...")
    
    for i in range(count):
        sample = SAMPLES[i % len(SAMPLES)]
        
        # Generate embedding
        if TRANSFORMERS_AVAILABLE:
            embedding = model.encode(sample["text"]).tolist()
        else:
            import random
            embedding = [random.random() for _ in range(dimension)]
        
        # Create message
        message = {
            "id": f"msg-{i+1:04d}",
            "vector": embedding,
            "payload": {
                "text": sample["text"],
                "category": sample["category"],
                "user_id": sample["user_id"],
                "message_index": i + 1
            }
        }
        
        messages.append(message)
        
        if (i + 1) % 10 == 0:
            print(f"   Generated {i + 1}/{count}...")
    
    # Save to file
    print(f"\n💾 Saving to {output_file}...")
    with open(output_file, 'w') as f:
        for msg in messages:
            f.write(json.dumps(msg) + '\n')
    
    print(f"✅ Saved {len(messages)} messages")
    print("\n" + "=" * 60)
    print("📊 Summary:")
    print(f"   Messages: {len(messages)}")
    print(f"   Dimension: {dimension}")
    print(f"   Model: {model_name if TRANSFORMERS_AVAILABLE else 'random'}")
    print(f"   Output: {output_file}")
    print("=" * 60)
    print("\n💡 Next step:")
    print(f"   Run: ./test_producer.sh")
    print()


def main():
    parser = argparse.ArgumentParser(
        description="Generate embeddings for Qdrant Sink Connector testing"
    )
    parser.add_argument(
        "--count",
        type=int,
        default=10,
        help="Number of messages to generate (default: 10)"
    )
    parser.add_argument(
        "--model",
        default="all-MiniLM-L6-v2",
        help="Sentence transformer model (default: all-MiniLM-L6-v2)"
    )
    parser.add_argument(
        "--output",
        default="embeddings.jsonl",
        help="Output file (default: embeddings.jsonl)"
    )
    
    args = parser.parse_args()
    
    generate_embeddings(args.count, args.model, args.output)


if __name__ == "__main__":
    main()
