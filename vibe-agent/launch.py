#!/usr/bin/env python3
"""MoonCoding vibe-agent launcher — sets env vars and starts the TUI."""
import os, subprocess, sys

PROJECT = os.path.dirname(os.path.abspath(__file__))
VIBE_AGENT = os.path.join(PROJECT, "target", "release", "vibe-agent.exe")
WORKSPACE = os.path.join(PROJECT, "..", "test_project")

# ── API key ──
key = os.environ.get("DEEPSEEK_API_KEY", "")
if not key:
    key = os.environ.get("MOONCODING_API_KEY", "")
if not key:
    key = os.environ.get("OPENAI_API_KEY", "")
if not key:
    print("no API key found. set DEEPSEEK_API_KEY env var.")
    print("  e.g.: $env:DEEPSEEK_API_KEY = 'sk-...'")
    sys.exit(1)

# ── model (optional) ──
model = os.environ.get("MOONCODING_MODEL", "deepseek-v4-flash")
base_url = os.environ.get("MOONCODING_BASE_URL", "https://api.deepseek.com")

# ── ensure vibe is built ──
if not os.path.exists(VIBE_AGENT):
    print("building vibe-agent...")
    subprocess.run(["cargo", "build", "--release"], cwd=PROJECT, check=True)

# ── launch ──
env = os.environ.copy()
env["DEEPSEEK_API_KEY"] = key
env["MOONCODING_MODEL"] = model
env["MOONCODING_BASE_URL"] = base_url

cmd = [VIBE_AGENT, "-C", WORKSPACE, "tui"]
if len(sys.argv) > 1:
    cmd = [VIBE_AGENT, "-C", WORKSPACE] + sys.argv[1:]

subprocess.run(cmd, env=env)