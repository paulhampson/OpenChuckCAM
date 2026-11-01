import json, math, pathlib, statistics, sys
root=pathlib.Path('/workspace/stepall-evidence')
ops=['roughing','rest','pencil','helical','axial','indexed']
all_data={}
manifest=json.loads((root/'full-m3-manifest.json').read_text())
for form in 'abc':
 for threads in ['1','all']:
  name=f'{form}-{threads}'; done=root/f'full-m3-{name}.done'
  if not done.exists():
   if '--partial' in sys.argv: continue
   raise SystemExit(f'Incomplete benchmark: {name}')
  completed=json.loads(done.read_text())
  assert completed['exit']==0 and all(completed[k]==manifest[k] for k in ['source','binary_sha256']),name
  expected_threads='1' if threads=='1' else 'all (4)'
  lines=(root/f'full-m3-{name}.log').read_text().splitlines()
  assert 'cores: 4' in lines and 'form c deflection: 0.01 mm and 0.02 rad' in lines,name
  rows={op:{} for op in ops}; wall={}; medians={}; reported={}
  for line in lines:
   cols=[c.strip() for c in line.split('|')]
   if cols[0].startswith('run '):
    run=int(cols[0].split()[1])
    assert cols[2]==expected_threads,line
    if cols[1]=='job wall time':
     assert cols[4]=='success'; wall[run]=float(cols[3].split()[0]); continue
    assert cols[1] in ops and cols[10]=='success',line
    assert run not in rows[cols[1]],line
    rows[cols[1]][run]=list(map(float,cols[3:10]))
    assert all(math.isfinite(v) and v>=0 for v in rows[cols[1]][run]),line
    assert rows[cols[1]][run][6]>0 and (cols[1]=='roughing' or rows[cols[1]][run][5]>0),(name,line,'missing stage timing')
   elif cols[0] in ops:
    assert cols[1]==expected_threads,line
    reported[cols[0]]=list(map(float,cols[2:]))
   elif cols[0] in ['job generation median','job wall median']:
    assert cols[1]==expected_threads,line
    reported[cols[0]]=float(cols[2].split()[0])
  assert set(wall)==set(range(1,6)),name
  for op in ops:
   assert set(rows[op])==set(range(1,6)),(name,op)
   medians[op]=[statistics.median(v[i] for v in rows[op].values()) for i in range(7)]
   # Drop count and query count must agree across equivalent repetitions.
   assert len({v[1] for v in rows[op].values()})==1,(name,op,'drop count')
   assert len({v[3] for v in rows[op].values()})==1,(name,op,'query count')
   assert all(abs(a-b)<0.0000011 for a,b in zip(medians[op],reported[op])),(name,op,'median')
  for run in range(1,6):
   assert math.isfinite(wall[run]) and wall[run]+0.0001>=sum(rows[op][run][0]+rows[op][run][4] for op in ops),(name,run,'job wall encloses generation and simulation')
  generation=statistics.median(sum(rows[op][run][0] for op in ops) for run in range(1,6))
  assert abs(generation-reported['job generation median'])<0.00001,(name,'job generation')
  assert abs(statistics.median(wall.values())-reported['job wall median'])<0.0000011,(name,'job wall')
  all_data[name]={'operations':medians,'generation':reported['job generation median'],'wall':reported['job wall median']}
  print(name,'complete: five equivalent successful runs;', 'generation', reported['job generation median'],'wall',reported['job wall median'])
for form in 'abc':
 if form+'-1' in all_data and form+'-all' in all_data:
  one, many=all_data[form+'-1'],all_data[form+'-all']
  for op in ops:
   assert all(one['operations'][op][i]==many['operations'][op][i] for i in [1,3]),(form,op,'thread-count changes drop/query counts')
   print('speedup',form,op,one['operations'][op][0]/many['operations'][op][0])
if 'b-all' in all_data and 'c-all' in all_data:
 b,c=all_data['b-all'],all_data['c-all']
 print('c/b job generation',c['generation']/b['generation'])
 print('c/b job wall',c['wall']/b['wall'])
 for op in ops: print('c/b operation generation',op,c['operations'][op][0]/b['operations'][op][0])
(root/'full-m3-summary.json').write_text(json.dumps(all_data,indent=2)+'\n')
