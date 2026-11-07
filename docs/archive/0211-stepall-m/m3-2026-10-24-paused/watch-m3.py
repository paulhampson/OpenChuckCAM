import datetime,json,pathlib,re,time
root=pathlib.Path('/workspace/stepall-evidence')
while True:
 try:
  data=json.loads((root/'full-m3-status.json').read_text())
 except (FileNotFoundError,json.JSONDecodeError):
  data={'state':'waiting'}
 if data['state'] in ['complete','failed']:
  print('M3 runner: '+json.dumps(data),flush=True)
  break
 if not pathlib.Path('/proc/'+(root/'full-m3-runner.pid').read_text().strip()).exists():
  raise SystemExit('M3 runner stopped unexpectedly; inspect batch logs.')
 if data['state']=='running':
  batch=data['batch'];log=root/f'full-m3-{batch}.log'
  contents=log.read_text() if log.exists() else ''
  runs=re.findall(r'^run (\d+) \| job wall time.*\| success$',contents,re.M)
  start=datetime.datetime.fromisoformat(data['started_utc'].replace('Z','+00:00'))
  elapsed=(datetime.datetime.now(datetime.timezone.utc)-start).total_seconds()/60
  print(time.strftime('%H:%M:%S UTC',time.gmtime())+f' | M3 {batch}: {len(runs)}/5 timed runs complete; batch elapsed {elapsed:.1f} min including setup/warm-up',flush=True)
 else:print('M3 waiting to start',flush=True)
 time.sleep(55)
