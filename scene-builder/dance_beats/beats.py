import array,math,sys
SR=11025;HOP=128;FR=SR/HOP  # ~86 frames/s
def env(path):
    a=array.array('h');a.frombytes(open(path,'rb').read())
    e=[]
    for i in range(0,len(a)-HOP,HOP):
        s=0
        for v in a[i:i+HOP]:s+=v*v
        e.append(math.log(1e-6+s/HOP))
    # onset = half-wave rectified difference
    o=[0.0]+[max(0.0,e[i]-e[i-1]) for i in range(1,len(e))]
    m=sum(o)/len(o);o=[x-m if x>m else 0.0 for x in o]
    return o,e
def analyze(name):
    of,_=env(f'/tmp/dance/{name}.full.raw');ol,el=env(f'/tmp/dance/{name}.low.raw')
    o=[a+b for a,b in zip(of,ol)];n=len(o)
    # tempo: autocorrelation over 70..180 BPM
    best=[]
    for bpm10 in range(700,1801,5):
        bpm=bpm10/10;p=FR*60/bpm
        # comb energy across phases (coarse)
        sc=0;cnt=0;L=int(p)
        lag=p
        s=0
        for i in range(0,n-int(lag)-1,2):
            j=i+lag;j0=int(j);f=j-j0
            s+=o[i]*(o[j0]*(1-f)+o[j0+1]*f)
        best.append((s,bpm))
    best.sort(reverse=True)
    return o,ol,el,best[:8]
for name in sys.argv[1:]:
    o,ol,el,best=analyze(name)
    print(name,'frames',len(o),'top tempos',[(round(b,1),round(s)) for s,b in best])
    import pickle;pickle.dump((o,ol,el),open(f'/tmp/dancetool/{name}.env','wb'))
