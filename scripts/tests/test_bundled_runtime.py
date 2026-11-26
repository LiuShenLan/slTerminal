import hashlib
import struct
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from scripts import check_bundled_runtime


def pe_bytes(machine: int) -> bytes:
    data = bytearray(0x80)
    data[:2] = b"MZ"
    struct.pack_into("<I", data, 0x3C, 0x40)
    data[0x40:0x44] = b"PE\0\0"
    struct.pack_into("<H", data, 0x44, machine)
    return bytes(data)


class BundledRuntimeTests(unittest.TestCase):
    def check(self, files: dict[str, bytes], assets, license_text="Copyright Microsoft Corporation"):
        with tempfile.TemporaryDirectory() as temp:
            directory = Path(temp)
            for name, content in files.items():
                (directory / name).write_bytes(content)
            if license_text is not None:
                (directory / "LICENSE").write_text(license_text, encoding="utf-8")
            with patch.object(check_bundled_runtime, "CONPTY_DIR", directory), patch.object(
                check_bundled_runtime, "ASSETS", assets
            ):
                return check_bundled_runtime.collect_findings()

    def test_matching_hash_and_x64_pe_pass(self):
        content = pe_bytes(check_bundled_runtime.PE_MACHINE_X64)
        digest = hashlib.sha256(content).hexdigest()
        findings = self.check({"conpty.dll": content}, (("conpty.dll", digest),))
        self.assertEqual(findings, [])

    def test_hash_drift_is_reported(self):
        content = pe_bytes(check_bundled_runtime.PE_MACHINE_X64)
        findings = self.check({"conpty.dll": content}, (("conpty.dll", "0" * 64),))
        self.assertTrue(any("SHA256 漂移" in finding for finding in findings))

    def test_non_x64_pe_is_reported(self):
        content = pe_bytes(0xAA64)
        digest = hashlib.sha256(content).hexdigest()
        findings = self.check({"conpty.dll": content}, (("conpty.dll", digest),))
        self.assertTrue(any("架构不是 x64" in finding for finding in findings))

    def test_missing_asset_and_license_are_reported(self):
        findings = self.check({}, (("conpty.dll", "0" * 64),), license_text=None)
        self.assertTrue(any("不存在" in finding for finding in findings))
        self.assertTrue(any("缺少 ConPTY 许可" in finding for finding in findings))


if __name__ == "__main__":
    unittest.main()
