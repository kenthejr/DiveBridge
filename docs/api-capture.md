# Capturing the SSI API (browser + mobile)

We compare two capture surfaces because the **mobile app may use cleaner JSON
endpoints** (and the dive-computer profile-attach flow) than the legacy web form.
Use `har-extract` skill to analyze any capture. **Always delete captures after
extracting; they hold live sessions.**

## A. Browser (done once; repeat for full dropdowns)
1. Chrome → my.divessi.com, log in. DevTools (Cmd+Opt+I) → Network, "Preserve log".
   **Also enable response bodies:** keep the tab open and DON'T use "HAR without
   content" — use **"Save all as HAR with content"**. (Chrome still strips Cookies.)
2. Do the action (add a dive / open the add form). Export HAR → scratchpad.
3. `python3 .claude/skills/har-extract/har_extract.py <har> --host divessi --values`

### Quicker: session-cookie handoff (best for testing + reading dropdowns)
Logged into my.divessi.com, in DevTools open any my.divessi.com request → Request
Headers → copy the entire **`cookie:`** value (likely contains `PHPSESSID`). Put it
in a scratchpad file (NOT chat, NOT the repo). With that session we can:
- `GET https://my.divessi.com/mydivelog/add` and parse every `<select>` to get the
  full `var_*` vocabulary tables (the HAR only showed selected values), and
- POST a single test dive to `mydivelog_18.php`.
Session cookies expire — grab a fresh one when testing.

## B. Mobile app (iPhone) — to see the app's API
The MySSI app likely talks to `rest.divessi.com` / `api.divessi.com` with JSON. To
intercept on macOS:

**Option 1 — Proxyman (easiest, free tier).**
1. Install Proxyman on the Mac (`brew install --cask proxyman`).
2. Proxyman → Certificate → "Install Certificate on iOS → Physical Device" and
   follow the guided steps:
   - Set the iPhone Wi-Fi HTTP proxy to the Mac's IP + Proxyman's port (e.g. 9090).
   - Safari on the phone → download Proxyman CA → Settings → General → VPN & Device
     Management → install profile → Settings → General → About → Certificate Trust
     Settings → **enable full trust** for the Proxyman cert.
3. Open the MySSI app, log in, add/sync a dive. Watch flows in Proxyman.
4. Export the relevant flows as HAR → scratchpad → run `har-extract`.

**Option 2 — mitmproxy (free, CLI).**
1. `brew install mitmproxy` then run `mitmweb` (web UI at 127.0.0.1:8081).
2. iPhone Wi-Fi → Configure Proxy → Manual → Mac IP, port 8080.
3. On the phone visit `http://mitm.it`, install the iOS cert, then trust it
   (VPN & Device Management + Certificate Trust Settings, as above).
4. Use the app; in mitmweb filter `~d divessi`. Export flows (File → Save) or save
   as HAR.

**Android** is similar (HTTP Toolkit is very smooth for Android; user cert trust
needed; from Android 7+ apps must opt into user CAs — most do for debug, some don't).

### Caveat: certificate pinning
If the app pins certs, the proxy will show TLS errors and no readable traffic. SSI's
app probably does NOT pin (it's a content app), but if it does, capture is blocked
without Frida/a patched build — fall back to the web flow we already have. Try the
proxy first; if flows appear, great.

## What we want from the app capture
- Whether create uses a clean JSON endpoint (vs the web form) and its auth (the SSO
  `x-ssi-auth` header / bearer token vs cookie).
- The **dive-computer profile attach** path (does the app upload the depth/time
  profile, not just summary?). The web form has empty `diveComputerData_ue` /
  `divecomputer_ref` fields — the app likely populates them.
- Dive-site search + vocabulary endpoints with real JSON responses.
