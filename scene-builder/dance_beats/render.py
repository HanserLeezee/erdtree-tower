import pickle,math,subprocess,array,os
OUT='/home/tamikip/ring/预览图片/2026-11-19/癫火舞会'
os.makedirs(OUT,exist_ok=True)
SR=44100
def click(freq,ms,amp):
    n=int(SR*ms/1000);return [amp*math.sin(2*math.pi*freq*i/SR)*math.exp(-i/(n/5)) for i in range(n)]
soft=click(1500,40,9000);hard=click(2400,70,16000)
for name in ['song1','song2']:
    bpm,s0,phrase,scored=pickle.load(open(f'/tmp/dancetool/{name}.excerpt','rb'))
    beat=60/bpm;bar=4*beat;start=s0-2*bar;length=2*bar+scored*phrase*beat+bar
    # game asset: stereo 44.1k excerpt with fades
    wav=f'/tmp/dancetool/{name}-excerpt.wav'
    subprocess.run(['ffmpeg','-v','error','-y','-ss',f'{start:.4f}','-t',f'{length:.4f}','-i',f'/tmp/dance/{name}.mp3','-ac','2','-ar',str(SR),
        '-af',f'afade=t=in:st=0:d=0.4,afade=t=out:st={length-bar:.3f}:d={bar:.3f}','-c:a','pcm_s16le',wav],check=True)
    # click track (mono) aligned: grid beat 0 at excerpt time 0 (start is on a downbeat)
    n=int(length*SR);c=array.array('h',[0])*n
    k=0
    while k*beat<length:
        t=k*beat;i=int(t*SR)
        scored_beat=k-8  # 2 intro bars = 8 beats
        target= scored_beat>=0 and scored_beat<scored*phrase and (scored_beat%phrase)==phrase-1
        for j,v in enumerate(hard if target else soft):
            if i+j<n:c[i+j]=max(-32767,min(32767,c[i+j]+int(v)))
        k+=1
    raw=f'/tmp/dancetool/{name}-click.raw';open(raw,'wb').write(c.tobytes())
    subprocess.run(['ffmpeg','-v','error','-y','-i',wav,'-f','s16le','-ar',str(SR),'-ac','1','-i',raw,
        '-filter_complex','[0:a]volume=0.8[a];[1:a]pan=stereo|c0=c0|c1=c0[b];[a][b]amix=inputs=2:normalize=0','-b:a','160k',f'{OUT}/{name}-节拍校对.mp3'],check=True)
    print(name,f'bpm={bpm} excerpt {start:.2f}s +{length:.1f}s ->',f'{OUT}/{name}-节拍校对.mp3')
