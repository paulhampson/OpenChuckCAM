import sys, math, re, collections
path=sys.argv[1]; lim=float(sys.argv[2])
pos={'X':0,'Y':0,'Z':0,'A':0}; F=0; g='0'
rows=[]
for n,line in enumerate(open(path),1):
    s=line.strip()
    if s.startswith('(') or not s: continue
    w=dict(re.findall(r'([A-Z])(-?[\d.]+)',s))
    if 'G' in w and w['G'] in ('0','1'): g=w['G']
    if 'F' in w: F=float(w['F'])
    if not any(k in w for k in 'XYZA'): continue
    new=dict(pos); [new.__setitem__(k,float(w[k])) for k in 'XYZA' if k in w]
    L=math.dist([pos[k] for k in 'XYZ'],[new[k] for k in 'XYZ']); dA=abs(new['A']-pos['A'])
    r=math.hypot(new['Y'],new['Z']); P=2*math.pi*max(r,1)+30
    amt=max(L,dA*P/360) if L>1e-9 else dA*min(P,360)/360
    t=amt*60/F*1000 if (g=='1' and F>0) else None
    rows.append((n,g,L,dA,F,t,s))
    pos=new
short=[r for r in rows if r[5] is not None and r[5]<lim]
print(len(short),'short of',sum(1 for r in rows if r[5] is not None))
Ls=sorted(r[2] for r in short); print('XYZ length of short blocks: min %.4f p50 %.4f max %.4f'%(Ls[0],Ls[len(Ls)//2],Ls[-1]))
Fs=collections.Counter(round(r[4]) for r in short).most_common(5); print('feeds',Fs)
dAs=sorted(r[3] for r in short); print('dA p50 %.3f max %.3f'%(dAs[len(dAs)//2],dAs[-1]))
# angle between consecutive directions at short blocks
idx={r[0]:i for i,r in enumerate(rows)}
for r in short[int(sys.argv[3]) if len(sys.argv)>3 else 0:][:3]:
    i=idx[r[0]]
    for q in rows[max(0,i-3):i+4]: print('  %6d %s L=%.4f dA=%.3f F=%.0f t=%s  %s'%(q[0],q[1],q[2],q[3],q[4],'%.2f'%q[5] if q[5] else '-',q[6]))
    print()
