import hashlib, json, math, os, pathlib, subprocess, time
root=pathlib.Path('/workspace/OpenChuckCAM')
pathlib.Path('/workspace/stepall-evidence/full-m3-runner.pid').write_text(str(os.getpid()))
out=pathlib.Path('/workspace/stepall-evidence')
binary=out/'m3-step-accuracy'
source='eee448609348e7ecc5bbfe71e8b3377d91170b32'
manifest={'source':subprocess.check_output(['git','rev-parse',source],cwd=root,text=True).strip(), 'binary_sha256':hashlib.sha256(binary.read_bytes()).hexdigest(), 'scope':'six successful operations in original sequence; simultaneous refusal excluded explicitly; not a seven-operation benchmark', 'runs':5, 'rayon_threads':4}
(out/'full-m3-manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
# M1/M4 are correctness investigations, not timing samples. Wait for them and
# the separate real-time simulator check before measuring benchmark time.
while not (out/'allow-full-m3').exists() or any(pathlib.Path(f'/proc/{pid}').exists() for pid in [62744,108771]):
 time.sleep(5)
env=os.environ.copy();env['OPENCHUCK_STEP_WORKER']=str(root/'target/release/openchuck-step-worker');env['RAYON_NUM_THREADS']='4'
for threads in ['all','1']:
 for form in ['b','c','a']:
  key=f'{form}-{threads}'; log=out/f'full-m3-{key}.log'; done=out/f'full-m3-{key}.done'
  if done.exists(): continue
  args=[str(binary),'--only','m3','--forms',form,'--ops','roughing,rest,pencil,helical,axial,indexed','--runs','5','--threads',threads,'--skip-probe','--fine','0.01','--angular','0.02']
  if form=='a':args+=['--stl','/workspace/owner-blisk.stl']
  start=time.time();(out/'full-m3-status.json').write_text(json.dumps({'batch':key,'state':'running','started_utc':time.strftime('%Y-%m-%dT%H:%M:%SZ',time.gmtime()),'argv':args},indent=2)+'\n')
  rejected=None
  with log.open('w') as f:
   with subprocess.Popen(args,cwd=root,env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True) as child:
    for line in child.stdout:
     f.write(line);f.flush()
     fields=[v.strip() for v in line.split('|')]
     if fields[0].startswith('run ') and len(fields)==11:
      values=list(map(float,fields[3:10]))
      if fields[10]!='success' or not all(math.isfinite(v) and v>=0 for v in values):
       rejected='unsuccessful or invalid operation timing'
      elif values[6]<=0 or (fields[1]!='roughing' and values[5]<=0):
       rejected='missing fitting or assembly timing in successful finishing work'
      if rejected:
       child.terminate()
    exit_code=child.wait()
  text=log.read_text()
  if rejected or exit_code or 'job wall median' not in text or 'unavailable' in text or 'REFUSED' in text:
   (out/'full-m3-status.json').write_text(json.dumps({'batch':key,'state':'failed','exit':exit_code,'reason':rejected},indent=2)+'\n');raise SystemExit(1)
  done.write_text(json.dumps({'elapsed_s':time.time()-start,'exit':0,'source':manifest['source'],'binary_sha256':manifest['binary_sha256']},indent=2)+'\n')
(out/'full-m3-status.json').write_text(json.dumps({'state':'complete'},indent=2)+'\n')
