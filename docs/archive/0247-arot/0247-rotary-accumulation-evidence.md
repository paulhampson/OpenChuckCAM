# AROT raw measurement evidence, 26 October 2026

Base `ba10400fff3bb7ad32526a7e540919c810e85434`, built uninstrumented with `cargo build -p openchuck-cli --locked` (0.23.0). Existing 0.20.0 binary was excluded. One generation worker; Linux x86_64 cloud environment; concurrent lint/audit/generation processes mean wall timings are observations, not benchmarks. No machining or LinuxCNC execution was performed for these measurements. The docs were subsequently rebased onto main `8f788bf` (0.24.0); the following numbers and hashes remain measurements of 0.23.0, not reruns on 0.24.0. The merged SHORTMOVE work does not change the rotary assembly or A convention.

[Raw JSON](0247-rotary-accumulation-raw.json) includes every invocation, exit status, log hashes/line counts and first/last lines (complete text for errors), complete TOML settings, output SHA-256, extrema and quarter checkpoints. Reference inputs are restored unchanged using `cargo run -p openchuck-cli --example sample_projects -- /tmp/arot-projects`; generated models are deliberately absent from committed projects. The CLI migrates temporary copies, not repository fixtures. Each invocation uses `--threads 1 regenerate PROJECT --post POST --tool-numbers keep --grouping operation --out OUT`.

LinuxCNC A words are absolute, unwrapped, same-direction, three decimals, with no counter reset. Sum of absolute differences from A0 is the total commanded A travel in degrees, including links/retracts; the first positioning word is measured from nominal A0, not a claim about an unknown actual machine start; G28 footer travel is not inferred. Table min/max are actual commanded positions, excluding an invented initial A0. Legacy JSON `A_min`/`A_max` include nominal A0; `commanded_A_min`/`commanded_A_max` give the actual extrema used in the table. JSON also reports `first_commanded_A` and `travel_between_commanded_positions`, excluding that initial nominal positioning contribution. For first-cut finishing, the first command is A−168.397 and travel between commanded positions is 20493.749 degrees; the table total including nominal A0 positioning is 20662.146. Other nonzero starts are first-cut roughing (A135; between-position travel 82265 degrees), blisk roughing (A125; 80850 degrees), and lobed simultaneous (A16.949; 3599.478 degrees). Other measured files start at A0. Quarter checkpoints are at 25/50/75/100 percent of A-bearing motion words, NOT time or axial distance. Z1 fields describe counter words; raw word differences across a reset must not be treated as physical travel. These single-operation files reset only after their motion, so their motion-word extrema match the unwrapped files. Reset commands have no physical travel.

All projects use their copied Makera Z1 Continuous A profile; shipped LinuxCNC and Makera posts. Reference settings are complete in JSON. Typical reference tool: 3 mm ball, oak, pitch 2 mm, depth step 1 mm, 12000 RPM, 0.02 mm/tooth, plunge 150 mm/min, angle step 5 degrees. First-cut instead uses Plastic: rough O-flute 3.175×25, 10000 RPM, 0.1 mm/tooth, pitch 2.2225 mm, depth step 2 mm; finish ball 3.175×22, 12000 RPM, 0.02083333333 mm/tooth, pitch 0.453571 mm. Full stock, offsets, workholding and machine provenance are in JSON, not omitted from the evidence.

Extra blisk jobs are controlled diagnostics rather than historical owner project reproductions: copy restored cone, copy `testdata/stl/blisk.stl` to `models/blisk.stl`, import it, edit Setup 1 with `--part models/blisk.stl --stock cylinder=42,52,-1`, retain the cone ball/oak tool and parameters, and replace the strategy as named. Pitch variants change only cone tool oak stepover to 1.0 or 0.5. All strategy defaults remain unless explicitly present in the saved TOML. These are measurement settings, not tool/material recommendations.

## Unwrapped motion measurements (nominal A0 reference)

| Job / operation | Min A ° | Max A ° | Total A travel from nominal A0 ° | Generation wall s | Programmed operation s (rounded post comment) |
| --- | ---: | ---: | ---: | ---: | ---: |
| cone / 01-setup-1-finish.ngc | 0.0 | 5400.0 | 5400.0 | 2.164 | 108 |
| cone_axial / 01-setup-1-finish.ngc | 0.0 | 348.75 | 348.75 | 1.964 | 454 |
| cone_axial_lead / 01-setup-1-finish.ngc | 0.0 | 348.75 | 348.75 | 2.088 | 471 |
| cone_held_tabs / 01-setup-1-finish.ngc | 0.0 | 7920.0 | 7920.0 | 27.775 | 154 |
| cone_indexed / 01-setup-1-finish.ngc | 0.0 | 270.0 | 270.0 | 1.726 | 61 |
| first_cut / 01-setup-1-rough.ngc | 135.0 | 82080.0 | 82400.0 | 89.729 | 1780 |
| first_cut / 02-setup-1-finish.ngc | -168.397 | 20325.352 | 20662.146 | 89.729 | 378 |
| groove / 01-setup-1-finish.ngc | 0.0 | 5400.0 | 5400.0 | 2.05 | 143 |
| lobed / 01-setup-1-finish.ngc | 16.949 | 3616.427 | 3616.427 | 12.662 | 101 |
| sphere / 01-setup-1-finish.ngc | 0.0 | 3600.0 | 3600.0 | 1.43 | 83 |
| blisk_helical / 01-setup-1-finish.ngc | 0.0 | 9000.0 | 9000.0 | 6.83 | 491 |
| blisk_rough / 01-setup-1-finish.ngc | 125.0 | 75495.0 | 80975.0 | 8.624 | 6652 |
| cone_pitch05 / 01-setup-1-finish.ngc | 0.0 | 21600.0 | 21600.0 | 4.006 | 398 |
| cone_pitch1 / 01-setup-1-finish.ngc | 0.0 | 10800.0 | 10800.0 | 2.751 | 205 |

## Refusals and scope

- drilling: STEP import failed: this build cannot read STEP files: the STEP reader program 'openchuck-step-worker' was not found next to the application (or set OPENCHUCK_STEP_WORKER) (wall 0.005 s). No G-code, no A extrema inferred.
- swarf: STEP import failed: this build cannot read STEP files: the STEP reader program 'openchuck-step-worker' was not found next to the application (or set OPENCHUCK_STEP_WORKER) (wall 0.007 s). No G-code, no A extrema inferred.
- blisk_simultaneous: simultaneous finishing cannot keep the move to X 11.334, A 1835.005 within the tolerance: it would cut 0.1571 mm into the surface; reduce the largest tilt or the largest angle step (wall 59.349 s). No G-code, no A extrema inferred.

Drilling and swarf require a STEP worker unavailable beside this build. They are attempted and refusals recorded, not counted as successful measurements. Blisk simultaneous finishing is attempted with defaults and refused by gouge protection; no program is claimed. The lobed reference supplies successful simultaneous coverage when generated. Original reference strategies are measured as saved; they are not all forced into helical roughing where stock, tool or operation purpose would differ. The extra blisk jobs supply both continuous helical roughing and finishing.

## Growth and interpretation

For the 30 mm cone, pitch 2/1/0.5 produces 5400/10800/21600 degrees (15/30/60 turns). This directly measures inverse-pitch growth with unchanged part and operation length; halving pitch doubles rotary travel. A 50 mm blisk at pitch 2 gives 9000 degrees for a finishing helix. Roughing layers add turns; totals can exceed the extrema span because of reversals in links. More block subdivision does not itself imply more rotary travel. First-cut demonstrates accumulation with realistic multiple layers, tabs and held regions. Programmed seconds and generation wall seconds are different quantities.

No run demonstrates a Z1 A-position failure, and no run reaches the LinuxCNC verification envelope ±1e9 degrees. Unknown Z1 word capacity/precision must not be replaced with an invented safe bound. The older 0241 historical 593275-degree blisk evidence is context, not one of these fresh measurements.

## Reproduction scripts

The following scripts only create temporary projects, invoke unchanged product code and parse emitted files; they do not instrument generation or change machine output.

```python
import pathlib,shutil,subprocess
root=pathlib.Path('/tmp/arot-extra'); root.mkdir(exist_ok=True)
cli='/workspace/OpenChuckCAM/target/debug/openchuck-cli'
for name,strategy,pitch in [('blisk_helical','helical_finishing',2),('blisk_rough','rotary_roughing',2),('blisk_simultaneous','simultaneous_finishing',2),('cone_pitch1','helical_finishing',1),('cone_pitch05','helical_finishing',.5)]:
 p=root/name; shutil.copytree('/tmp/arot-projects/cone',p)
 if name.startswith('blisk'):
  shutil.copy('/workspace/OpenChuckCAM/testdata/stl/blisk.stl',p/'models/blisk.stl')
  for args in [['import',str(p),str(p/'models/blisk.stl')],['setup',str(p),'Setup 1','--part','models/blisk.stl','--stock','cylinder=42,52,-1']]:
   r=subprocess.run([cli]+args,capture_output=True,text=True); assert r.returncode==0,r.stderr
 for f in (p/'operations').glob('*.toml'):
  t=f.read_text().replace('helical_finishing',strategy); f.write_text(t)
 for f in (p/'tools').glob('*.toml'): f.write_text(f.read_text().replace('stepover = 2.0',f'stepover = {pitch:.1f}'))
```

```python
import pathlib,subprocess,time,json,re,hashlib,shutil
root=pathlib.Path('/tmp/arot-projects'); output=pathlib.Path('/tmp/arot-results'); output.mkdir(exist_ok=True)
rows=[]
for p in sorted(root.iterdir()):
 if not (p/'project.toml').exists(): continue
 for post in ['linuxcnc','makera-z1']:
  out=output/p.name/post; out.mkdir(parents=True,exist_ok=True)
  cmd=['/workspace/OpenChuckCAM/target/debug/openchuck-cli','--threads','1','regenerate',str(p),'--post',post,'--tool-numbers','keep','--grouping','operation','--out',str(out)]
  start=time.monotonic(); r=subprocess.run(cmd,capture_output=True,text=True); elapsed=time.monotonic()-start
  (out/'run.log').write_text(r.stdout+r.stderr)
  row={'job':p.name,'post':post,'seconds':round(elapsed,3),'exit':r.returncode,'command':cmd,'files':[]}
  for f in sorted(out.glob('*')):
   if f.suffix not in ['.ngc','.nc']: continue
   a=0.; vals=[a]; travel=0.; checkpoints=[]
   # For LinuxCNC all A words are absolute and no resets; Z1 track words only (reset is NOT physical motion).
   for line in f.read_text().splitlines():
    if line.startswith('('): continue
    if line.startswith('G92.4'): continue
    m=re.search(r'\bA(-?\d+(?:\.\d+)?)',line)
    if m:
     b=float(m[1]); travel+=abs(b-a); a=b; vals.append(a)
   row['files'].append({'file':f.name,'sha256':hashlib.sha256(f.read_bytes()).hexdigest(),'A_min':min(vals),'A_max':max(vals),'A_last':a,'sum_abs_delta_A':round(travel,6),'A_word_count':len(vals)-1,'A_at_quartiles':[vals[int((len(vals)-1)*q)] for q in [.25,.5,.75,1.]]})
  rows.append(row); (output/'measurements.json').write_text(json.dumps(rows,indent=2)); print(p.name,post,r.returncode,elapsed,flush=True)
```

Run the measurement script again with root `/tmp/arot-extra` and output `/tmp/arot-extra-results` for extra cases. Raw JSON renames Z1 extrema to counter-word extrema and marks its raw difference sum as nonphysical.
