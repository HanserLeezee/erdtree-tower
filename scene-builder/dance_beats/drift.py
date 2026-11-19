import pickle,sys
sys.path.insert(0,'/tmp/dancetool')
from refine import best_phase,FR
name,win=sys.argv[1],float(sys.argv[2])
o,ol,el=pickle.load(open(f'/tmp/dancetool/{name}.env','rb'))
bpm,first=pickle.load(open(f'/tmp/dancetool/{name}.tempo','rb'))
p=FR*60/bpm;ph=first*FR;seg=int(win*FR);rows=[]
for st in range(0,len(o)-seg,seg):
    s2,ph2=best_phase(o,p,st,st+seg,steps=120)
    d=((ph2-ph)/p+0.5)%1-0.5;rows.append(f"{st/FR:5.0f}s {d*p/FR*1000:+5.0f}ms {s2:4.2f}")
print(f"{name} bpm={bpm} first={first:.3f}")
for i in range(0,len(rows),6):print("   ".join(rows[i:i+6]))
