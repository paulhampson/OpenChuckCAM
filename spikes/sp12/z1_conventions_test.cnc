(OpenChuckCAM spike SP-12: Makera Z1 rotary conventions and G93 test)
(Hand-written and checked by hand and in the LinuxCNC simulator. Not post output.)
(AIR CUT ONLY: no tool in the spindle, no material, spindle off, attended.)
(Follow spikes/sp12/AIR-CUT.md. Verify this file yourself before running it.)
(G54 must put Y0 Z0 on the A axis centreline and X0 at least 10 mm clear of the jaws.)
(The program uses X0 to X60, and Z30 and Z60 above the axis.)
(Comments sit on their own lines: the firmware drops the rest of a line after a comment.)
G90 G21 G17 G94
G54
M5
G53 G0 Z-3
(Step 0: drop whole turns left on A by an earlier job, then go to A0 X0 Y0 Z30)
G92.4 A0 S0
G1 A0 F600
G1 X0 Y0 F1000
G1 Z30 F600
(Step 1: axis letter and direction. Tape mark on top of a jaw. Resume to turn A to +90.)
M600
G1 A90 F600
(Expect DRO A90. Record which way the mark went: toward the front, or toward the back.)
M600
G1 A0 F600
(Step 2: wrap. A370, then G92.4 A0 S0 should change the DRO to A10 without moving.)
M600
G1 A370 F1200
G92.4 A0 S0
(Expect DRO A10, no motion. Resume: A should turn 10 degrees back, not 370.)
M600
G1 A0 F600
G1 A-370 F1200
G92.4 A0 S0
(Expect DRO A-10, no motion. Resume: A should turn 10 degrees forward.)
M600
G1 A0 F600
(Step 3: timing at Z30. Start a stopwatch when each move starts and stop it when it ends.)
(T3a: X only. Expect 12.0 s.)
M600
G1 X60 F300
G1 X0 F1000
(T3b: A only, 360 degrees at radius 30. Z1 rule 21.8 s, LinuxCNC rule 36.0 s.)
M600
G1 A360 F600
(T3c: X20 and A plus 180 together. Z1 rule 10.9 s, LinuxCNC rule 2.0 s.)
M600
G1 X20 A540 F600
G1 X0 F1000
(T3d: X60 and A plus 30 together. Expect 12.0 s on either rule.)
M600
G1 X60 A570 F300
G1 X0 F1000
(Expect DRO A210 after the next line, no motion.)
G92.4 A0 S0
(Step 4: A only at radius 60. Skip to step 5 if Z60 above the axis is not clear.)
(T4: expect 36.0 s on either rule.)
G1 Z60 F600
M600
G1 A570 F600
G1 Z30 F600
(Step 5: G93. T5: 10.0 s if G93 works; 100 s if it is ignored and F6 is read as mm/min.)
(If X is still crawling after 20 s, feed hold is fine: the answer is already clear.)
M600
G93
G1 X10 F6
G94
G1 X0 F1000
(End: unwind A to zero, retract, go to clearance, end of program.)
G92.4 A0 S0
G1 A0 F600
G53 G0 Z-3
M5
G28
M2
