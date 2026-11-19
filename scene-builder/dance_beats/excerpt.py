import pickle,sys
sys.path.insert(0,'/tmp/dancetool')
from refine import FR
def at(o,t):
    i=int(t*FR);return max(o[max(i-1,0):i+2]) if i+2<len(o) else 0
for name,phrase,scored in [('song1',4,32),('song2',8,20)]:
    o,ol,el=pickle.load(open(f'/tmp/dancetool/{name}.env','rb'))
    bpm,first=pickle.load(open(f'/tmp/dancetool/{name}.tempo','rb'))
    beat=60/bpm;dur=len(o)/FR;nbeats=int((dur-first)/beat)
    # downbeat: phase mod 4 with strongest kick
    sums=[sum(at(ol,first+k*beat) for k in range(m,nbeats,4)) for m in range(4)]
    m=max(range(4),key=lambda i:sums[i])
    bar=4*beat;length=scored*phrase*beat
    # loudest window by mean full-band energy, start on a downbeat, leave 2 bars intro
    best=None
    k=m
    while first+k*beat+length+bar<dur-1:
        s0=first+k*beat
        if s0-2*bar>=0:
            a,b=int(s0*FR),int((s0+length)*FR);loud=sum(el[a:b])/(b-a)
            if not best or loud>best[0]:best=(loud,s0)
        k+=4
    loud,s0=best
    print(f"{name}: bpm={bpm} beat={beat*1000:.1f}ms downbeat_phase={m} kick_sums={[round(x) for x in sums]}")
    print(f"   excerpt scored start={s0:.3f}s length={length:.1f}s ({scored} phrases x {phrase} beats) -> audio {s0-2*bar:.3f}s .. {s0+length+bar:.3f}s")
    pickle.dump((bpm,s0,phrase,scored),open(f'/tmp/dancetool/{name}.excerpt','wb'))
