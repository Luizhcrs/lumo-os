"""
Lumo OS E2E Test Harness
Bridge: http://127.0.0.1:7778
Auth:   Bearer token from ~/.config/lumo/bridge-token
"""

import os
import re
import signal
import subprocess
import sys
import time
from pathlib import Path

import requests
from PIL import Image, ImageChops


BRIDGE_URL = "http://127.0.0.1:7778"
TOKEN_PATH = Path.home() / ".config/lumo/bridge-token"
BINARY_DIR = Path.home() / "Projects/lumo-shell/target/release"
SCREENSHOT_DIR = Path("/tmp/e2e")

WAYLAND_ENV = {
    "XDG_RUNTIME_DIR": "/run/user/1000",
    "WAYLAND_DISPLAY": "wayland-1",
    "DBUS_SESSION_BUS_ADDRESS": "unix:path=/run/user/1000/bus",
    "HOME": str(Path.home()),
    "USER": os.environ.get("USER", "luizhcrds"),
    "XDG_CURRENT_DESKTOP": "lumo",
    "PATH": os.environ.get("PATH", "/usr/local/bin:/usr/bin:/bin"),
}


class LumoBridgeError(RuntimeError):
    pass


class LumoE2E:
    def __init__(self):
        token_raw = TOKEN_PATH.read_text().strip()
        self._token = token_raw
        self._session = requests.Session()
        self._session.headers["Authorization"] = f"Bearer {token_raw}"
        self._session.headers["Content-Type"] = "application/json"
        SCREENSHOT_DIR.mkdir(parents=True, exist_ok=True)
        self._procs: dict[str, subprocess.Popen] = {}

    # ------------------------------------------------------------------
    # App lifecycle
    # ------------------------------------------------------------------

    def launch_app(self, app_name: str) -> None:
        binary = BINARY_DIR / app_name
        if not binary.exists():
            raise FileNotFoundError(f"Binary not found: {binary}")
        env = dict(os.environ)
        env.update(WAYLAND_ENV)
        log_path = Path(f"/tmp/e2e/{app_name}.log")
        log_fh = log_path.open("w")
        proc = subprocess.Popen(
            [str(binary)],
            env=env,
            stdout=log_fh,
            stderr=log_fh,
            start_new_session=True,
        )
        self._procs[app_name] = proc
        time.sleep(1.5)

    def kill_app(self, app_name: str) -> None:
        proc = self._procs.pop(app_name, None)
        if proc is not None:
            try:
                proc.terminate()
                proc.wait(timeout=3)
            except subprocess.TimeoutExpired:
                proc.kill()
        # also kill any stray process
        subprocess.run(["pkill", "-x", app_name], check=False,
                       capture_output=True)
        time.sleep(0.3)

    def assert_no_crash(self, app_name: str) -> None:
        proc = self._procs.get(app_name)
        if proc is None:
            # not tracked; check by name
            r = subprocess.run(["pgrep", "-x", app_name],
                               capture_output=True)
            if r.returncode != 0:
                raise AssertionError(f"{app_name} is not running (crashed?)")
            return
        ret = proc.poll()
        if ret is not None:
            log = Path(f"/tmp/e2e/{app_name}.log").read_text(errors="replace")
            raise AssertionError(
                f"{app_name} exited with code {ret}\nLog tail:\n{log[-800:]}"
            )

    # ------------------------------------------------------------------
    # Input
    # ------------------------------------------------------------------

    def click(self, x: float, y: float, button: str = "left") -> None:
        self._post("/pointer/click", {"x": x, "y": y, "button": button})

    def move_mouse(self, x: float, y: float) -> None:
        self._post("/pointer/move", {"x": x, "y": y})

    def type_text(self, text: str) -> None:
        self._post("/keyboard/type", {"text": text})

    def press_key(self, key: str) -> None:
        self._post("/keyboard/key", {"sequence": key})

    # ------------------------------------------------------------------
    # Screenshot
    # ------------------------------------------------------------------

    def screenshot(self, name: str) -> Path:
        resp = self._session.get(f"{BRIDGE_URL}/screenshot", timeout=10)
        if resp.status_code != 200:
            raise LumoBridgeError(
                f"screenshot failed: {resp.status_code} {resp.text[:200]}"
            )
        path = SCREENSHOT_DIR / f"{name}.png"
        path.write_bytes(resp.content)
        return path

    def screenshot_diff(self, before: Path, after: Path) -> float:
        img_a = Image.open(before).convert("RGB")
        img_b = Image.open(after).convert("RGB")
        if img_a.size != img_b.size:
            return 100.0
        diff = ImageChops.difference(img_a, img_b)
        pixels = list(diff.getdata())
        changed = sum(1 for p in pixels if any(c > 10 for c in p))
        total = len(pixels)
        return (changed / total) * 100.0 if total > 0 else 0.0

    # ------------------------------------------------------------------
    # Log
    # ------------------------------------------------------------------

    def wait_for_log_line(
        self,
        log_path: str,
        pattern: str,
        timeout: float = 5.0,
    ) -> str:
        rx = re.compile(pattern)
        deadline = time.monotonic() + timeout
        seen_lines: set[str] = set()
        while time.monotonic() < deadline:
            try:
                content = Path(log_path).read_text(errors="replace")
            except FileNotFoundError:
                time.sleep(0.2)
                continue
            for line in content.splitlines():
                if line not in seen_lines and rx.search(line):
                    return line
                seen_lines.add(line)
            time.sleep(0.2)
        raise TimeoutError(
            f"pattern {pattern!r} not found in {log_path} within {timeout}s"
        )

    # ------------------------------------------------------------------
    # Internal
    # ------------------------------------------------------------------

    def _post(self, path: str, payload: dict) -> dict:
        resp = self._session.post(f"{BRIDGE_URL}{path}", json=payload, timeout=5)
        if resp.status_code not in (200, 201):
            raise LumoBridgeError(
                f"POST {path} -> {resp.status_code}: {resp.text[:300]}"
            )
        return resp.json()

    def _get(self, path: str, **params) -> dict:
        resp = self._session.get(f"{BRIDGE_URL}{path}", params=params, timeout=5)
        if resp.status_code not in (200, 201):
            raise LumoBridgeError(
                f"GET {path} -> {resp.status_code}: {resp.text[:300]}"
            )
        return resp.json()
