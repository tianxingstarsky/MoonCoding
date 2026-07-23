# AGENTS.md — MoonCoding

## Product

MoonCoding is a human-directed desktop/board agent: humans own the project tree;
AI fills nodes. Protocol safety (vibe blocks, rev, purpose) beats convenience.

## UI stack (desktop and board)

- GUI is **Qt 6 / C++** (`vibe-ui`) on **both** developer desktops and Luckfox Lyra boards.
- Do **not** port the product to Qt 5. Board Buildroot must enable **Qt6** (`BR2_PACKAGE_QT6*`,
  Widgets + linuxfb). Qt5 and Qt6 are mutually exclusive in Luckfox Buildroot.
- Full product on board: Chat, Tree, Apps, Settings, RustBridge — same source tree as desktop.
- Board display: **native portrait 720×1280** on linuxfb (no software rotation). Touch via
  `evdevtouch` + `/dev/input/event0`. Script: `scripts/lyra-run-mooncoding.sh`.
  Deploy CJK fonts under `/root/mooncoding/fonts/` (Noto Sans SC / simhei) + `fonts.conf`.
  Fullscreen on board (`MOONCODING_BOARD=1`). Desktop preview: `--ui-profile portrait`.

## Board build / deploy (Lyra RK3506B)

- SDK lunch may use **Zero-W** Buildroot for **Qt6 userspace/sysroot** (same RK3506B).
  Do **not** flash Zero-W full firmware onto Lyra Pi W (different DTS/partitions/DSI).
- Prefer: extract Qt6 libs from Buildroot target/sysroot → **adb push** onto the running Pi W image.
- Cross-build output dir: `build-board/` (toolchain: `cmake/lyra-rk3506-toolchain.cmake`).
- Scripts: `scripts/lyra-cross-build.sh`, `scripts/lyra-adb-deploy.ps1`.
- WSL SDK path (dev): `~/Lyra-sdk`; Qt6 build log: `~/lyra-qt6-buildroot.log`.

## Extension slots

New features go through Provider / Tool / AgentEvent / Session / ProjectTree / Prompt slots.
Do not rewrite the agent main loop for one-off board hacks.

## Coding norms

- No `unwrap` in Rust agent code — `anyhow::Result` + `?`.
- Tool output truncated; prefer evidence commands on tree nodes.
- Module work should be independently verifiable.
