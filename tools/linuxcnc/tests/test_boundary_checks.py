# SPDX-License-Identifier: AGPL-3.0-or-later
"""GC-10/NFR-07: actual paused interpreter state must match the source boundary."""
import copy
import math
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from boundary_checks import verify


class BoundaryChecks(unittest.TestCase):
    def setUp(self):
        self.expected = dict(position=[1, 2, 3, 450], motion=1, feed=60, tool=2,
                             spindle_on=True, rpm=9000)
        self.actual = dict(position=[1, 2, 3, 450], actual_position=[1, 2, 3, 450],
                           gcodes=[10, 170, 210, 540, 610, 900, 940], feed=60,
                           tool=2, spindle_on=True, direction=1, rpm=9000,
                           g5x_offset=[0]*4, g92_offset=[0]*4, rotation_xy=0)

    def test_req_gc_10_the_paused_boundary_matches_coordinates_and_all_modal_state(self):
        verify(self.expected, self.actual)
        # An unset original feed stays unconstrained until its first F word.
        self.expected['feed'] = None
        verify(self.expected, self.actual)

    def test_req_gc_10_a_feed_held_in_single_precision_matches(self):
        # LinuxCNC keeps the feed as a 32-bit float: F2755.6 reads back as
        # 2755.60009765625.
        self.expected['feed'] = 2755.6
        self.actual['feed'] = 2755.60009765625
        verify(self.expected, self.actual)
        # Its neighbouring 32-bit float is still a mismatch.
        self.actual['feed'] = 2755.6003417968750
        with self.assertRaises(ValueError):
            verify(self.expected, self.actual)

    def test_req_gc_10_req_nfr_07_a_position_or_modal_discontinuity_fails(self):
        for key, value in [
            ('position', [1.01, 2, 3, 450]),
            ('actual_position', [1, 2, 3, 450.01]),
            ('gcodes', [10, 170, 210, 540, 610, 910, 940]),
            ('gcodes', [0, 170, 210, 540, 610, 900, 940]),
            ('feed', 61), ('feed', 60.001), ('feed', 60.00001), ('tool', 1), ('spindle_on', False), ('rpm', 9001),
            ('direction', -1), ('g5x_offset', [1, 0, 0, 0]),
            ('g92_offset', [0, 0, 0, 360]), ('rotation_xy', 1),
            ('position', [math.nan, 2, 3, 450]), ('feed', math.inf),
        ]:
            with self.subTest(key=key, value=value):
                actual = copy.deepcopy(self.actual)
                actual[key] = value
                with self.assertRaises(ValueError):
                    verify(self.expected, actual)


if __name__ == '__main__':
    unittest.main()
