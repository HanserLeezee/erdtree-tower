# 癫火舞会：舞曲节拍分析

纯 Python（不需要 numpy），只需要 ffmpeg。分析得到速度、第一拍、重拍和截取段，再叠节拍器渲染校对音频给人耳确认。

1. 解码：`ffmpeg -i song.mp3 -ac 1 -ar 11025 -f s16le /tmp/dance/<name>.full.raw`，再加 `-af lowpass=f=150` 生成 `<name>.low.raw`（底鼓）
2. `beats.py <name>…`：起音包络与粗略速度候选（70–180 BPM）
3. `refine.py <name>:<低>:<高>`：0.02 BPM 精度的速度与第一拍，并按 30 秒分段看相位漂移
4. `local.py <name> <低> <高> <窗口秒>`：逐段局部速度（判断是否变速、是否被倍速 / 半速误判）
5. `drift.py <name> <窗口秒>`：逐段相位偏移（±半拍多是反拍镲片，稳定的小偏移多是切分段落）
6. `excerpt.py`：找重拍与最响的一段，`render.py`：截取舞曲（重拍起、开头 2 小节预备、结尾 1 小节淡出）并叠节拍器出校对 mp3

脚本里的路径是当时的临时目录（/tmp/dance、/tmp/dancetool），换歌时按需修改。结果写进 `scene-assets/dance-songs.tsv`；音频放 `scene-assets/dance/`（不进仓库）。
