//! 癫火舞会：节奏小游戏的谱面、判定与计分。与画面、音频无关，时间都以「舞曲开始播放后的秒数」计。
//!
//! 每一句（phrase）给出一串方向键：在这一句里按对整串，再在这一句最后一拍（判定点）按下确认键。
//! 按的时机决定评价：完美 / 很好 / 不错 / 勉强 / 失误。方向键按错只是这一下不算，已经按对的保留。
//! 舞曲截取段从重拍开始，开头留 intro_beats 拍做预备，之后才是计分的句子。

/// 一首舞曲：速度、每句几拍、计分的句数、开头预备拍数（scene-assets/dance-songs.tsv）
#[derive(Clone,Debug,PartialEq)]
pub struct Song {pub id:String,pub file:String,pub name:String,pub bpm:f64,pub phrase_beats:u32,pub phrases:u32,pub intro_beats:u32}
impl Song {
    pub fn fit_audio(&mut self,seconds:f64) {
        if !seconds.is_finite()||seconds<=0.0||self.phrase_beats==0{return;}
        let phrases=((seconds/self.beat()-f64::from(self.intro_beats))/f64::from(self.phrase_beats)).floor();
        if phrases>=1.0 {self.phrases=phrases.min(10000.0) as u32;}
    }
    pub fn beat(&self)->f64 {60.0/self.bpm}
    /// 第 k 句开始的时间
    pub fn phrase_start(&self,k:u32)->f64 {f64::from(self.intro_beats+k*self.phrase_beats)*self.beat()}
    /// 第 k 句的判定点：这一句的最后一拍
    pub fn target(&self,k:u32)->f64 {self.phrase_start(k)+f64::from(self.phrase_beats-1)*self.beat()}
    /// 计分结束、可以结算的时间（最后一句之后再留一小节）
    pub fn end(&self)->f64 {self.phrase_start(self.phrases)+4.0*self.beat()}
    /// 按难度调整句长：普通难度每句拉长到至少 8 拍（句数相应减半），新手有足够时间看清、按完箭头
    pub fn for_difficulty(&self,difficulty:Difficulty)->Song {
        let mut song=self.clone();
        if difficulty==Difficulty::Normal&&song.phrase_beats<8 {
            let factor=8_u32.div_ceil(song.phrase_beats);
            song.phrase_beats*=factor;song.phrases=(song.phrases/factor).max(1);
        }
        song
    }
}
pub fn songs()->Vec<Song> {
    include_str!("../scene-assets/dance-songs.tsv").lines().filter(|l|!l.trim().is_empty()&&!l.starts_with('#')).map(|line|{
        let p:Vec<_>=line.split('|').collect();
        Song{id:p[0].into(),file:p[1].into(),name:p[2].into(),bpm:p[3].parse().expect("dance bpm"),phrase_beats:p[4].parse().expect("phrase beats"),
            phrases:p[5].parse().expect("phrases"),intro_beats:p[6].parse().expect("intro beats")}
    }).collect()
}

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Arrow {Up,Down,Left,Right}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Difficulty {Normal,Hard}
#[derive(Clone,Copy,Debug,PartialEq,Eq,PartialOrd,Ord,Hash)]
pub enum Judge {Miss,Bad,Cool,Great,Perfect}
impl Judge {
    pub fn name(self)->&'static str {match self {Judge::Perfect=>"完美",Judge::Great=>"很好",Judge::Cool=>"不错",Judge::Bad=>"勉强",Judge::Miss=>"失误"}}
    fn points(self)->f64 {match self {Judge::Perfect=>1000.0,Judge::Great=>700.0,Judge::Cool=>400.0,Judge::Bad=>100.0,Judge::Miss=>0.0}}
    /// 不错以上才接连击
    pub fn keeps_combo(self)->bool {self>=Judge::Cool}
}
/// 判定窗口（秒，按下时间与判定点之差的绝对值）
pub const WINDOWS:[(Judge,f64);4]=[(Judge::Perfect,0.080),(Judge::Great,0.150),(Judge::Cool,0.220),(Judge::Bad,0.300)];
pub fn judge(delta:f64)->Judge {WINDOWS.iter().find(|(_,w)|delta.abs()<=*w).map_or(Judge::Miss,|(j,_)|*j)}
const LATE:f64=0.300;

fn next(state:&mut u64)->u64 {*state^=*state<<13;*state^=*state>>7;*state^=*state<<17;*state}
/// 第 k 句的箭头数：随进度变长，困难更长；困难下每句 8 拍的歌多给一个
pub fn arrows_in(song:&Song,difficulty:Difficulty,k:u32)->usize {
    let (low,high)=match difficulty {Difficulty::Normal=>(2,4),Difficulty::Hard=>(4,7)};
    let extra=usize::from(difficulty==Difficulty::Hard&&song.phrase_beats>=8);
    low+extra+((high-low) as u32*k/song.phrases.max(1)) as usize
}
/// 谱面：每句的箭头，由种子决定（同一种子、同一首歌、同一难度永远一样）；不会连续三个相同
pub fn chart(song:&Song,difficulty:Difficulty,seed:u64)->Vec<Vec<Arrow>> {
    let mut state=seed^0xda9ce_f17e^(song.bpm.to_bits());if state==0 {state=1;}
    (0..song.phrases).map(|k|{
        let mut line:Vec<Arrow>=Vec::new();
        while line.len()<arrows_in(song,difficulty,k) {
            let arrow=[Arrow::Up,Arrow::Down,Arrow::Left,Arrow::Right][(next(&mut state)%4) as usize];
            if line.len()>=2&&line[line.len()-1]==arrow&&line[line.len()-2]==arrow {continue;}
            line.push(arrow);
        }
        line
    }).collect()
}

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Grade {S,A,B,C}
impl Grade {pub fn name(self)->&'static str {match self {Grade::S=>"S",Grade::A=>"A",Grade::B=>"B",Grade::C=>"C"}}}

impl crate::Run {
    fn dance_node(&self)->crate::RuleResult<usize> {
        let node=self.active_node(crate::NodeKind::Event)?;
        if self.event_draw(node)!=crate::events::Draw::Dance||self.regional_event.settled{return Err("当前不是可用的舞会事件".into());}Ok(node)
    }
    pub fn begin_dance(&mut self,song:usize)->crate::RuleResult<()> {
        self.dance_node()?;
        if song>=songs().len()||self.regional_event.opened!=0{return Err("舞会已经参加或曲目无效".into());}
        self.regional_event.opened=1;Ok(())
    }
    pub fn finish_dance(&mut self,grade:Option<Grade>)->crate::RuleResult<u32> {
        self.dance_node()?;
        if self.regional_event.opened!=1{return Err("舞会未开始或已经结算".into());}
        let reward=match grade{Some(Grade::S)=>5000,Some(Grade::A)=>3500,Some(Grade::B)=>2000,Some(Grade::C)=>1000,None=>0};
        let gold=self.gold.checked_add(reward).ok_or("卢恩数量超限")?;
        self.gold=gold;self.regional_event.opened=3;Ok(reward)
    }
}

#[cfg(test)]
mod event_tests {
    use super::*;
    fn room()->crate::Run {
        let mut run=crate::Run::new(7,crate::Class::Samurai);
        let node=run.nodes.iter().position(|n|n.kind==crate::NodeKind::Event).unwrap();
        run.debug_jump(node).unwrap();run.event_test=Some((run.act,node,crate::events::Draw::Dance));run.enter(node).unwrap();run
    }
    #[test]
    fn dance_is_consumed_before_playback_and_reward_is_once_only() {
        let mut run=room();let gold=run.gold;
        run.begin_dance(0).unwrap();assert!(run.begin_dance(1).is_err());
        let mut run=crate::Run::decode(&run.encode().unwrap()).unwrap();
        assert_eq!(run.finish_dance(Some(Grade::S)),Ok(5000));assert_eq!(run.gold,gold+5000);
        assert!(run.finish_dance(Some(Grade::S)).is_err());run.leave_regional_event().unwrap();
    }
    #[test]
    fn interrupted_and_skipped_dances_pay_nothing() {
        let mut run=room();let gold=run.gold;
        assert!(run.begin_dance(2).is_err());run.begin_dance(1).unwrap();
        assert_eq!(run.finish_dance(None),Ok(0));assert_eq!(run.gold,gold);
    }
    #[test]
    fn every_grade_pays_the_configured_reward_once() {
        for (grade,reward) in [(Grade::S,5000),(Grade::A,3500),(Grade::B,2000),(Grade::C,1000)] {
            let mut run=room();let gold=run.gold;run.begin_dance(0).unwrap();
            assert_eq!(run.finish_dance(Some(grade)),Ok(reward));
            assert_eq!(run.gold,gold+reward);assert!(run.finish_dance(Some(grade)).is_err());
        }
    }
    #[test]
    fn older_event_pools_never_draw_dance() {
        for seed in 0..100 {let mut run=crate::Run::new(seed,crate::Class::Samurai);run.event_rules=5;
            for (id,node) in run.nodes.iter().enumerate(){if node.kind==crate::NodeKind::Event{assert_ne!(run.event_draw(id),crate::events::Draw::Dance);}}
        }
    }
}

/// 一次跳舞的进度
#[derive(Clone,Debug)]
pub struct Play {
    pub song:Song,pub chart:Vec<Vec<Arrow>>,
    /// 当前句（全部判定完后等于句数）与这一句已按对的箭头数
    pub phrase:u32,pub entered:usize,
    pub combo:u32,pub max_combo:u32,pub score:f64,
    /// 各评价的次数，下标同 Judge 的顺序（失误、勉强、不错、很好、完美）
    pub counts:[u32;5],
    /// 最近一次判定与时间（画面弹字用）；最近一次按错箭头的时间（画面闪红）
    pub last:Option<(Judge,f64)>,pub wrong:Option<f64>,
}
impl Play {
    pub fn new(song:Song,difficulty:Difficulty,seed:u64)->Self {
        let song=song.for_difficulty(difficulty);
        let chart=chart(&song,difficulty,seed);
        Self{song,chart,phrase:0,entered:0,combo:0,max_combo:0,score:0.0,counts:[0;5],last:None,wrong:None}
    }
    pub fn current(&self)->Option<&[Arrow]> {self.chart.get(self.phrase as usize).map(|v|v.as_slice())}
    /// 这一句是否已经可以输入：上一句判定完、这一句的箭头一出现就能按（不用等这一句的第一拍）
    pub fn open(&self,_t:f64)->bool {self.phrase<self.song.phrases}
    /// 这一句的箭头是否已经按完（该等判定点按确认键了）
    pub fn ready(&self)->bool {self.current().is_some_and(|l|self.entered>=l.len())}
    pub fn arrow(&mut self,t:f64,arrow:Arrow) {
        if !self.open(t) {return;}
        let Some(line)=self.current() else {return};
        if self.entered>=line.len() {return;}
        // 按错只闪一下红、这一个不算，已经按对的保留（不用从头再来）
        if line[self.entered]==arrow {self.entered+=1;} else {self.wrong=Some(t);}
    }
    /// 按下确认键：离判定点太远（早于勉强窗口）不算；在窗口内按，箭头没按完算失误
    pub fn hit(&mut self,t:f64) {
        if self.phrase>=self.song.phrases {return;}
        let target=self.song.target(self.phrase);
        if t<target-LATE {return;}
        let complete=self.current().is_some_and(|l|self.entered>=l.len());
        self.resolve(if complete {judge(t-target)} else {Judge::Miss},t);
    }
    /// 每帧调用：过了判定点还没按的句子记失误
    pub fn tick(&mut self,t:f64) {
        while self.phrase<self.song.phrases&&t>self.song.target(self.phrase)+LATE {self.resolve(Judge::Miss,t);}
    }
    fn resolve(&mut self,judge:Judge,t:f64) {
        let length=self.current().map_or(4,|l|l.len()) as f64;
        self.counts[judge as usize]+=1;
        if judge.keeps_combo() {self.combo+=1;self.max_combo=self.max_combo.max(self.combo);} else {self.combo=0;}
        self.score+=judge.points()*length/4.0*multiplier(self.combo);
        self.last=Some((judge,t));self.phrase+=1;self.entered=0;
    }
    pub fn finished(&self,t:f64)->bool {self.phrase>=self.song.phrases&&t>=self.song.end()}
    /// 满分：每句都完美、全程连击
    pub fn max_score(&self)->f64 {self.chart.iter().enumerate().map(|(k,l)|Judge::Perfect.points()*l.len() as f64/4.0*multiplier(k as u32+1)).sum()}
    pub fn grade(&self)->Grade {
        let ratio=self.score/self.max_score().max(1.0);
        if ratio>=0.85 {Grade::S} else if ratio>=0.7 {Grade::A} else if ratio>=0.45 {Grade::B} else {Grade::C}
    }
}
/// 连击加成：每连一句 +2%，最多 +50%
fn multiplier(combo:u32)->f64 {1.0+f64::from(combo.min(25))*0.02}

/// 舞步表（scene-assets/dance-moves.tsv）：(档位, 动画编号)
pub fn moves()->Vec<(String,i32)> {
    include_str!("../scene-assets/dance-moves.tsv").lines().filter(|l|!l.trim().is_empty()&&!l.starts_with('#')).map(|l|{
        let p:Vec<_>=l.split('|').collect();(p[0].to_owned(),p[1].parse().expect("dance move anim"))
    }).collect()
}
/// 一句判定后角色要做的舞步：完美 / 很好挑华丽的（连击越高越偏向表里靠前的几个），不错挑轻快的，勉强 / 失误做失望；
/// 同一句的结果固定（种子与句号决定），不和上一句重复
pub fn move_for(judge:Judge,combo:u32,phrase:u32,seed:u64,previous:Option<i32>)->Option<i32> {
    let tier=match judge {Judge::Perfect|Judge::Great=>"flashy",Judge::Cool=>"light",Judge::Bad|Judge::Miss=>"miss"};
    let pool:Vec<i32>=moves().into_iter().filter(|(t,_)|t==tier).map(|(_,a)|a).collect();
    if pool.is_empty() {return None;}
    // 连击高时只在前面一段里挑（最花的那几个）
    let span=if tier=="flashy" {pool.len().saturating_sub((combo as usize/4).min(pool.len().saturating_sub(4))).max(1)}else{pool.len()};
    let mut state=seed^u64::from(phrase).wrapping_mul(0x9e37_79b9_7f4a_7c15)^0x5eed;if state==0{state=1;}
    for _ in 0..4 {
        let pick=pool[(next(&mut state)%span.min(pool.len()) as u64) as usize];
        if Some(pick)!=previous||pool.len()==1 {return Some(pick);}
    }
    pool.iter().copied().find(|a|Some(*a)!=previous)
}
/// 曲终收尾的动作
pub fn finale(grade:Grade)->Option<i32> {
    let tier=match grade {Grade::S=>"finale_s",Grade::A=>"finale_a",Grade::B=>"finale_b",Grade::C=>"finale_c"};
    moves().into_iter().find(|(t,_)|t==tier).map(|(_,a)|a)
}

#[cfg(test)]
mod tests {
    #[test]
    fn full_tracks_extend_the_chart_without_targets_after_audio_end() {
        for (mut song,duration) in super::songs().into_iter().zip([238.435918,260.839388]) {
            let old=song.phrases;song.fit_audio(duration);
            assert!(song.phrases>old);
            assert!(song.target(song.phrases-1)<duration);
            assert!(duration-song.phrase_start(song.phrases)<f64::from(song.phrase_beats)*song.beat());
        }
    }
    use super::*;
    fn song(phrase_beats:u32)->Song {Song{id:"t".into(),file:"t.wav".into(),name:"测试".into(),bpm:120.0,phrase_beats,phrases:16,intro_beats:8}}
    fn perfect(play:&mut Play) {
        for k in 0..play.song.phrases {
            let start=play.song.phrase_start(k);
            for (i,a) in play.chart[k as usize].clone().into_iter().enumerate() {play.arrow(start+0.05*i as f64,a);}
            play.hit(play.song.target(k));
        }
    }
    #[test]
    fn bundled_songs_parse_and_match_their_measured_tempo() {
        let songs=songs();assert_eq!(songs.len(),2);
        for s in &songs {assert!(s.bpm>60.0&&s.bpm<200.0&&s.phrases>0&&s.intro_beats%4==0&&s.file.ends_with(".wav"));}
        // 每句给的时间要够按完箭头：判定点前至少 1 秒
        for s in &songs {assert!(f64::from(s.phrase_beats-1)*s.beat()>=1.0,"{} phrase too short",s.id);}
    }
    #[test]
    fn normal_difficulty_gives_longer_phrases_and_arrows_can_be_entered_early() {
        let s=song(4);
        let normal=Play::new(s.clone(),Difficulty::Normal,1);let hard=Play::new(s.clone(),Difficulty::Hard,1);
        assert_eq!((normal.song.phrase_beats,normal.song.phrases),(8,8));
        assert_eq!((hard.song.phrase_beats,hard.song.phrases),(4,16));
        // 预备段里就按箭头：算数
        let mut play=Play::new(s,Difficulty::Hard,3);
        for a in play.chart[0].clone() {play.arrow(0.5,a);}
        assert!(play.ready());
        play.hit(play.song.target(0));assert_eq!(play.last.map(|l|l.0),Some(Judge::Perfect));
    }
    #[test]
    fn chart_is_deterministic_grows_and_never_triples() {
        let s=song(4);
        assert_eq!(chart(&s,Difficulty::Normal,7),chart(&s,Difficulty::Normal,7));
        assert_ne!(chart(&s,Difficulty::Normal,7),chart(&s,Difficulty::Normal,8));
        for d in [Difficulty::Normal,Difficulty::Hard] {
            let c=chart(&s,d,99);
            assert!(c.windows(2).all(|w|w[0].len()<=w[1].len()));
            assert!(c.iter().all(|l|l.windows(3).all(|w|!(w[0]==w[1]&&w[1]==w[2]))));
        }
        assert_eq!((arrows_in(&s,Difficulty::Normal,0),arrows_in(&s,Difficulty::Hard,15)),(2,6));
        assert_eq!((arrows_in(&song(8),Difficulty::Normal,0),arrows_in(&song(8),Difficulty::Hard,0)),(2,5));
    }
    #[test]
    fn judge_windows_have_exact_boundaries() {
        assert_eq!([judge(0.0),judge(-0.08),judge(0.081),judge(0.15),judge(-0.22),judge(0.30),judge(0.301)],
            [Judge::Perfect,Judge::Perfect,Judge::Great,Judge::Great,Judge::Cool,Judge::Bad,Judge::Miss]);
    }
    #[test]
    fn perfect_run_scores_the_maximum_and_grades_s() {
        let mut play=Play::new(song(4),Difficulty::Hard,3);perfect(&mut play);
        assert!(play.finished(play.song.end()));
        assert!((play.score-play.max_score()).abs()<1e-6);assert_eq!(play.grade(),Grade::S);
        assert_eq!((play.counts[Judge::Perfect as usize],play.max_combo),(16,16));
    }
    #[test]
    fn idle_player_misses_every_phrase_and_grades_c() {
        let mut play=Play::new(song(8),Difficulty::Normal,3);
        let mut t=0.0;while t<play.song.end() {play.tick(t);t+=1.0/60.0;}
        assert_eq!((play.counts[Judge::Miss as usize],play.score,play.grade()),(16,0.0,Grade::C));
    }
    #[test]
    fn wrong_arrow_keeps_progress_and_incomplete_hit_misses() {
        let mut play=Play::new(song(4),Difficulty::Hard,5);
        let start=play.song.phrase_start(0);let line=play.chart[0].clone();
        play.arrow(start,line[0]);play.arrow(start,line[1]);assert_eq!(play.entered,2);
        let wrong=[Arrow::Up,Arrow::Down,Arrow::Left,Arrow::Right].into_iter().find(|a|*a!=line[2]).unwrap();
        play.arrow(start+0.1,wrong);assert_eq!((play.entered,play.wrong),(2,Some(start+0.1)));
        play.hit(play.song.target(0));
        assert_eq!((play.last.map(|l|l.0),play.combo,play.phrase),(Some(Judge::Miss),0,1));
    }
    #[test]
    fn early_confirm_is_ignored_and_late_hits_still_count_within_the_window() {
        let mut play=Play::new(song(4),Difficulty::Normal,5);
        let line=play.chart[0].clone();let start=play.song.phrase_start(0);
        for a in line {play.arrow(start,a);}
        play.hit(play.song.target(0)-0.5);assert_eq!(play.phrase,0,"too early: ignored");
        play.tick(play.song.target(0)+0.26);play.hit(play.song.target(0)+0.26);
        assert_eq!(play.last.map(|l|l.0),Some(Judge::Bad));
        assert_eq!(play.combo,0,"bad breaks the combo");
    }
    #[test]
    fn moves_follow_the_judgement_and_never_repeat_back_to_back() {
        let table=moves();
        assert!(table.iter().all(|(_,a)|(80000..82000).contains(a)),"gesture animations only");
        for tier in ["flashy","light","miss","finale_s","finale_a","finale_b","finale_c"] {assert!(table.iter().any(|(t,_)|t==tier),"{tier}");}
        let flashy:Vec<i32>=table.iter().filter(|(t,_)|t=="flashy").map(|(_,a)|*a).collect();
        let mut previous=None;
        for k in 0..40 {
            let m=move_for(Judge::Perfect,k,k,9,previous).unwrap();
            assert!(flashy.contains(&m));assert_ne!(Some(m),previous);previous=Some(m);
        }
        // 连击很高时只挑最前面几个
        for k in 0..20 {let m=move_for(Judge::Great,60,k,3,None).unwrap();assert!(flashy[..4].contains(&m),"{m}");}
        assert_eq!(move_for(Judge::Miss,0,0,1,None).map(|m|table.iter().any(|(t,a)|t=="miss"&&*a==m)),Some(true));
        assert_eq!(finale(Grade::S),Some(81060));
    }
}
