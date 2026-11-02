# SPDX-License-Identifier: AGPL-3.0-or-later
"""GC-10: check the actual paused interpreter at each original-source boundary."""
import math
import struct


def snapshot(stat):
    return dict(position=list(stat.position[:4]),
                actual_position=list(stat.actual_position[:4]),
                gcodes=list(stat.gcodes[1:]), feed=stat.settings[1],
                rpm=stat.settings[2], tool=stat.tool_in_spindle,
                spindle_on=bool(stat.spindle[0]['enabled']),
                direction=stat.spindle[0]['direction'],
                g5x_offset=list(stat.g5x_offset[:4]),
                g92_offset=list(stat.g92_offset[:4]), rotation_xy=stat.rotation_xy)



def single(value):
    """`value` rounded to the nearest 32-bit float."""
    return struct.unpack('f', struct.pack('f', value))[0]


def verify(expected, actual):
    for key in ('position', 'actual_position'):
        values = actual[key]
        if len(values) != 4 or any(not math.isfinite(v) or abs(v - e) > 0.001
                                    for v, e in zip(values, expected['position'])):
            raise ValueError(f'{key}: expected {expected["position"]}, got {values}')
    required = [expected['motion'] * 10, 170, 210, 540, 610, 900, 940]
    if any(code not in actual['gcodes'] for code in required):
        raise ValueError(f'G modes: expected {required}, got {actual["gcodes"]}')
    for key in ('tool', 'spindle_on'):
        if actual[key] != expected[key]:
            raise ValueError(f'{key}: expected {expected[key]}, got {actual[key]}')
    if actual['direction'] != (1 if expected['spindle_on'] else 0):
        raise ValueError('spindle direction does not match M3/M5')
    for key in ('g5x_offset', 'g92_offset'):
        values = actual[key]
        if len(values) != 4 or any(not math.isfinite(v) or abs(v) > 1e-9 for v in values):
            raise ValueError(f'unexpected {key}: {values}')
    if not math.isfinite(actual['rotation_xy']) or abs(actual['rotation_xy']) > 1e-9:
        raise ValueError('unexpected coordinate rotation')
    # LinuxCNC may hold the feed and speed as 32-bit floats: F2755.6 reads
    # back as 2755.60009765625, so the value also matches its single-precision
    # rounding.
    for key in ('feed', 'rpm'):
        want = expected[key]
        value = actual[key]
        if want is not None and (not math.isfinite(value)
                                 or min(abs(value - want), abs(value - single(want))) > 1e-6):
            raise ValueError(f'{key}: expected {want}, got {value}')
