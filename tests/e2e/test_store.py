"""
E2E: lumo-store
Steps: launch -> screenshot home -> assert no crash
"""

import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from harness import LumoE2E


def test_store():
    e2e = LumoE2E()
    e2e.kill_app("lumo-store")

    e2e.launch_app("lumo-store")
    e2e.assert_no_crash("lumo-store")

    time.sleep(0.5)
    home = e2e.screenshot("store_home")

    e2e.assert_no_crash("lumo-store")
    e2e.kill_app("lumo-store")

    assert home.exists() and home.stat().st_size > 1000, "store screenshot not saved"
    print(f"  screenshot size={home.stat().st_size} bytes")


if __name__ == "__main__":
    test_store()
    print("PASS")
