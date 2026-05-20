"""
E2E: lumo-settings
Steps: launch -> screenshot home -> click second tab -> screenshot tab
"""

import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from harness import LumoE2E


def test_settings():
    e2e = LumoE2E()
    e2e.kill_app("lumo-settings")

    e2e.launch_app("lumo-settings")
    e2e.assert_no_crash("lumo-settings")

    home = e2e.screenshot("settings_home")

    # Settings sidebar tabs on left; first tab active by default.
    # Click second tab (y offset ~60px below first)
    # Window center ~960,540; sidebar left ~700; first tab ~440, second ~500
    e2e.click(720, 500)
    time.sleep(0.4)

    tab2 = e2e.screenshot("settings_tab2")
    diff = e2e.screenshot_diff(home, tab2)

    e2e.assert_no_crash("lumo-settings")
    e2e.kill_app("lumo-settings")

    assert home.exists() and home.stat().st_size > 1000, "settings screenshot not saved"
    print(f"  tab click diff={diff:.2f}%")


if __name__ == "__main__":
    test_settings()
    print("PASS")
