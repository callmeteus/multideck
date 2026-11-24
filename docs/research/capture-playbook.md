> The installer is a per-machine MSI. `scripts/fetch-streamdeck.ps1` downloads it and
> extracts the payload with an administrative install (`msiexec /a`), so no system
> state changes. The extracted app can be started in place for live observations.

# Capture playbook: Stream Deck desktop to Stream Deck Mobile

Status as of 2026-11-24: **nothing has been captured yet.** This document is a
plan with explicit confidence markers. Do not read any line here as evidence
that a packet was seen.

Confidence markers used throughout:

- **Verified**: observed on this workstation or stated by the vendor/tool docs.
- **Reported**: found in public documentation or open-source implementations,
  not reproduced by us.
- **Hypothesis**: our guess. Must be checked before it is used.
- **Unknown**: we have no information.

## 1. Scope and ground rules

- Only our own phone, our own desktop install and our own network.
- One link only: official Stream Deck desktop app on Windows to the official
  Stream Deck Mobile app on the phone, both on the same subnet.
- No ARP spoofing on a shared network, no capture of third-party traffic. If the
  network is not isolated, use a hotspot we control (section 2).
- Packet captures contain device names, IP addresses and possibly account data.
  Keep them out of git. `docs/research/` holds notes, never `.pcapng` files.

## 2. Capture path A: Windows mobile hotspot (preferred)

**Verified**: this workstation runs Windows (`multideck-probe env` reports
`os: windows`, `arch: x86_64`).

Why a hotspot: both endpoints end up on an adapter we own, there is no switch,
and no other client shares the segment. That removes the need for port
mirroring and for any disruptive technique.

Steps:

1. Plug the PC into the internet by Ethernet if possible. A hotspot that shares
   Wi-Fi can work but is slower and may drop the capture adapter.
2. Settings, Network and internet, Mobile hotspot. Turn it on. Set the band to
   2.4 GHz if the phone refuses 5 GHz (**Hypothesis**: some phones refuse
   hotspot bands; check the phone's Wi-Fi list after enabling).
3. Note that Windows exposes the hotspot as a virtual adapter named like
   `Local Area Connection* 12` or `Microsoft Wi-Fi Direct Virtual Adapter`.
   **Hypothesis**: the adapter only appears while the hotspot is on, and it can
   take a few seconds after the toggle.
4. Connect the phone to that SSID.
5. In Wireshark, refresh the interface list and capture on that adapter name,
   not on `Wi-Fi`. Capturing the physical Wi-Fi adapter of a hotspot can show
   nothing useful because the hotspot path is bridged inside the OS.

**Hypothesis**: Windows Mobile Hotspot may present a captive-portal style state
to phones when internet sharing is off. If the phone shows "connected, no
internet" the app usually still works, but the OS may route around the hotspot.
Keep internet sharing on to avoid that.

## 3. Capture path B: mirrored switch port

If the hotspot is unusable:

- Managed switch with port mirroring: mirror the desktop port to a second port
  and capture there. Requires hardware we may not have.
- Consumer router: disable AP isolation, put both devices on the same SSID, and
  accept that we only see unicast traffic addressed to or from the desktop
  (**Verified**: without mirroring, a switched LAN does not deliver unrelated
  unicast frames to a third port).

Do not use ARP spoofing on a network that is not ours.

## 4. What to capture

### 4.1 Discovery (mDNS)

**Reported**: the Stream Deck Mobile link advertises itself over mDNS with the
service `_elg._tcp`. This appears in open-source clients of the protocol. We have
not confirmed it.

mDNS is carried in DNS wire format on UDP port 5353, multicast to
`224.0.0.251` and `ff02::fb`. Wireshark dissects it as DNS, so use DNS fields:

```
udp.port == 5353
dns.flags.response == 0 && dns.qry.type == 12          # PTR queries
dns.qry.name contains "elg"
dns.resp.name contains "_elg._tcp"
frame contains "_elg._tcp"                              # raw byte match, case sensitive
```

**Hypothesis**: if the app uses unicast DNS-SD instead of multicast mDNS, the
multicast filter above stays empty and the traffic appears as ordinary UDP to
the desktop IP. Check `udp.port == 5353` alone first.

### 4.2 The session channel

The port is unknown. Do not hardcode one. Start broad:

```
ip.addr == <PHONE_IP>
tcp.flags.syn == 1 && tcp.flags.ack == 0
tcp.port == <PORT_FROM_STEP_ABOVE>
```

Find the phone IP first:

```powershell
arp -a
Get-NetNeighbor -AddressFamily IPv4 | Where-Object State -eq Reachable
```

Cross-check the MAC prefix against the phone's vendor before trusting the entry,
because the hotspot gateway and other clients also appear.

Once a candidate flow exists, look at both directions and the first payload:

- Plain framing: readable bytes at the start of the TCP payload.
- TLS: a `Client Hello` record starting with `16 03 01` or `16 03 03`. If every
  candidate flow starts that way, the channel is encrypted and the gate in
  `sdmobile-protocol.md` applies.

**Reported**: open implementations describe a frame that begins with the magic
bytes `43 93 8A 41` and refer to the framing as CORA. Not confirmed here. Try:

```
tcp contains 43:93:8a:41
```

A match is strong evidence. No match proves nothing while the stream is TLS.

### 4.3 Loopback

**Hypothesis**: the desktop app may run a local helper on `127.0.0.1` and the
mobile link may be proxied through it. Capture the Npcap loopback interface
(named `Adapter for loopback traffic capture` in Wireshark) at the same time as
the hotspot adapter. Npcap must be installed with loopback support; the default
installer asks for it.

### 4.4 Pairing versus session

Capture both, in separate files, with timestamps:

1. Start the capture.
2. Open the desktop app, start the Stream Deck mobile connection flow.
3. Pair the phone (scan the code or enter the PIN, whichever the app offers).
4. Press a few keys from both the phone and the desktop.
5. Change a profile or a key image from the desktop and watch what goes out.
6. Stop the capture.

Name the files with the step range, for example `01-discovery.pcapng`,
`02-pairing.pcapng`, `03-keys.pcapng`. Notes belong next to them in
`ghidra-notes.md` or a local scratch file, not in git.

## 5. Desktop side evidence

### 5.1 Logs

**Hypothesis**: the desktop app writes under `%APPDATA%\Elgato\StreamDeck\logs`,
with older entries rotated. `%LOCALAPPDATA%\Elgato` and `%PROGRAMDATA%\Elgato`
are also worth listing. Verify the path on the actual install before relying on
it.

```powershell
Get-ChildItem "$env:APPDATA\Elgato" -Recurse -Filter *.log -ErrorAction SilentlyContinue |
  Select-Object FullName, Length, LastWriteTime
Get-ChildItem "$env:APPDATA\Elgato" -Recurse -Include *.log |
  Select-String -Pattern "mdns|5353|elg|pair|pin|token" -CaseSensitive:$false
```

If the log directory is empty, capture file writes with Process Monitor instead
of guessing.

**Hypothesis**: a Chromium/Electron style helper logs to
`%LOCALAPPDATA%\<vendor>\<product>\logs`. The install layout has to be inspected
first; see `ghidra-notes.md`.

### 5.2 Sockets and processes

Our own tool, no install needed:

```powershell
cargo run -p multideck-probe -- ports
cargo run -p multideck-probe -- ports --pid <PID>
```

Compare with the OS view:

```powershell
Get-NetTCPConnection -State Listen | Sort-Object LocalPort
netstat -ano | Select-String LISTENING
Get-Process -Id <PID> | Select-Object Id, ProcessName, Path
```

If the app listens on a port only during pairing, run the listing before and
after the pairing flow and diff the two outputs.

## 6. If the channel is TLS

**Reported**: Windows Schannel and most bundled TLS stacks honour
`SSLKEYLOGFILE` only when built to do so. .NET Framework and many native apps
ignore it. Chromium-based helpers do honour it.

If the Client Hello is present but no keylog is produced:

- Check the certificate: a self-signed or vendor certificate suggests a pinned
  or custom trust store. Pinning plus no keylog means we cannot read the
  plaintext by capture alone.
- That is not a failure. It is exactly the input the phase 0 gate in
  `sdmobile-protocol.md` needs: the channel is opaque, so the adapter decision
  changes from "implement the protocol" to "drive the app another way or defer".

Do not spend days trying to defeat pinning on software we do not own. The gate
exists to stop that.

## 7. Static analysis track (runs in parallel)

Dynamic capture answers "what goes on the wire". Static analysis answers "what
does the binary expect", and it is the only path when the channel is opaque.

### 7.1 Tooling

- **Given**: Ghidra 12.1.4, which requires JDK 21. Install a JDK 21 (Temurin is
  fine), set `JAVA_HOME`, and confirm `ghidraRun` starts.
- **Given**: the REA MCP server, started with
  `npx -y rea-agents@6.10.1`. It gives the agent scripted access to Ghidra, so
  findings can be recorded without hand-copying decompiler output.
- Work on a copy of the install directory. Ghidra writes project files, and the
  app may self-update or hold locks on its own binaries.

### 7.2 Procedure

1. Inventory: list every `.exe`, `.dll` and `.node` file with size and version
   resource. Do not import everything; start with the executable that owns the
   mobile link.
2. Cheap pass first, no Ghidra: `multideck-probe strings <file> --grep elg`,
   then `--grep 5353`, `--grep cora`, `--grep pair`. Printable strings often
   locate the protocol code faster than a full auto-analysis.
3. Check for bundled JavaScript or ASAR archives before decompiling native
   code. If the app is Electron-based, the link logic may be readable script,
   and Ghidra is the wrong tool.
4. Import the candidate binary into a new Ghidra project, accept the default
   PE x86-64 analysis.
5. Search for the strings found in step 2, follow references to the functions
   that use them, and decompile only those.
6. Record every claim, with the binary hash and the function address, in
   `ghidra-notes.md`. A finding without an address is a guess.

### 7.3 What to look for

- mDNS service registration (string `_elg._tcp`, `5353`, `224.0.0.251`).
- Socket setup: `WSAStartup`, `bind`, `listen`, `getaddrinfo`, or the
  equivalent in whichever runtime the app uses.
- Frame magic comparison against `0x41938A43` or the byte order the compiler
  would produce.
- Pairing: anything that looks like a key exchange, a PIN, a QR payload or a
  stored secret in a config file.
- TLS configuration: certificate pinning callbacks, custom trust stores,
  `SSL_CTX_set_verify`.

## 8. Definition of done for this playbook

A capture run is complete when we have all of the following:

- [ ] mDNS or unicast discovery from the phone to the desktop, in a pcapng.
- [ ] A TCP or UDP session with both directions present.
- [ ] The desktop log lines that name the phone or the pairing step, with
      timestamps that line up with the capture.
- [ ] A decision on the channel: plaintext framing, TLS with a keylog, or TLS
      without one (opaque).
- [ ] The binary or script that owns the link, identified by path and hash.
- [ ] Notes in `ghidra-notes.md` for every string and function that mattered.

Anything less is a partial run, which is fine, as long as the next section of
`sdmobile-protocol.md` says so.
