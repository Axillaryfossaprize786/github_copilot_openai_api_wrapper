"""Quick end-to-end test for the copilot-wrapper (Python or Rust)."""
import httpx
import json
import sys
import time

BASE = "http://127.0.0.1:8080"
PASSED = 0
FAILED = 0


def test(name, fn):
    global PASSED, FAILED
    try:
        fn()
        print(f"  ✓ {name}")
        PASSED += 1
    except Exception as e:
        print(f"  ✗ {name}: {e}")
        FAILED += 1


def test_health():
    r = httpx.get(f"{BASE}/health", timeout=5)
    assert r.status_code == 200, f"status={r.status_code}"
    data = r.json()
    assert data == {"status": "ok"}, f"body={data}"


def test_models():
    r = httpx.get(f"{BASE}/v1/models", timeout=15)
    assert r.status_code == 200, f"status={r.status_code}"
    data = r.json()
    assert data.get("object") == "list", f"object={data.get('object')}"
    assert isinstance(data.get("data"), list), "data is not a list"
    assert len(data["data"]) > 0, "no models returned"
    first = data["data"][0]
    assert "id" in first, f"model missing id: {first}"
    print(f"      → {len(data['data'])} models, first: {first['id']}")


def test_chat_non_streaming():
    payload = {
        "model": "gpt-4o-mini",
        "messages": [{"role": "user", "content": "Say exactly: HELLO TEST"}],
        "stream": False,
        "max_tokens": 20,
    }
    r = httpx.post(f"{BASE}/v1/chat/completions", json=payload, timeout=30)
    assert r.status_code == 200, f"status={r.status_code}, body={r.text[:200]}"
    data = r.json()
    assert "choices" in data, f"no choices: {list(data.keys())}"
    assert len(data["choices"]) > 0, "empty choices"
    msg = data["choices"][0].get("message", {}).get("content", "")
    print(f"      → response: {msg[:80]}")


def test_chat_streaming():
    payload = {
        "model": "gpt-4o-mini",
        "messages": [{"role": "user", "content": "Say exactly: STREAM TEST"}],
        "stream": True,
        "max_tokens": 20,
    }
    chunks = []
    content_parts = []
    with httpx.stream("POST", f"{BASE}/v1/chat/completions", json=payload, timeout=30) as r:
        assert r.status_code == 200, f"status={r.status_code}"
        ct = r.headers.get("content-type", "")
        assert "text/event-stream" in ct, f"content-type={ct}"
        for line in r.iter_lines():
            line = line.strip()
            if not line:
                continue
            if line.startswith("data: "):
                data_str = line[6:]
                if data_str == "[DONE]":
                    chunks.append("[DONE]")
                    break
                try:
                    chunk = json.loads(data_str)
                    chunks.append(chunk)
                    choices = chunk.get("choices", [])
                    if choices:
                        delta = choices[0].get("delta", {})
                        if "content" in delta and delta["content"] is not None:
                            content_parts.append(delta["content"])
                except json.JSONDecodeError:
                    pass

    assert len(chunks) > 1, f"only {len(chunks)} chunks"
    assert chunks[-1] == "[DONE]", f"last chunk: {chunks[-1]}"
    full_content = "".join(content_parts)
    print(f"      → {len(chunks)} chunks, content: {full_content[:80]}")


if __name__ == "__main__":
    print(f"\nTesting copilot-wrapper at {BASE}\n")

    test("GET /health", test_health)
    test("GET /v1/models", test_models)
    test("POST /v1/chat/completions (non-streaming)", test_chat_non_streaming)
    test("POST /v1/chat/completions (streaming)", test_chat_streaming)

    print(f"\n{'='*40}")
    print(f"  {PASSED} passed, {FAILED} failed")
    print(f"{'='*40}\n")

    sys.exit(1 if FAILED else 0)
