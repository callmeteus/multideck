# Ghidra notes: Stream Deck desktop and mobile link

This file is a skeleton. It records what we have actually opened, what we saw,
and where. It contains no findings yet, because no binary has been analysed as
of 2026-11-24.

Rules for this file:

- Every claim needs a binary hash, a function address and what was on screen.
- "Interesting" is not evidence. A string is evidence of a string, not of a
  protocol.
- Never paste vendor code or long decompiler output into the repository. Quote
  the minimum, in a line or two, and describe the rest.
- Keep personal data out: no phone names, no account e-mails, no tokens.

## Status

| Area | Status | Owner | Notes |
|------|--------|-------|-------|
| Tooling installed (Ghidra 12.1.4, JDK 21) | Done | | JDK 21 at `C:\Users\Gio\scoop\apps\temurin21-jdk\current`, `analyzeHeadless.bat` verified with `JAVA_HOME` pointing there |
| Elgato desktop app installed | Blocked | | Only WaveLink is installed on this machine; the Stream Deck app is required before any binary work |
| REA MCP reachable (`npx -y rea-agents@6.10.1`) | Not started | | |
| Binary inventory | Not started | | |
| Cheap string pass (`multideck-probe strings`) | Not started | | |
| Discovery code located | Not started | | mDNS `_elg._tcp` is the lead |
| Session channel located | Not started | | Port and framing unknown |
| Pairing flow located | Not started | | |
| TLS or pinning confirmed | Not started | | |
| Dynamic capture cross-check | Not started | | See `capture-playbook.md` |

## Binary inventory

Fill one row per file that was opened in Ghidra or scanned for strings. Do not
list the whole install tree; only files that were touched.

| File | Hash (SHA-256, first 16) | Version resource | Architecture | Opened in Ghidra | Why |
|------|--------------------------|------------------|--------------|------------------|-----|
| | | | | | |

How to collect the basics:

```powershell
Get-ChildItem "<install dir>" -Recurse -Include *.exe,*.dll,*.node |
  Select-Object FullName, Length, LastWriteTime
Get-FileHash "<file>" -Algorithm SHA256
(Get-Item "<file>").VersionInfo | Format-List FileVersion, ProductVersion, CompanyName
```

## Interesting strings

One row per hit that mattered. `offset` comes straight from
`multideck-probe strings --grep <text>`, so it can be reproduced.

| File | Offset | String | Lead | Followed up |
|------|--------|--------|------|-------------|
| | | | | |

Source of the leads, in order of cost:

1. `_elg._tcp`, `elg`, `cora`, `5353`, `224.0.0.251`
2. `pair`, `pin`, `qr`, `token`, `session`
3. `mdns`, `bonjour`, `dnssd`, `zeroconf`
4. certificate related text if TLS is suspected: `pinning`, `trust`, `verify`

## Functions

Only functions that were actually decompiled. An address alone is not a
finding.

| Address | Binary | Name in Ghidra | Why it matters | Confidence |
|---------|--------|----------------|----------------|------------|
| | | | | |

Confidence vocabulary:

- **Confirmed**: the code does what the row says, we followed the calls.
- **Likely**: one strong signal, not fully traced.
- **Guess**: pattern matched, unverified.

## Cross references

Record the edges that took us from a string to a function, so the path can be
replayed.

| From | To | Kind | Note |
|------|----|------|------|
| | | | |

## Evidence log

Append-only. One block per session, newest at the bottom. Include the date, the
tool versions, the binary hash and the exact filters or address ranges used.

```text
2026-MM-DD
Ghidra 12.1.4, JDK 21.x, REA MCP 6.10.1
Binary: <path> sha256:<hash>
Steps: <what was done>
Observed: <what was on screen, quoted minimally>
Conclusion: <confirmed / likely / guess>
Next: <the next concrete check>
```

## Open questions

Keep this list honest. An empty list means the analysis is done, not that
nothing is left.

- [ ] Is the mobile link owned by a native service, a bundled script, or the
      main window process?
- [ ] Does discovery use multicast mDNS or unicast DNS-SD?
- [ ] Is the session channel plaintext framing, TLS, or TLS with pinning?
- [ ] Where does the pairing secret live, and is it per-device or per-account?
- [ ] Does the protocol carry key images as PNG, JPEG, raw RGBA or a custom
      encoding?
- [ ] What is the maximum frame size, and is there a length field before the
      magic bytes?

## TODOs

- [x] Install JDK 21 and confirm Ghidra starts.
      Done: `JAVA_HOME=C:\Users\Gio\scoop\apps\temurin21-jdk\current` and
      `support\analyzeHeadless.bat` prints its usage instead of a Java error.
- [ ] Install the Elgato Stream Deck desktop app, then copy the install
      directory to a scratch location before touching it.
- [ ] Start the REA MCP server and confirm the agent can list a project.
- [ ] Copy the install directory to a scratch location and hash the candidates.
- [ ] Run the cheap string pass on each candidate before importing.
- [ ] Check for bundled JavaScript or ASAR before decompiling native code.
- [ ] Import the first candidate and record the inventory row.
- [ ] Fill the open questions above only with confirmed answers, or mark them
      still open.
