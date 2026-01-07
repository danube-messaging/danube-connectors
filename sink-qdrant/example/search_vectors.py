#!/usr/bin/env python3
"""
Vector search script for Qdrant

Demonstrates semantic search over vectors stored by the Qdrant Sink Connector.

Install dependencies:
    pip install qdrant-client sentence-transformers
"""

import argparse
from typing import List, Dict, Any

try:
    from qdrant_client import QdrantClient
    from qdrant_client.models import Distance, VectorParams
    QDRANT_AVAILABLE = True
except ImportError:
    QDRANT_AVAILABLE = False
    print("❌ qdrant-client not installed")
    print("   Install with: pip install qdrant-client")
    exit(1)

try:
    from sentence_transformers import SentenceTransformer
    TRANSFORMERS_AVAILABLE = True
except ImportError:
    TRANSFORMERS_AVAILABLE = False
    print("⚠️  sentence-transformers not installed")
    print("   Install with: pip install sentence-transformers")


def search_similar(
    client: QdrantClient,
    collection_name: str,
    query_text: str,
    model: SentenceTransformer,
    limit: int = 5
) -> List[Dict[str, Any]]:
    """Search for similar vectors"""
    
    # Generate query embedding
    print(f"\n🔍 Generating embedding for: '{query_text}'")
    query_vector = model.encode(query_text).tolist()
    
    # Search Qdrant
    print(f"🔎 Searching collection '{collection_name}'...")
    results = client.query_points(
        collection_name=collection_name,
        query=query_vector,
        limit=limit,
        with_payload=True
    ).points
    
    return results


def print_results(results: List[Any], show_metadata: bool = False):
    """Pretty print search results"""
    
    if not results:
        print("\n❌ No results found")
        return
    
    print(f"\n✅ Found {len(results)} similar vectors:\n")
    print("=" * 80)
    
    for i, hit in enumerate(results, 1):
        score = hit.score
        payload = hit.payload
        
        # Extract main fields
        text = payload.get("text", "N/A")
        category = payload.get("category", "N/A")
        user_id = payload.get("user_id", "N/A")
        
        # Print result
        print(f"\n{i}. Score: {score:.4f}")
        print(f"   Text: {text}")
        print(f"   Category: {category}")
        print(f"   User: {user_id}")
        
        # Show Danube metadata if available and requested
        if show_metadata:
            danube_fields = {
                k: v for k, v in payload.items() 
                if k.startswith("_danube_")
            }
            if danube_fields:
                print(f"   Danube Metadata:")
                for key, value in danube_fields.items():
                    print(f"     - {key}: {value}")
        
        print("-" * 80)


def list_collections(client: QdrantClient):
    """List all collections"""
    collections = client.get_collections()
    
    print("\n📚 Available Collections:")
    print("=" * 60)
    
    for collection in collections.collections:
        print(f"  - {collection.name}")
    
    print("=" * 60)


def collection_info(client: QdrantClient, collection_name: str):
    """Show collection information"""
    try:
        info = client.get_collection(collection_name)
        
        print(f"\n📊 Collection: {collection_name}")
        print("=" * 60)
        print(f"Points Count: {info.points_count}")
        print(f"Status: {info.status}")
        print(f"Vector Size: {info.config.params.vectors.size}")
        print(f"Distance: {info.config.params.vectors.distance}")
        print("=" * 60)
        
    except Exception as e:
        print(f"❌ Error getting collection info: {e}")


def main():
    parser = argparse.ArgumentParser(description="Search vectors in Qdrant")
    parser.add_argument(
        "--url",
        default="http://localhost:6333",
        help="Qdrant HTTP URL"
    )
    parser.add_argument(
        "--collection",
        default="vectors",
        help="Collection name"
    )
    parser.add_argument(
        "--query",
        help="Search query text"
    )
    parser.add_argument(
        "--limit",
        type=int,
        default=5,
        help="Number of results to return"
    )
    parser.add_argument(
        "--model",
        default="all-MiniLM-L6-v2",
        help="Sentence transformer model name"
    )
    parser.add_argument(
        "--show-metadata",
        action="store_true",
        help="Show Danube metadata in results"
    )
    parser.add_argument(
        "--list",
        action="store_true",
        help="List all collections"
    )
    parser.add_argument(
        "--info",
        action="store_true",
        help="Show collection info"
    )
    
    args = parser.parse_args()
    
    print("=" * 80)
    print("🔍 Qdrant Vector Search")
    print("=" * 80)
    
    # Connect to Qdrant
    print(f"🔗 Connecting to Qdrant at {args.url}...")
    client = QdrantClient(url=args.url)
    
    # List collections
    if args.list:
        list_collections(client)
        return
    
    # Show collection info
    if args.info:
        collection_info(client, args.collection)
        return
    
    # Search requires query
    if not args.query:
        print("❌ --query is required for search")
        print("   Example: --query 'how to reset password'")
        return
    
    if not TRANSFORMERS_AVAILABLE:
        print("❌ Cannot search without sentence-transformers")
        return
    
    # Load model
    print(f"📊 Loading model: {args.model}")
    model = SentenceTransformer(args.model)
    
    # Perform search
    results = search_similar(
        client,
        args.collection,
        args.query,
        model,
        args.limit
    )
    
    # Print results
    print_results(results, args.show_metadata)
    
    print("\n" + "=" * 80)


if __name__ == "__main__":
    main()
