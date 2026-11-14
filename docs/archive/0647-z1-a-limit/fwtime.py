#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
"""Times a Makera Z1 program as the firmware model of decision record 0647 does.

Fed moves: max(L, dA*P/360)/F (or dA*min(P,360)/360/F for A alone), P = 2 pi r + 30
(decision 0007), then no axis faster than X/Y 1200, Z 600 mm/min and A at the given
limit (deg/min). Rapids: a rough model (the XYZ line at 1200 mm/min, A at its limit),
for comparison only. Usage: python3 fwtime.py <A limit> <file or main:path>...
"""
import math
import subprocess
import sys

def run(text,alim):
    pos=[0,0,0,0];f=0;mode=None;tot=0;rap=0
    for line in text.splitlines():
        if line.startswith('('):continue
        w=line.split()
        if 'G92.4' in w:
            for x in w:
                if x[0]=='A':pos[3]=float(x[1:])
            continue
        if any(x in ('G28','G53') for x in w): break
        if 'G0' in w:mode=0
        if 'G1' in w:mode=1
        to=pos[:]
        for x in w:
            if x[0]=='F':f=float(x[1:])
            for i,c in enumerate('XYZA'):
                if x[0]==c:to[i]=float(x[1:])
        if to!=pos:
            d=[abs(a-b) for a,b in zip(pos,to)]
            L=math.sqrt(d[0]**2+d[1]**2+d[2]**2)
            axes=60*max(d[0]/1200,d[1]/1200,d[2]/600,d[3]/alim)
            if mode==1:
                P=2*math.pi*math.hypot(to[1],to[2])+30
                c=max(L,d[3]*P/360) if L>0 else d[3]*min(P,360)/360
                tot+=max(60*c/f,axes)
            else:
                rap+=max(60*L/1200 if L>0 else 0,axes)
        pos=to
    return tot,rap
for path in sys.argv[2:]:
    t=open(path).read() if not path.startswith('main:') else subprocess.run(['git','show',path],capture_output=True,text=True).stdout
    a,b=run(t,float(sys.argv[1]));print(path,'fed %.1f s rapid %.1f s total %.1f s'%(a,b,a+b))
