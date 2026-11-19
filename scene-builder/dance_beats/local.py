import pickle,sys
sys.path.insert(0,'/tmp/dancetool')
from refine import best_phase,FR
name,lo,hi,seglen=sys.argv[1],float(sys.argv[2]),float(sys.argv[3]),float(sys.argv[4])
o,ol,el=pickle.load(open(f'/tmp/dancetool/{name}.env','rb'))
seg=int(seglen*FR)
for st in range(0,len(o)-seg+1,seg//2):
    res=[]
    b=lo
    while b<=hi:
        p=FR*60/b;sc,ph=best_phase(o,p,st,st+seg,steps=24);res.append((sc,b));b=round(b+0.1,2)
    res.sort(reverse=True)
    print(f"{st/FR:5.1f}s  best={res[0][1]:6.1f} ({res[0][0]:.2f})  2nd={res[1][1]:6.1f}  3rd={res[2][1]:6.1f}")
