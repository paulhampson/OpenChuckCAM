from decimal import Decimal as D
import pathlib,re,json,hashlib,sys

def decode(p):
    pos=[D(0)]*4;offset=D(0);kind=0;feed=None;motions=[];controls=[];stats={};op='header';op_index=0
    for line in p.read_text().splitlines():
        if line.startswith('(operation '):
            op_index+=1;op=str(op_index)+':'+line.split(', tool')[0][11:].rstrip(')')
        if line.startswith('('):continue
        words=[(a,D(b)) for a,b in re.findall(r'([A-Z])\s*([-+]?\d+(?:\.\d+)?)',line.split(';')[0])]
        w=dict(words);gs=[v for k,v in words if k=='G']
        if D('92.4') in gs:
            assert set(w)=={'G','A','S'} and gs==[D('92.4')] and w.get('S')==0 and w.get('A')==0
            if abs(pos[3])>360:
                remainder=pos[3]%360;offset+=pos[3]-remainder;pos[3]=remainder
            stats.setdefault(op,{'counter_min':None,'counter_max':None,'resets':0})['resets']+=1
            continue
        if 'M' in w:controls.append(tuple((a,v) for a,v in words if a in 'MST'))
        if 'F' in w:feed=w['F']
        for g in gs:
            if g in [0,1]:kind=int(g)
        axes=any(a in w for a in 'XYZA')
        if not axes:continue
        if D(53) in gs:
            # Compare the explicit machine-coordinate footer without inferring
            # work offsets, G28 homing travel, acceleration or hardware time.
            motions.append(('machine',tuple((a,w[a]) for a in 'XYZA' if a in w),kind));continue
        for i,a in enumerate('XYZA'):
            if a in w:pos[i]=w[a]
        stat=stats.setdefault(op,{'counter_min':None,'counter_max':None,'resets':0})
        stat['counter_min']=pos[3] if stat['counter_min'] is None else min(stat['counter_min'],pos[3])
        stat['counter_max']=pos[3] if stat['counter_max'] is None else max(stat['counter_max'],pos[3])
        state=('work',tuple(pos[:3])+ (pos[3]+offset,),kind,feed if kind==1 else None)
        if not motions or state!=motions[-1]:motions.append(state)
    return motions,stats,controls

old,new=map(pathlib.Path,sys.argv[1:]);a,sa,ca=decode(old);b,sb,cb=decode(new)
assert ca==cb, "spindle/tool control blocks changed"
if a!=b:
    for i,(x,y) in enumerate(zip(a,b)):
        if x!=y:print('first mismatch',i,x,y);break
    raise SystemExit(f'motion mismatch: {len(a)} versus {len(b)}')
report={'baseline':str(old),'new':str(new),'baseline_sha256':hashlib.sha256(old.read_bytes()).hexdigest(),'new_sha256':hashlib.sha256(new.read_bytes()).hexdigest(),'identical_programmed_coordinate_records':len(a),'identical_spindle_tool_control_blocks':len(ca),'baseline_operations':sa,'new_operations':sb,'limitations':'Explicit programmed XYZ/A/feed/motion records only; no G28 homing inference, acceleration or new hardware validation.'}
print(json.dumps(report,indent=2,default=str))
