import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from regex_baseline.detect import detect  # noqa: E402
from regex_baseline.vault import Vault  # noqa: E402


def labels(text: str) -> list[tuple[str, str]]:
    return [(m.label, text[m.start:m.end]) for m in detect(text)]


class DetectTest(unittest.TestCase):
    def test_checksummed_ids(self) -> None:
        self.assertEqual(labels("Aadhaar 2345 6789 0124 ok"), [("AADHAAR", "2345 6789 0124")])
        self.assertEqual(labels("Aadhaar 2345 6789 0125 ok"), [])
        self.assertEqual(labels("card 4111-1111-1111-1111"), [("CARD", "4111-1111-1111-1111")])
        self.assertEqual(labels("GSTIN 27AAPFU0939F1ZV."), [("GSTIN", "27AAPFU0939F1ZV")])

    def test_email_wins_over_upi_and_upi_alone_is_found(self) -> None:
        self.assertEqual(labels("mail rahul.s@gmail.com"), [("EMAIL", "rahul.s@gmail.com")])
        self.assertEqual(labels("pay rahul.s@okaxis now"), [("UPI", "rahul.s@okaxis")])
        self.assertEqual(labels("pay rahul.s@okaxis."), [("UPI", "rahul.s@okaxis")])

    def test_phone_with_country_code(self) -> None:
        self.assertEqual(labels("call +91 98765 43210."), [("PHONE", "+91 98765 43210")])

    def test_offsets_are_code_points_after_devanagari(self) -> None:
        text = "मेरा पैन ABCPS1234K है"
        [m] = detect(text)
        self.assertEqual(text[m.start:m.end], "ABCPS1234K")


class VaultTest(unittest.TestCase):
    def test_mask_unmask_round_trip_reuses_placeholders(self) -> None:
        v = Vault()
        masked = v.mask("s", "PAN ABCPS1234K and again ABCPS1234K")
        self.assertEqual(masked, "PAN [PAN_1] and again [PAN_1]")
        self.assertEqual(v.unmask("s", "ok [PAN_1]"), "ok ABCPS1234K")

    def test_sessions_are_isolated(self) -> None:
        v = Vault()
        v.mask("a", "PAN ABCPS1234K")
        self.assertEqual(v.unmask("b", "[PAN_1]"), "[PAN_1]")

    def test_stream_holds_back_split_placeholder(self) -> None:
        v = Vault()
        v.mask("s", "PAN ABCPS1234K")
        out = [v.unmask_stream("s", "Your PAN is [PA", False), v.unmask_stream("s", "N_1] ok", False),
               v.unmask_stream("s", "", True)]
        self.assertEqual(out, ["Your PAN is ", "ABCPS1234K ok", ""])

    def test_stream_releases_unrelated_bracket_at_end(self) -> None:
        v = Vault()
        self.assertEqual(v.unmask_stream("s", "see [note", False), "see ")
        self.assertEqual(v.unmask_stream("s", "", True), "[note")


if __name__ == "__main__":
    unittest.main()
