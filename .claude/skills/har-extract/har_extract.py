#!/usr/bin/env python3
"""Extract and summarize API requests from a HAR capture, redacting secrets.

Built for reverse-engineering web/app APIs (e.g. the SSI dive-log flow). Skips
static assets, lists hosts, and dumps method/status/path + query/form/JSON params
for dynamic requests, with sensitive keys and header values redacted.

Usage:
    python3 har_extract.py <file.har> [--host SUBSTR] [--path SUBSTR]
                           [--headers] [--assets] [--values]

    --host SUBSTR   only requests whose host contains SUBSTR (repeatable-ish:
                    comma-separated)
    --path SUBSTR   only requests whose path/url contains SUBSTR
    --headers       also print request header NAMES (+ presence of auth headers)
    --assets        include static assets (css/js/img/font) — off by default
    --values        show param values (still redacted); off => names + redaction
"""
import argparse
import json
import sys
import urllib.parse as up

SECRET_KEYS = {
    "password", "passwd", "pass", "pwd", "p", "token", "access_token",
    "id_token", "refresh_token", "authorization", "cookie", "set-cookie",
    "sessionid", "phpsessid", "secret", "apikey", "api_key", "x-ssi-auth",
    "x-auth-token", "x-csrf-token", "csrf",
}
ASSET_EXT = (
    ".css", ".js", ".png", ".jpg", ".jpeg", ".gif", ".svg", ".woff", ".woff2",
    ".ttf", ".ico", ".map", ".webp", ".mp4",
)


def red(key, val, show_values):
    if key.lower() in SECRET_KEYS:
        return "<REDACTED>"
    if not show_values:
        return "…" if val else ""
    return val


def is_asset(path):
    return path.lower().endswith(ASSET_EXT)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("har")
    ap.add_argument("--host", default="")
    ap.add_argument("--path", default="")
    ap.add_argument("--headers", action="store_true")
    ap.add_argument("--assets", action="store_true")
    ap.add_argument("--values", action="store_true")
    args = ap.parse_args()

    with open(args.har) as f:
        har = json.load(f)
    entries = har["log"]["entries"]
    hosts = [h.strip() for h in args.host.split(",") if h.strip()]

    # Host overview
    counts = {}
    for e in entries:
        netloc = up.urlparse(e["request"]["url"]).netloc
        counts[netloc] = counts.get(netloc, 0) + 1
    print(f"# {len(entries)} entries\n## hosts")
    for h, c in sorted(counts.items(), key=lambda x: -x[1]):
        print(f"  {c:4} {h}")

    print("\n## requests")
    for e in entries:
        req = e["request"]
        u = up.urlparse(req["url"])
        if hosts and not any(h in u.netloc for h in hosts):
            continue
        if args.path and args.path not in req["url"]:
            continue
        if not args.assets and is_asset(u.path):
            continue
        status = e["response"]["status"]
        ct = next((h["value"] for h in req.get("headers", [])
                   if h["name"].lower() == "content-type"), "")
        print(f"\n{req['method']} {status} {u.netloc}{u.path}")
        if u.query:
            for k, v in up.parse_qsl(u.query):
                print(f"    ?{k} = {red(k, v, args.values)}")
        pd = req.get("postData", {})
        if pd.get("params"):
            print(f"  [form] {ct}")
            for p in pd["params"]:
                print(f"    {p['name']} = {red(p['name'], up.unquote_plus(p.get('value','')), args.values)}")
        elif pd.get("text"):
            try:
                j = json.loads(pd["text"])
                print("  [json]")
                _walk(j, args.values)
            except Exception:
                print(f"  [body] {pd['text'][:300]}")
        if args.headers:
            names = [h["name"] for h in req.get("headers", [])]
            auth = [n for n in names if n.lower() in SECRET_KEYS]
            print(f"  headers: {names}")
            if auth:
                print(f"  AUTH headers present: {auth}")
        body = (e["response"].get("content", {}).get("text") or "")
        rct = next((h["value"] for h in e["response"].get("headers", [])
                    if h["name"].lower() == "content-type"), "")
        print(f"  -> {status} {rct} body_len={len(body)}")


def _walk(o, show_values, pre="    "):
    if isinstance(o, dict):
        for k, v in o.items():
            if isinstance(v, (dict, list)):
                print(f"{pre}{k}:")
                _walk(v, show_values, pre + "  ")
            else:
                print(f"{pre}{k} = {red(k, v, show_values)}")
    elif isinstance(o, list):
        print(f"{pre}[{len(o)} items]")
        if o:
            _walk(o[0], show_values, pre + "  ")


if __name__ == "__main__":
    main()
