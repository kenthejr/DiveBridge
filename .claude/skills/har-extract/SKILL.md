---
name: har-extract
description: Extract and summarize API requests from a HAR capture (hosts, endpoints, form/JSON params, auth headers) with secret redaction. Use when reverse-engineering a web or mobile API from a browser DevTools or proxy (mitmproxy/Proxyman) capture — e.g. discovering the SSI dive-log endpoints.
---

# har-extract

Reverse-engineer an HTTP API from a `.har` capture without leaking secrets or
overflowing context.

## Safety first
HAR files contain **live session cookies, tokens, and sometimes passwords**. Never
read a raw HAR into the conversation and never commit one. Keep HARs in the
scratchpad (outside the repo), run the script below to extract only what's needed
(redacted), then **delete the HAR**.

## Use
```sh
python3 .claude/skills/har-extract/har_extract.py <file.har> [options]
```
Options:
- `--host divessi` — only requests whose host contains the substring (comma-list ok)
- `--path mydivelog` — only requests whose URL contains the substring
- `--headers` — also print request header names + flag any auth headers present
- `--values` — show param values (still redacting secret keys); default hides them
- `--assets` — include static css/js/img/font requests (off by default)

## Workflow
1. Capture (see `docs/api-capture.md`): browser DevTools "Save all as HAR with
   content", or a mobile proxy (mitmproxy/Proxyman) export.
2. Move the HAR into the scratchpad.
3. Run with `--host <vendor>` to see the host list + dynamic endpoints.
4. Narrow with `--path` and add `--values` to read the actual create/login payload.
5. Copy the field names / endpoint into docs and a **sanitized** fixture (scrub
   account ids, buddy ids, free-text, emails). 
6. Delete the HAR.

## Notes / gotchas
- Browser HAR export often **strips Cookie/Set-Cookie** and may omit dynamic
  **response bodies** — so you'll see *selected* form values but not full dropdown
  option lists. To get full `<select>` options, fetch the form HTML with an
  authenticated session instead, or capture via a proxy that keeps responses.
- The redaction denylist lives at the top of `har_extract.py`; extend as needed.
