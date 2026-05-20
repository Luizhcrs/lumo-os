"""
E2E: lumo-monitor
Steps: launch -> screenshot default tab -> assert no crash
"""

import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from harness import LumoE2E


def test_monitor():
    e2e = LumoE2E()
    e2e.kill_app("lumo-monitor")

    e2e.launch_app("lumo-monitor")
    e2e.assert_no_crash("lumo-monitor")

    time.sleep(0.5)
    shot = e2e.screenshot("monitor_default")

    e2e.assert_no_crash("lumo-monitor")
    e2e.kill_app("lumo-monitor")

    assert shot.exists() and shot.stat().st_size > 1000, "screenshot not saved"
    print(f"  screenshot size={shot.stat().st_size} bytes")


if __name__ == "__main__":
    test_monitor()
    print("PASS")
