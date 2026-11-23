use std::{fs, io, path::Path, sync::{LazyLock,Mutex}, time::{Duration,Instant}};
use erdtree_tower::Run;

type Owner=(u32,i32);
type Fight=(u8,usize,u64);
#[derive(Default)]
struct Clock {
    owner:Option<(Owner,u64)>,
    total:Duration,
    encounter:Duration,
    fight:Option<Fight>,
    last:Option<Instant>,
    saved:Option<Instant>,
}
static CLOCK:LazyLock<Mutex<Clock>>=LazyLock::new(Default::default);

impl Clock {
    fn advance(&mut self,now:Instant) {
        if let Some(last)=self.last {
            let delta=now.saturating_duration_since(last);
            self.total+=delta;
            if self.fight.is_some(){self.encounter+=delta;}
        }
        self.last=Some(now);
    }
    fn update(&mut self,now:Instant,fight:Option<Fight>) {
        self.advance(now);
        if self.fight!=fight && fight.is_some(){self.encounter=Duration::ZERO;}
        self.fight=fight;
    }
    fn save(&mut self,now:Instant) {
        let Some((owner,seed))=self.owner else{return};
        let (act,node,wave)=self.fight.unwrap_or_default();
        let text=format!("{seed} {} {} {act} {node} {wave}",self.total.as_millis(),self.encounter.as_millis());
        let path=super::hub::path(owner,"timer");
        let tmp=path.with_extension("timer.tmp");
        if let Err(e)=fs::write(&tmp,text).and_then(|_|fs::rename(&tmp,&path)){
            super::log(format!("Run timer checkpoint failed: {e}"));
        }
        self.saved=Some(now);
    }
    fn load(path:&Path,seed:u64)->io::Result<Self> {
        let text=fs::read_to_string(path)?;
        Ok(Self::decode(&text,seed).unwrap_or_default())
    }
    fn decode(text:&str,seed:u64)->Option<Self> {
        let mut v=text.split_whitespace();
        if v.next()?.parse::<u64>().ok()?!=seed {return None;}
        let total=Duration::from_millis(v.next()?.parse().ok()?);
        let encounter=Duration::from_millis(v.next()?.parse().ok()?);
        let act=v.next()?.parse::<u8>().ok()?;
        let node=v.next()?.parse().ok()?;
        let wave=v.next()?.parse().ok()?;
        Some(Self{total,encounter,fight:(act>0).then_some((act,node,wave)),..Default::default()})
    }
}
fn seed(run:&Run)->u64 {run.initial_seed.unwrap_or(run.seed)}
pub fn new_run(owner:Owner,run:&Run) {
    if let Ok(mut c)=CLOCK.lock(){
        *c=Clock{owner:Some((owner,seed(run))),..Default::default()};
        c.save(Instant::now());
    }
}
pub fn tick(active:Option<(Owner,&Run)>,fight:Option<(usize,u64)>) {
    let Ok(mut c)=CLOCK.lock() else{return};
    let now=Instant::now();
    let key=active.map(|(owner,run)|(owner,seed(run)));
    if c.owner!=key {
        if c.owner.is_some() && c.last.is_some(){c.advance(now);c.save(now);}
        *c=key.and_then(|(owner,seed)|Clock::load(&super::hub::path(owner,"timer"),seed).ok()).unwrap_or_default();
        c.owner=key;
    }
    let Some((_,run))=active else{return};
    let fight=fight.map(|(node,wave)|(run.act,node,wave));
    let changed=c.fight!=fight;
    c.update(now,fight);
    if changed || c.saved.is_none_or(|at|now.duration_since(at)>=Duration::from_secs(5)){c.save(now);}
}
pub fn total(owner:Owner,run:&Run)->Option<u64> {
    let key=seed(run);
    if let Ok(c)=CLOCK.lock() && c.owner==Some((owner,key)){return Some(c.total.as_secs());}
    let text=fs::read_to_string(super::hub::path(owner,"timer")).ok()?;
    Clock::decode(&text,key).map(|c|c.total.as_secs())
}
pub fn format_time(seconds:u64)->String {
    if seconds>=3600 {format!("{}:{:02}:{:02}",seconds/3600,seconds/60%60,seconds%60)}
    else {format!("{:02}:{:02}",seconds/60,seconds%60)}
}
pub fn draw(ui:&hudhook::imgui::Ui) {
    let Some((total,fight,fighting))=CLOCK.try_lock().ok().and_then(|c|c.owner.and(c.last)
        .map(|_|(c.total.as_secs(),(c.fight.is_some()||!c.encounter.is_zero()).then_some(c.encounter.as_secs()),c.fight.is_some()))) else{return};
    use hudhook::imgui::ImColor32;
    let draw=ui.get_foreground_draw_list();
    let size=ui.io().display_size;
    let right=size[0]-0.025*size[0];
    let mut y=size[1]*0.06;
    for (label,seconds) in [("本局用时",Some(total)),(if fighting {"本场用时"} else {"上场用时"},fight)] {
        let Some(seconds)=seconds else{continue};
        let text=format!("{}  {}",crate::i18n::t(label),format_time(seconds));
        let x=right-ui.calc_text_size(&text)[0];
        draw.add_text([x+1.0,y+1.0],ImColor32::from_rgba(0,0,0,220),&text);
        draw.add_text([x,y],ImColor32::from_rgb(234,221,190),&text);
        y+=ui.text_line_height_with_spacing();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn first_spawn_starts_clock_waves_continue_and_next_fight_resets() {
        let t=Instant::now();let mut c=Clock::default();
        let battle=Some((1,4,0));
        for (n,f) in [(0,None),(8,battle),(12,battle),(18,None),(25,Some((1,5,0)))] {
            c.update(t+Duration::from_secs(n),f);
            if n==18 {assert_eq!(c.encounter.as_secs(),10);}
        }
        assert_eq!(c.total.as_secs(),25);assert_eq!(c.encounter,Duration::ZERO);
    }
    #[test]
    fn restored_clock_excludes_offline_time_and_checks_seed() {
        let text="12 95000 4000 2 3 0";
        assert!(Clock::decode(text,11).is_none());
        let mut c=Clock::decode(text,12).unwrap();let now=Instant::now();
        c.update(now,Some((2,3,0)));c.update(now+Duration::from_secs(2),Some((2,3,0)));
        assert_eq!((c.total.as_secs(),c.encounter.as_secs()),(97,6));
        assert_eq!(format_time(3661),"1:01:01");
    }
}
