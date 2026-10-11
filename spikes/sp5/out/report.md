## Fixtures

### sp3_g94.ngc (318 lines)
- gcode 0.7.0 (f32 values): ok, 318 blocks, 944 words (310 A), 1 comments, max |value - text| = 1.250e-5, matches split oracle: NO (line 6 col 4: expected Z-15.2 at 6:4, got Z-15.199999809265137 at 6:4), 312 moves
- gcode 0.7.0 + f64 layer: ok, 318 blocks, 944 words (310 A), 1 comments, max |value - text| = 0.000e0, matches split oracle: yes, 312 moves
- g-code 0.6.0: ok, 318 blocks, 944 words (310 A), 1 comments, max |value - text| = 0.000e0, matches split oracle: yes, 312 moves
- in-house subset reader: ok, 318 blocks, 944 words (310 A), 1 comments, max |value - text| = 0.000e0, matches split oracle: yes, 312 moves

### sp3_g93.ngc (319 lines)
- gcode 0.7.0 (f32 values): ok, 319 blocks, 948 words (310 A), 1 comments, max |value - text| = 5.859e-5, matches split oracle: NO (line 7 col 4: expected Z-15.2 at 7:4, got Z-15.199999809265137 at 7:4), 312 moves
- gcode 0.7.0 + f64 layer: ok, 319 blocks, 948 words (310 A), 1 comments, max |value - text| = 0.000e0, matches split oracle: yes, 312 moves
- g-code 0.6.0: ok, 319 blocks, 948 words (310 A), 1 comments, max |value - text| = 0.000e0, matches split oracle: yes, 312 moves
- in-house subset reader: ok, 319 blocks, 948 words (310 A), 1 comments, max |value - text| = 0.000e0, matches split oracle: yes, 312 moves

### sp3_naive.ngc (318 lines)
- gcode 0.7.0 (f32 values): ok, 318 blocks, 640 words (310 A), 1 comments, max |value - text| = 1.250e-5, matches split oracle: NO (line 6 col 4: expected Z-15.2 at 6:4, got Z-15.199999809265137 at 6:4), 312 moves
- gcode 0.7.0 + f64 layer: ok, 318 blocks, 640 words (310 A), 1 comments, max |value - text| = 0.000e0, matches split oracle: yes, 312 moves
- g-code 0.6.0: ok, 318 blocks, 640 words (310 A), 1 comments, max |value - text| = 0.000e0, matches split oracle: yes, 312 moves
- in-house subset reader: ok, 318 blocks, 640 words (310 A), 1 comments, max |value - text| = 0.000e0, matches split oracle: yes, 312 moves

### linuxcnc_post_subset.ngc (53 lines)
- gcode 0.7.0 (f32 values): ok, 53 blocks, 152 words (8 A), 7 comments, max |value - text| = 1.891e-4, matches split oracle: NO (line 5 col 37: expected P0.01 at 5:37, got P0.009999999776482582 at 5:37), 17 moves
- gcode 0.7.0 + f64 layer: ok, 53 blocks, 152 words (8 A), 7 comments, max |value - text| = 0.000e0, matches split oracle: yes, 17 moves
- g-code 0.6.0: error (1 diagnostics), first: line 53, column 2: expected EOF
- in-house subset reader: ok, 51 blocks, 152 words (8 A), 7 comments, max |value - text| = 0.000e0, matches split oracle: yes, 17 moves

### linuxcnc_dialect_extra.ngc (13 lines)
- gcode 0.7.0 (f32 values): error (36 diagnostics), first: line 3, column 1: Unknown content: #
- gcode 0.7.0 + f64 layer: error (36 diagnostics), first: line 3, column 1: Unknown content: #
- g-code 0.6.0: error (1 diagnostics), first: line 3, column 1: expected one of EOF, checksum asterisk, letter, letters, newline, opening parenthesis, semicolon, whitespace
- in-house subset reader: error (4 diagnostics), first: line 3, column 1: parameters (`#`) are not part of the post subset

## Replay agreement on the SP-3 outputs

- sp3_g94.ngc, gcode 0.7.0 (f32 values): max position difference 1.250e-5, max feed difference 7.446e-6
- sp3_g94.ngc, gcode 0.7.0 + f64 layer: max position difference 0.000e0, max feed difference 0.000e0
- sp3_g94.ngc, g-code 0.6.0: max position difference 0.000e0, max feed difference 0.000e0
- sp3_g94.ngc, in-house subset reader: max position difference 0.000e0, max feed difference 0.000e0
- sp3_g93.ngc, gcode 0.7.0 (f32 values): max position difference 1.250e-5, max feed difference 5.859e-5
- sp3_g93.ngc, gcode 0.7.0 + f64 layer: max position difference 0.000e0, max feed difference 0.000e0
- sp3_g93.ngc, g-code 0.6.0: max position difference 0.000e0, max feed difference 0.000e0
- sp3_g93.ngc, in-house subset reader: max position difference 0.000e0, max feed difference 0.000e0
- sp3_naive.ngc, gcode 0.7.0 (f32 values): max position difference 1.250e-5, max feed difference 0.000e0
- sp3_naive.ngc, gcode 0.7.0 + f64 layer: max position difference 0.000e0, max feed difference 0.000e0
- sp3_naive.ngc, g-code 0.6.0: max position difference 0.000e0, max feed difference 0.000e0
- sp3_naive.ngc, in-house subset reader: max position difference 0.000e0, max feed difference 0.000e0
- linuxcnc_post_subset.ngc, gcode 0.7.0 (f32 values): max position difference 1.891e-4, max feed difference 0.000e0
- linuxcnc_post_subset.ngc, gcode 0.7.0 + f64 layer: max position difference 0.000e0, max feed difference 0.000e0
- linuxcnc_post_subset.ngc, g-code 0.6.0: not comparable
- linuxcnc_post_subset.ngc, in-house subset reader: max position difference 0.000e0, max feed difference 0.000e0

## Precision on stress values

- gcode 0.7.0 (f32 values): worst |value - text| = 2.413e-3 at A98765.4321
- gcode 0.7.0 + f64 layer: worst |value - text| = 0.000e0 at 
- g-code 0.6.0: worst |value - text| = 0.000e0 at 
- in-house subset reader: worst |value - text| = 0.000e0 at 

## Malformed input

- `G1 X+-1.0` (sign then sign)
  - gcode 0.7.0 (f32 values): error at line 1, column 4: Unexpected: X (expected: number)
  - gcode 0.7.0 + f64 layer: error at line 1, column 4: Unexpected: X (expected: number)
  - g-code 0.6.0: PANIC
  - in-house subset reader: error at line 1, column 4: `X` needs a number, found `+`
- `G1 X--1` (double minus)
  - gcode 0.7.0 (f32 values): error at line 1, column 4: Unexpected: X (expected: number)
  - gcode 0.7.0 + f64 layer: error at line 1, column 4: Unexpected: X (expected: number)
  - g-code 0.6.0: error at line 1, column 6: expected one of decimal point, integer
  - in-house subset reader: error at line 1, column 4: `X` needs a number, found `-`
- `G1 X` (letter with no number)
  - gcode 0.7.0 (f32 values): error at line 1, column 4: Unexpected: X (expected: number)
  - gcode 0.7.0 + f64 layer: error at line 1, column 4: Unexpected: X (expected: number)
  - g-code 0.6.0: error at line 1, column 4: letter with no value
  - in-house subset reader: error at line 1, column 4: `X` needs a number, found ``
- `G1 X1.2.3` (two decimal points)
  - gcode 0.7.0 (f32 values): error at line 1, column 8: Unexpected: .3 (expected: letter)
  - gcode 0.7.0 + f64 layer: error at line 1, column 8: Unexpected: .3 (expected: letter)
  - g-code 0.6.0: error at line 1, column 8: expected one of EOF, checksum asterisk, letter, letters, newline, opening parenthesis, semicolon, whitespace
  - in-house subset reader: error at line 1, column 4: `X` needs a number, found `1.2.3`
- `G1 X1 (no close` (unclosed comment)
  - gcode 0.7.0 (f32 values): accepted: ["G1", "X1"]
  - gcode 0.7.0 + f64 layer: accepted: ["G1", "X1"]
  - g-code 0.6.0: error at line 1, column 16: expected closing parenthesis
  - in-house subset reader: error at line 1, column 7: comment not closed with `)`
- `G1 X1 (Ä°)` (non-ASCII in a comment)
  - gcode 0.7.0 (f32 values): accepted: ["G1", "X1"]
  - gcode 0.7.0 + f64 layer: accepted: ["G1", "X1"]
  - g-code 0.6.0: error at line 1, column 8: expected one of closing parenthesis, opening parenthesis
  - in-house subset reader: accepted: ["G1", "X1"]
- `G1 X1 Ä` (non-ASCII outside a comment)
  - gcode 0.7.0 (f32 values): error at line 1, column 7: Unknown content: Ä
  - gcode 0.7.0 + f64 layer: error at line 1, column 7: Unknown content: Ä
  - g-code 0.6.0: error at line 1, column 7: expected one of EOF, checksum asterisk, letter, letters, newline, opening parenthesis, semicolon, whitespace
  - in-house subset reader: error at line 1, column 7: unexpected character `Ä`
- `G1 X1\nA2\n` (CRLF line endings)
  - gcode 0.7.0 (f32 values): accepted: ["G1", "X1", "A2"]
  - gcode 0.7.0 + f64 layer: accepted: ["G1", "X1", "A2"]
  - g-code 0.6.0: accepted: ["G1", "X1", "A2"]
  - in-house subset reader: accepted: ["G1", "X1", "A2"]
- `G1 X1e400` (huge number)
  - gcode 0.7.0 (f32 values): accepted: ["G1", "X1", "E400"]
  - gcode 0.7.0 + f64 layer: accepted: ["G1", "X1", "E400"]
  - g-code 0.6.0: accepted: ["G1", "X1", "E400"]
  - in-house subset reader: accepted: ["G1", "X1", "E400"]
- `G1 X123456789012345678901234567890.5` (very long number)
  - gcode 0.7.0 (f32 values): accepted: ["G1", "X123456789182729270000000000000"]
  - gcode 0.7.0 + f64 layer: accepted: ["G1", "X123456789012345680000000000000"]
  - g-code 0.6.0: error at line 1, column 37: expected cannot parse as decimal (unable to display root cause)
  - in-house subset reader: accepted: ["G1", "X123456789012345680000000000000"]
- `g1 x1.5 a-2` (lowercase)
  - gcode 0.7.0 (f32 values): accepted: ["G1", "X1.5", "A-2"]
  - gcode 0.7.0 + f64 layer: accepted: ["G1", "X1.5", "A-2"]
  - g-code 0.6.0: accepted: ["G1", "X1.5", "A-2"]
  - in-house subset reader: accepted: ["G1", "X1.5", "A-2"]
- `G1X1.5A-2` (no space between words)
  - gcode 0.7.0 (f32 values): accepted: ["G1", "X1.5", "A-2"]
  - gcode 0.7.0 + f64 layer: accepted: ["G1", "X1.5", "A-2"]
  - g-code 0.6.0: accepted: ["G1", "X1.5", "A-2"]
  - in-house subset reader: accepted: ["G1", "X1.5", "A-2"]
- `G 1 X 1 . 5` (spaces inside a word)
  - gcode 0.7.0 (f32 values): error at line 1, column 9: Unexpected: . (expected: letter)
  - gcode 0.7.0 + f64 layer: error at line 1, column 9: Unexpected: . (expected: letter)
  - g-code 0.6.0: error at line 1, column 3: expected one of EOF, checksum asterisk, letter, letters, newline, opening parenthesis, semicolon, whitespace
  - in-house subset reader: accepted: ["G1", "X1.5"]
- `G1.25 X1` (two-digit G minor)
  - gcode 0.7.0 (f32 values): accepted: ["X1"]
  - gcode 0.7.0 + f64 layer: accepted: ["X1"]
  - g-code 0.6.0: accepted: ["G1.25", "X1"]
  - in-house subset reader: accepted: ["G1.25", "X1"]
- `G99999999999 X1` (huge G number)
  - gcode 0.7.0 (f32 values): accepted: ["X1"]
  - gcode 0.7.0 + f64 layer: accepted: ["X1"]
  - g-code 0.6.0: accepted: ["G99999999999", "X1"]
  - in-house subset reader: accepted: ["G99999999999", "X1"]
- `N99999999999 G1 X1` (huge line number)
  - gcode 0.7.0 (f32 values): accepted: ["G1", "X1"]
  - gcode 0.7.0 + f64 layer: accepted: ["G1", "X1"]
  - g-code 0.6.0: accepted: ["N99999999999", "G1", "X1"]
  - in-house subset reader: accepted: ["N99999999999", "G1", "X1"]
- `G01 X01.500` (leading zeros)
  - gcode 0.7.0 (f32 values): accepted: ["G1", "X1.5"]
  - gcode 0.7.0 + f64 layer: accepted: ["G1", "X1.5"]
  - g-code 0.6.0: accepted: ["G1", "X1.5"]
  - in-house subset reader: accepted: ["G1", "X1.5"]
- `%\nG1 X1\n%\n` (file ending in % and a newline)
  - gcode 0.7.0 (f32 values): accepted: ["G1", "X1"]
  - gcode 0.7.0 + f64 layer: accepted: ["G1", "X1"]
  - g-code 0.6.0: error at line 3, column 2: expected EOF
  - in-house subset reader: accepted: ["G1", "X1"]
- `%\nG1 X1\n%` (file ending in % without a newline)
  - gcode 0.7.0 (f32 values): accepted: ["G1", "X1"]
  - gcode 0.7.0 + f64 layer: accepted: ["G1", "X1"]
  - g-code 0.6.0: accepted: ["G1", "X1"]
  - in-house subset reader: accepted: ["G1", "X1"]
- `/G0 X99` (block delete)
  - gcode 0.7.0 (f32 values): error at line 1, column 1: Unexpected: / (expected: letter, comment)
  - gcode 0.7.0 + f64 layer: error at line 1, column 1: Unexpected: / (expected: letter, comment)
  - g-code 0.6.0: error at line 1, column 1: expected one of EOF, checksum asterisk, letter, letters, newline, opening parenthesis, percent sign, semicolon, whitespace
  - in-house subset reader: accepted: ["G0", "X99"]

## Robustness (mutation fuzz, no panics allowed: NFR-07)

- gcode 0.7.0 (f32 values): 20000 inputs, 0 panics, 12801 errors
- gcode 0.7.0 + f64 layer: 20000 inputs, 0 panics, 12801 errors
- g-code 0.6.0: 20000 inputs, 7 panics, 13476 errors, e.g. `G0 A+-90.0000\nM5`
- in-house subset reader: 20000 inputs, 0 panics, 12755 errors

## LinuxCNC dialect features beyond the post subset, line by line

- `#1 = 5.0`
  - gcode 0.7.0 (f32 values): error at line 1, column 1: Unknown content: #
  - gcode 0.7.0 + f64 layer: error at line 1, column 1: Unknown content: #
  - g-code 0.6.0: error at line 1, column 1: expected one of EOF, checksum asterisk, letter, letters, newline, opening parenthesis, percent sign, semicolon, whitespace
  - in-house subset reader: error at line 1, column 1: parameters (`#`) are not part of the post subset
- `#<depth> = -1.5`
  - gcode 0.7.0 (f32 values): error at line 1, column 1: Unknown content: #<
  - gcode 0.7.0 + f64 layer: error at line 1, column 1: Unknown content: #<
  - g-code 0.6.0: error at line 1, column 1: expected one of EOF, checksum asterisk, letter, letters, newline, opening parenthesis, percent sign, semicolon, whitespace
  - in-house subset reader: error at line 1, column 1: parameters (`#`) are not part of the post subset
- `G1 X#1 Y[2 * 3] Z#<depth> F100`
  - gcode 0.7.0 (f32 values): error at line 1, column 4: Unexpected: X (expected: number)
  - gcode 0.7.0 + f64 layer: error at line 1, column 4: Unexpected: X (expected: number)
  - g-code 0.6.0: error at line 1, column 5: expected one of EOF, checksum asterisk, decimal point, integer, letter, letters, minus sign, newline, opening parenthesis, plus sign, quotation mark, semicolon, whitespace
  - in-house subset reader: error at line 1, column 4: `X` needs a number, found ``
- `g1 x1.0 y2.0 a3.0`
  - gcode 0.7.0 (f32 values): accepted: ["G1", "X1", "Y2", "A3"]
  - gcode 0.7.0 + f64 layer: accepted: ["G1", "X1", "Y2", "A3"]
  - g-code 0.6.0: accepted: ["G1", "X1", "Y2", "A3"]
  - in-house subset reader: accepted: ["G1", "X1", "Y2", "A3"]
- `G 1 X 1 . 5 A 2 0`
  - gcode 0.7.0 (f32 values): error at line 1, column 9: Unexpected: . (expected: letter)
  - gcode 0.7.0 + f64 layer: error at line 1, column 9: Unexpected: . (expected: letter)
  - g-code 0.6.0: error at line 1, column 3: expected one of EOF, checksum asterisk, letter, letters, newline, opening parenthesis, semicolon, whitespace
  - in-house subset reader: accepted: ["G1", "X1.5", "A20"]
- `/ G0 X99`
  - gcode 0.7.0 (f32 values): error at line 1, column 1: Unexpected: / (expected: letter, comment)
  - gcode 0.7.0 + f64 layer: error at line 1, column 1: Unexpected: / (expected: letter, comment)
  - g-code 0.6.0: error at line 1, column 1: expected one of EOF, checksum asterisk, letter, letters, newline, opening parenthesis, percent sign, semicolon, whitespace
  - in-house subset reader: accepted: ["G0", "X99"]
- `o100 sub`
  - gcode 0.7.0 (f32 values): error at line 1, column 6: Unexpected: s (expected: number)
  - gcode 0.7.0 + f64 layer: error at line 1, column 6: Unexpected: s (expected: number)
  - g-code 0.6.0: error at line 1, column 6: letter with no value
  - in-house subset reader: error at line 1, column 1: O-words are not part of the post subset
- `G38.2 Z-10 F50`
  - gcode 0.7.0 (f32 values): accepted: ["G38.2", "Z-10", "F50"]
  - gcode 0.7.0 + f64 layer: accepted: ["G38.2", "Z-10", "F50"]
  - g-code 0.6.0: accepted: ["G38.2", "Z-10", "F50"]
  - in-house subset reader: accepted: ["G38.2", "Z-10", "F50"]
- `G33.1 Z-5 K1.25`
  - gcode 0.7.0 (f32 values): accepted: ["G33.1", "Z-5", "K1.25"]
  - gcode 0.7.0 + f64 layer: accepted: ["G33.1", "Z-5", "K1.25"]
  - g-code 0.6.0: accepted: ["G33.1", "Z-5", "K1.25"]
  - in-house subset reader: accepted: ["G33.1", "Z-5", "K1.25"]
- `G10 L2 P1 X0 Y0 Z0`
  - gcode 0.7.0 (f32 values): accepted: ["G10", "L2", "P1", "X0", "Y0", "Z0"]
  - gcode 0.7.0 + f64 layer: accepted: ["G10", "L2", "P1", "X0", "Y0", "Z0"]
  - g-code 0.6.0: accepted: ["G10", "L2", "P1", "X0", "Y0", "Z0"]
  - in-house subset reader: accepted: ["G10", "L2", "P1", "X0", "Y0", "Z0"]
- `M30`
  - gcode 0.7.0 (f32 values): accepted: ["M30"]
  - gcode 0.7.0 + f64 layer: accepted: ["M30"]
  - g-code 0.6.0: accepted: ["M30"]
  - in-house subset reader: accepted: ["M30"]

## Speed (1,000,000-line SP-3-style program, release build)

- input 31.8 MB
- gcode 0.7.0 (f32 values): 1.647 s, 1000004 blocks
- gcode 0.7.0 + f64 layer: 1.274 s, 1000004 blocks
- g-code 0.6.0: 2.790 s, 1000004 blocks
- in-house subset reader: 0.532 s, 1000004 blocks
