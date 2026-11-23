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
pub fn draw(ui:&hudhook::imgui::Ui,right_offset:f32) {
    let Some((total,fight,fighting))=CLOCK.try_lock().ok().and_then(|c|c.owner.and(c.last)
        .map(|_|(c.total.as_secs(),(c.fight.is_some()||!c.encounter.is_zero()).then_some(c.encounter.as_secs()),c.fight.is_some()))) else{return};
    use hudhook::imgui::{ImColor32,sys};
    let size=ui.io().display_size;
    let s=(size[1]/1080.0).clamp(0.65,1.8);
    let now=ui.time() as f32;
    let label_px=15.0*s;
    let digit_px=21.0*s;
    let pad_x=14.0*s;
    let pad_y=9.0*s;
    let gap_x=18.0*s;
    let row_gap=7.0*s;
    let dot_w=12.0*s;

    let total_label=crate::i18n::t("本局用时");
    let total_val=format_time(total);
    let fight_row=fight.map(|sec|(crate::i18n::t(if fighting {"本场用时"} else {"上场用时"}),format_time(sec)));

    // 测量宽度：标签用正文字体，数字优先用粗黑体（纯 ASCII，字形必然完整）
    let measure_body=|text:&str,px:f32|->f32 {crate::fancy_text::measure(text,px)};
    let measure_digits=|text:&str,px:f32|->f32 {
        let token=crate::fancy_text::button_font().map(|f|ui.push_font(f));
        let w=crate::fancy_text::measure(text,px);
        drop(token);
        w
    };
    let mut label_w=measure_body(&total_label,label_px);
    let mut digit_w=measure_digits(&total_val,digit_px).max(measure_digits("00:00",digit_px));
    if let Some((ref fl,ref fv))=fight_row {
        label_w=label_w.max(measure_body(fl,label_px));
        digit_w=digit_w.max(measure_digits(fv,digit_px));
    }
    let row_h=digit_px.max(label_px)+2.0*s;
    let rows=if fight_row.is_some(){2}else{1};
    let card_w=pad_x*2.0+dot_w+label_w+gap_x+digit_w;
    let card_h=pad_y*2.0+row_h*rows as f32+if rows==2 {row_gap+1.0*s}else{0.0};
    let margin_r=(24.0*s).max(size[0]*0.016);
    let x1=(size[0]-margin_r-right_offset).max(card_w+12.0*s);
    let x0=x1-card_w;
    let y0=(22.0*s).max(size[1]*0.02);
    let y1=y0+card_h;

    let draw=ui.get_foreground_draw_list();
    let r=4.0*s;
    // 柔和阴影 + 暗金黑曜石面板底
    draw.add_rect([x0+2.0*s,y0+3.0*s],[x1+2.0*s,y1+3.0*s],ImColor32::from_rgba(0,0,0,120)).filled(true).rounding(r).build();
    draw.add_rect_filled_multicolor([x0,y0],[x1,y1],
        ImColor32::from_rgba(14,12,10,206),ImColor32::from_rgba(14,12,10,206),
        ImColor32::from_rgba(8,7,6,222),ImColor32::from_rgba(8,7,6,222));
    // 顶部微弱金光洗染
    draw.add_rect_filled_multicolor([x0,y0],[x1,y0+card_h*0.45],
        ImColor32::from_rgba(218,178,96,24),ImColor32::from_rgba(218,178,96,24),
        ImColor32::from_rgba(218,178,96,0),ImColor32::from_rgba(218,178,96,0));
    // 细金边框与左侧强调竖条
    draw.add_rect([x0,y0],[x1,y1],ImColor32::from_rgba(186,152,88,125)).rounding(r).thickness(1.0*s).build();
    let accent_alpha=if fighting {220} else {160};
    draw.add_rect([x0,y0+2.0*s],[x0+2.5*s,y1-2.0*s],ImColor32::from_rgba(222,182,96,accent_alpha)).filled(true).build();
    // 四个角点细饰
    let tick=5.0*s;
    draw.add_line([x1-tick,y0],[x1,y0],ImColor32::from_rgba(232,198,124,180)).thickness(1.4*s).build();
    draw.add_line([x1,y0],[x1,y0+tick],ImColor32::from_rgba(232,198,124,180)).thickness(1.4*s).build();

    let add_text_sized=|list:&hudhook::imgui::DrawListMut<'_>,pos:[f32;2],px:f32,col:ImColor32,text:&str| {
        let range=text.as_bytes().as_ptr_range();
        let raw=unsafe {sys::igGetForegroundDrawList()};
        let _=list;
        unsafe {
            let font=sys::igGetFont();
            let shadow=ImColor32::from_rgba(0,0,0,215).to_bits();
            sys::ImDrawList_AddText_FontPtr(raw,font,px,sys::ImVec2{x:pos[0]+1.0*s,y:pos[1]+1.2*s},shadow,range.start.cast(),range.end.cast(),0.0,std::ptr::null());
            sys::ImDrawList_AddText_FontPtr(raw,font,px,sys::ImVec2{x:pos[0],y:pos[1]},col.to_bits(),range.start.cast(),range.end.cast(),0.0,std::ptr::null());
        }
    };
    let diamond=|cx:f32,cy:f32,rad:f32,col:ImColor32,filled:bool| {
        let pts=vec![[cx,cy-rad],[cx+rad,cy],[cx,cy+rad],[cx-rad,cy]];
        if filled {draw.add_polyline(pts,col).filled(true).build();}
        else {
            let mut closed=pts;closed.push([cx,cy-rad]);
            draw.add_polyline(closed,col).thickness(1.1*s).build();
        }
    };

    // 第一行：本局用时
    let row0_y=y0+pad_y;
    let cy0=row0_y+row_h*0.5;
    diamond(x0+pad_x+3.5*s,cy0,3.2*s,ImColor32::from_rgba(205,174,112,210),false);
    add_text_sized(&draw,[x0+pad_x+dot_w,cy0-label_px*0.52],label_px,ImColor32::from_rgb(184,174,152),&total_label);
    {
        let token=crate::fancy_text::button_font().map(|f|ui.push_font(f));
        let vw=crate::fancy_text::measure(&total_val,digit_px);
        add_text_sized(&draw,[x1-pad_x-vw,cy0-digit_px*0.52],digit_px,ImColor32::from_rgb(242,232,206),&total_val);
        drop(token);
    }

    // 第二行：本场 / 上场用时
    if let Some((fight_label,fight_val))=fight_row {
        let div_y=row0_y+row_h+row_gap*0.5;
        draw.add_line([x0+pad_x,div_y],[x1-pad_x,div_y],ImColor32::from_rgba(186,152,88,58)).thickness(1.0*s).build();
        let row1_y=div_y+row_gap*0.5+1.0*s;
        let cy1=row1_y+row_h*0.5;
        if fighting {
            let pulse=0.55+0.45*(now*3.4).sin();
            draw.add_circle([x0+pad_x+3.5*s,cy1],5.5*s,ImColor32::from_rgba(235,180,72,(65.0*pulse) as u8)).filled(true).build();
            diamond(x0+pad_x+3.5*s,cy1,3.5*s,ImColor32::from_rgba(244,196,88,(180.0+75.0*pulse) as u8),true);
        } else {
            diamond(x0+pad_x+3.5*s,cy1,3.0*s,ImColor32::from_rgba(150,142,126,180),false);
        }
        let lbl_col=if fighting {ImColor32::from_rgb(224,202,150)} else {ImColor32::from_rgb(154,146,130)};
        let val_col=if fighting {ImColor32::from_rgb(248,210,116)} else {ImColor32::from_rgb(198,190,170)};
        add_text_sized(&draw,[x0+pad_x+dot_w,cy1-label_px*0.52],label_px,lbl_col,&fight_label);
        {
            let token=crate::fancy_text::button_font().map(|f|ui.push_font(f));
            let vw=crate::fancy_text::measure(&fight_val,digit_px);
            add_text_sized(&draw,[x1-pad_x-vw,cy1-digit_px*0.52],digit_px,val_col,&fight_val);
            drop(token);
        }
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
