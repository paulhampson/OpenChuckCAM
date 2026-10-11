# Throwaway: random mutation of STEP fixtures; classify how the reader process ends.
import random, subprocess, sys, collections
B="target/release/sp1-step"
random.seed(1)
res=collections.Counter(); bad=[]
for name in ["cylinder","filleted_box","rotary_blank_drilled"]:
    data=open(f"fixtures/{name}.step","rb").read()
    for i in range(150):
        b=bytearray(data)
        for _ in range(random.choice([1,3,10])):
            k=random.randrange(len(b))
            m=random.choice(["flip","del","dup","digit"])
            if m=="flip": b[k]^=1<<random.randrange(8)
            elif m=="del": del b[k:k+random.randint(1,40)]
            elif m=="dup": b[k:k]=b[k:k+random.randint(1,40)]
            else: b[k]=ord(random.choice("0123456789#.,;()"))
        open("/tmp/mut.step","wb").write(b)
        try:
            p=subprocess.run([B,"walk","/tmp/mut.step"],capture_output=True,timeout=30)
            rc=p.returncode
        except subprocess.TimeoutExpired:
            rc="timeout"
        key="ok(0)" if rc==0 else "clean-error(2)" if rc==2 else f"CRASH({rc})"
        res[key]+=1
        if key.startswith("CRASH") or rc=="timeout":
            bad.append((name,i,rc)); open(f"results/crash_{name}_{i}.step","wb").write(b)
print(dict(res)); print(bad[:10])
