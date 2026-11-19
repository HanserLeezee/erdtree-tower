import pickle,math,sys
SR=11025;HOP=128;FR=SR/HOP
def comb(o,period,phase,start=0,end=None):
    end=end or len(o);s=0;k=0
    t=phase
    while t<start:t+=period
    while t<end-2:
        j0=int(t);f=t-j0
        # small window max (±1 frame) to tolerate jitter
        s+=max(o[j0-1] if j0>0 else 0,o[j0]*(1-f)+o[j0+1]*f,o[j0+1]);k+=1;t+=period
    return s/max(k,1)
def best_phase(o,period,start=0,end=None,steps=48):
    return max(((comb(o,period,period*i/steps,start,end),period*i/steps) for i in range(steps)))
if __name__=="__main__":
  for name,lo,hi in [(a,float(b),float(c)) for a,b,c in (x.split(':') for x in sys.argv[1:])]:
     o,ol,el=pickle.load(open(f'/tmp/dancetool/{name}.env','rb'))
     res=[]
     bpm=lo
     while bpm<=hi:
         p=FR*60/bpm;sc,ph=best_phase(o,p,steps=32);res.append((sc,bpm,ph));bpm=round(bpm+0.02,3)
     res.sort(reverse=True);sc,bpm,ph=res[0]
     # finer phase
     p=FR*60/bpm;sc,ph=best_phase(o,p,steps=200)
     print(f"{name}: bpm={bpm:.2f} score={sc:.3f} first_beat={ph/FR:.3f}s  runner-ups={[(round(b,2),round(s,3)) for s,b,_ in res[1:4]]}")
     # stability: per 30s segment best phase vs global
     seg=int(30*FR);out=[]
     for st in range(0,len(o)-seg,seg):
         s2,ph2=best_phase(o,p,st,st+seg,steps=100)
         d=((ph2-ph)/p+0.5)%1-0.5
         out.append(f"{st/FR:4.0f}s:{d*p/FR*1000:+5.0f}ms({s2:.2f})")
     print("  segment phase drift:"," ".join(out))
     pickle.dump((bpm,ph/FR),open(f'/tmp/dancetool/{name}.tempo','wb'))
