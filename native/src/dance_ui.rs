// 癫火舞会：跳舞画面（规则在 erdtree_tower::dance）。像跳舞机：角色在场上真的跳，界面只在脚下留一条箭头条与节拍光标。
// 每句判定后挑一个手势动作当舞步（erdtree_tower::dance::move_for），交给游戏线程写进 CSChrEventModule.request_animation_id。
// 时间以舞曲的实际播放位置为准（sfx::Music），开头的预备拍里显示倒数与操作说明；结束后弹出结算卡片。
// 操作：方向键 / WASD / 手柄十字键按箭头，空格 / 手柄 A 在判定点确认；Esc / 手柄 B 放弃（按当前分数结算）。
use std::{cell::RefCell,sync::atomic::{AtomicBool,AtomicI32,Ordering},time::Instant};
use hudhook::imgui::{Condition,ImColor32,Key,Ui,WindowFlags};
use erdtree_tower::dance::{Arrow,Difficulty,Grade,Judge,Play};

static ACTIVE:AtomicBool=AtomicBool::new(false);
static DANCING:AtomicBool=AtomicBool::new(false);
static PENDING_MOVE:AtomicI32=AtomicI32::new(0);
/// 跳舞中或结算卡片还开着（接管输入）
pub fn active()->bool {ACTIVE.load(Ordering::Relaxed)}
/// 舞曲正在放（游戏线程据此暂停背景音乐）
pub fn dancing()->bool {DANCING.load(Ordering::Relaxed)}
/// 游戏线程每帧取一次：要让本机角色做的舞步（手势动画编号）
pub fn take_move()->Option<i32> {Some(PENDING_MOVE.swap(0,Ordering::AcqRel)).filter(|m|*m>0)}
fn perform(anim:Option<i32>) {if let Some(anim)=anim {PENDING_MOVE.store(anim,Ordering::Release);}}

struct Session {play:Play,music:Option<crate::sfx::Music>,clock:Instant,difficulty:Difficulty,result:Option<f64>,quit:bool,seed:u64,last_move:Option<i32>}
thread_local! {static SESSION:RefCell<Option<Session>>=const {RefCell::new(None)};}

/// 开始跳一首（界面线程调用）；index 超出范围时取第一首
pub fn start(index:usize,difficulty:Difficulty,seed:u64) {
    let songs=erdtree_tower::dance::songs();
    let Some(song)=songs.get(index).or(songs.first()).cloned() else {crate::log("Dance: no songs");return;};
    let music=crate::sfx::Music::start(&song.file);
    crate::log(format!("DANCE start song={} bpm={} difficulty={difficulty:?} music={}",song.id,song.bpm,music.is_some()));
    SESSION.with(|s|*s.borrow_mut()=Some(Session{play:Play::new(song,difficulty,seed),music,clock:Instant::now(),difficulty,result:None,quit:false,seed,last_move:None}));
    ACTIVE.store(true,Ordering::Relaxed);DANCING.store(true,Ordering::Relaxed);
}

const AMBER:[f32;4]=[0.91,0.71,0.29,1.0];const WHITE:[f32;4]=[0.95,0.93,0.88,1.0];const MUTED:[f32;4]=[0.66,0.63,0.57,1.0];
const RED:[f32;4]=[0.92,0.3,0.25,1.0];const INK:[f32;4]=[0.07,0.07,0.06,1.0];
fn col(c:[f32;4],a:f32)->ImColor32 {ImColor32::from_rgba((c[0]*255.0) as u8,(c[1]*255.0) as u8,(c[2]*255.0) as u8,(c[3]*a.clamp(0.0,1.0)*255.0) as u8)}
fn fade(c:[f32;4],k:f32)->[f32;4] {[c[0],c[1],c[2],c[3]*k.clamp(0.0,1.0)]}
fn ease(t:f64,start:f64,len:f64)->f32 {let x=((t-start)/len).clamp(0.0,1.0) as f32;1.0-(1.0-x).powi(3)}
fn judge_color(j:Judge)->[f32;4] {match j {Judge::Perfect=>[1.0,0.84,0.38,1.0],Judge::Great=>[0.55,0.86,1.0,1.0],Judge::Cool=>[0.6,0.9,0.55,1.0],Judge::Bad=>[0.85,0.62,0.95,1.0],Judge::Miss=>RED}}
fn grade_color(g:Grade)->[f32;4] {match g {Grade::S=>[1.0,0.84,0.38,1.0],Grade::A=>[0.95,0.6,0.35,1.0],Grade::B=>[0.55,0.8,1.0,1.0],Grade::C=>MUTED}}

/// 每帧画（返回 true 表示跳舞画面占着屏幕，其余界面不画）
pub fn draw(ui:&Ui)->bool {
    SESSION.with(|cell|{
        let mut slot=cell.borrow_mut();
        let Some(session)=slot.as_mut() else {ACTIVE.store(false,Ordering::Relaxed);return false;};
        let t=session.music.as_ref().map_or_else(||session.clock.elapsed().as_secs_f64(),|m|m.position());
        if session.result.is_none() {
            let pressed=|keys:&[Key]|keys.iter().any(|k|ui.is_key_pressed_no_repeat(*k));
            for (keys,arrow) in [(&[Key::UpArrow,Key::W,Key::GamepadDpadUp][..],Arrow::Up),(&[Key::DownArrow,Key::S,Key::GamepadDpadDown][..],Arrow::Down),
                (&[Key::LeftArrow,Key::A,Key::GamepadDpadLeft][..],Arrow::Left),(&[Key::RightArrow,Key::D,Key::GamepadDpadRight][..],Arrow::Right)] {
                if pressed(keys) {session.play.arrow(t,arrow);}
            }
            let before=session.play.phrase;
            if pressed(&[Key::Space,Key::GamepadFaceDown]) {
                session.play.hit(t);
                if session.play.phrase!=before {crate::sfx::play(if session.play.last.is_some_and(|l|l.0>=Judge::Great){"tick1"}else{"tick3"});}
            }
            if pressed(&[Key::Escape,Key::GamepadFaceRight]) {session.quit=true;}
            session.play.tick(t);
            // 这一帧判定了一句：角色做对应的舞步
            if session.play.phrase!=before && let Some((judge,_))=session.play.last {
                let anim=erdtree_tower::dance::move_for(judge,session.play.combo,session.play.phrase,session.seed,session.last_move);
                if anim.is_some() {session.last_move=anim;}
                perform(anim);
            }
            if session.quit||session.play.finished(t)||session.music.as_ref().is_some_and(|m|t>=m.length()-0.05&&session.play.phrase>=session.play.song.phrases) {
                session.result=Some(ui.time());session.music=None;DANCING.store(false,Ordering::Relaxed);
                perform(erdtree_tower::dance::finale(session.play.grade()));
                let p=&session.play;
                crate::log(format!("DANCE end song={} difficulty={:?} quit={} score={:.0}/{:.0} grade={} max_combo={} counts={:?}",
                    p.song.id,session.difficulty,session.quit,p.score,p.max_score(),p.grade().name(),p.max_combo,p.counts));
                crate::sfx::play("total");
            }
        }
        let close=match session.result {
            None=>{if crate::dance_art::ready() {stage(ui,&session.play,t);} else {stage_plain(ui,&session.play,t);}false},
            Some(at)=>result(ui,&session.play,session.quit,ui.time()-at),
        };
        if close {*slot=None;ACTIVE.store(false,Ordering::Relaxed);DANCING.store(false,Ordering::Relaxed);crate::sfx::play("close");}
        true
    })
}

/// 画布 1600×900 等比缩放；字体规则与 notice_ui 相同（纯英文黑体、22 像素以上标题宋体、其余正文宋体）
struct Canvas<'a> {ui:&'a Ui,s:f32,ox:f32,oy:f32}
impl Canvas<'_> {
    fn new(ui:&Ui)->Canvas<'_> {let size=ui.io().display_size;let s=(size[0]/1600.0).min(size[1]/900.0);Canvas{ui,s,ox:(size[0]-1600.0*s)*0.5,oy:(size[1]-900.0*s)*0.5}}
    fn p(&self,x:f32,y:f32)->[f32;2] {[self.ox+x*self.s,self.oy+y*self.s]}
    fn font(&self,value:&str,px:f32)->(Option<hudhook::imgui::FontStackToken<'_>>,f32) {
        use crate::fancy_text as ft;
        if value.is_ascii() {if let Some(f)=ft::button_font() {return (Some(self.ui.push_font(f)),ft::SANS_PIXELS);}}
        if px>=22.0 {if let Some(f)=ft::title_font() {return (Some(self.ui.push_font(f)),32.0);}}
        (None,20.0)
    }
    fn measure(&self,value:&str,px:f32)->f32 {let (_f,base)=self.font(value,px);self.ui.set_window_font_scale(px/base*self.s);self.ui.calc_text_size(value)[0]/self.s}
    fn text(&self,x:f32,y:f32,value:&str,color:[f32;4],px:f32,align:f32) {
        if color[3]<=0.01 {return;}
        let (_f,base)=self.font(value,px);self.ui.set_window_font_scale(px/base*self.s);
        let w=self.ui.calc_text_size(value)[0];
        // 深色描边，叠在游戏画面上也看得清
        for (dx,dy) in [(-1.5,0.0),(1.5,0.0),(0.0,-1.5),(0.0,1.5)] {
            self.ui.set_cursor_screen_pos([self.ox+x*self.s-w*align+dx,self.oy+y*self.s+dy]);self.ui.text_colored([0.0,0.0,0.0,color[3]*0.7],value);
        }
        self.ui.set_cursor_screen_pos([self.ox+x*self.s-w*align,self.oy+y*self.s]);self.ui.text_colored(color,value);
    }
}
fn window(ui:&Ui,id:&str,f:impl FnOnce()) {
    let size=ui.io().display_size;
    ui.window(id).position([0.0,0.0],Condition::Always).size(size,Condition::Always)
        .flags(WindowFlags::NO_DECORATION|WindowFlags::NO_BACKGROUND|WindowFlags::NO_MOVE|WindowFlags::NO_NAV|WindowFlags::NO_INPUTS|WindowFlags::NO_SCROLL_WITH_MOUSE).build(f);
}
/// 一个方向箭头（三角形箭头 + 箭杆）
fn arrow_glyph(draw:&hudhook::imgui::DrawListMut<'_>,c:[f32;2],r:f32,arrow:Arrow,color:ImColor32) {
    let (dx,dy)=match arrow {Arrow::Up=>(0.0,-1.0),Arrow::Down=>(0.0,1.0),Arrow::Left=>(-1.0,0.0),Arrow::Right=>(1.0,0.0)};
    let (px,py)=(-dy,dx);
    let tip=[c[0]+dx*r,c[1]+dy*r];let base=[c[0]+dx*r*0.05,c[1]+dy*r*0.05];
    draw.add_triangle(tip,[base[0]+px*r*0.8,base[1]+py*r*0.8],[base[0]-px*r*0.8,base[1]-py*r*0.8],color).filled(true).build();
    let tail=[c[0]-dx*r*0.85,c[1]-dy*r*0.85];let w=r*0.28;
    draw.add_polyline(vec![[base[0]+px*w,base[1]+py*w],[tail[0]+px*w,tail[1]+py*w],[tail[0]-px*w,tail[1]-py*w],[base[0]-px*w,base[1]-py*w]],color).filled(true).build();
}

/// 跳舞中的界面：顶部一枚舞曲名牌、右上分数，脚下一条半透明发光条（节拍光标在上、这一句的箭头在下），判定字弹在脚边
fn stage_plain(ui:&Ui,play:&Play,t:f64) {
    window(ui,"##dance-stage",||{
        let c=Canvas::new(ui);let s=c.s;let song=&play.song;let beat=song.beat();
        let draw=ui.get_window_draw_list();
        // ── 顶部名牌：舞会名 · 舞曲名，右边 BPM ──
        let title=format!("癫火舞会 · {}",song.name);let tw=c.measure(&title,18.0)+150.0;
        draw.add_rect(c.p(800.0-tw*0.5,14.0),c.p(800.0+tw*0.5,50.0),ImColor32::from_rgba(12,8,6,150)).filled(true).rounding(18.0*s).build();
        draw.add_rect(c.p(800.0-tw*0.5,14.0),c.p(800.0+tw*0.5,50.0),col(AMBER,0.45)).thickness(1.2*s).rounding(18.0*s).build();
        drop(draw);
        c.text(800.0-tw*0.5+22.0,21.0,&title,AMBER,18.0,0.0);
        c.text(800.0+tw*0.5-22.0,23.0,&format!("{:.0} BPM",song.bpm),MUTED,15.0,1.0);
        // 右上：分数与连击
        c.text(1560.0,14.0,&format!("{:.0}",play.score),WHITE,30.0,1.0);
        if play.combo>=2 {c.text(1560.0,52.0,&format!("{} 连击",play.combo),AMBER,18.0,1.0);}
        // ── 预备：倒数（最后一小节 3、2、1、开跳）与一行操作说明 ──
        let first=song.phrase_start(0);
        if t<first {
            c.text(800.0,866.0,"方向键按出箭头，光标走到金圈时按空格　·　手柄：十字键 + A　·　Esc 放弃",fade(WHITE,0.85),15.0,0.5);
            let left=((first-t)/beat).ceil() as i32;
            if left<=4 {
                let k=1.0-(((first-t)/beat).fract() as f32);
                c.text(800.0,330.0-k*10.0,if left<=1 {"开跳！"} else {["","","3","2","1"][left.clamp(0,4) as usize]},fade(AMBER,1.0-k*0.6),if left<=1{60.0}else{76.0},0.5);
            }
        }
        // ── 脚下的发光条 ──
        let k=play.phrase.min(song.phrases.saturating_sub(1));
        let (start,target)=(song.phrase_start(k),song.target(k));let span=f64::from(song.phrase_beats)*beat;
        let (x0,x1)=(480.0f32,1120.0f32);let (top,bottom)=(748.0f32,846.0f32);let ty=766.0f32;
        let at=|time:f64|x0+28.0+(x1-x0-56.0)*((time-start)/span).clamp(0.0,1.0) as f32;
        let pulse=0.5+0.5*((t/beat*std::f64::consts::TAU).cos() as f32);
        {let draw=ui.get_window_draw_list();
            // 外发光（几层渐淡的描边）+ 半透明底
            for (i,a) in [0.10f32,0.06,0.03].into_iter().enumerate() {let d=(3.0+i as f32*4.0)*s;
                draw.add_rect([c.p(x0,top)[0]-d,c.p(x0,top)[1]-d],[c.p(x1,bottom)[0]+d,c.p(x1,bottom)[1]+d],col(AMBER,a*(0.7+0.3*pulse))).thickness(4.0*s).rounding(20.0*s).build();}
            draw.add_rect(c.p(x0,top),c.p(x1,bottom),ImColor32::from_rgba(10,7,5,150)).filled(true).rounding(16.0*s).build();
            draw.add_rect(c.p(x0,top),c.p(x1,bottom),col(AMBER,0.5)).thickness(1.4*s).rounding(16.0*s).build();
            // 节拍轨道：细线 + 每拍刻度 + 判定区与金圈 + 光标
            draw.add_rect(c.p(x0+28.0,ty-1.5),c.p(x1-28.0,ty+1.5),col(WHITE,0.22)).filled(true).build();
            for b in 0..song.phrase_beats {let x=at(start+f64::from(b)*beat);draw.add_rect(c.p(x-1.0,ty-6.0),c.p(x+1.0,ty+6.0),col(WHITE,0.4)).filled(true).build();}
            let tx=at(target);let px=(x1-x0-56.0)/span as f32;
            draw.add_rect(c.p(tx-0.110*px,ty-7.0),c.p(tx+0.110*px,ty+7.0),col(AMBER,0.25)).filled(true).rounding(3.0*s).build();
            draw.add_rect(c.p(tx-0.060*px,ty-7.0),c.p(tx+0.060*px,ty+7.0),col(AMBER,0.6)).filled(true).rounding(3.0*s).build();
            draw.add_circle(c.p(tx,ty),(11.0+3.0*pulse)*s,col(AMBER,0.55+0.4*pulse)).thickness(2.0*s).build();
            if t>=start-0.05 {let x=at(t);draw.add_circle(c.p(x,ty),5.5*s,col(WHITE,0.95)).filled(true).build();
                draw.add_circle(c.p(x,ty),9.0*s,col(WHITE,0.25)).filled(true).build();}
        }
        // 句数（截图里的 level）
        c.text(x0-16.0,782.0,&format!("第 {} 句",(play.phrase+1).min(song.phrases)),fade(MUTED,0.9),15.0,1.0);
        // 这一句的箭头
        if let Some(line)=play.current() {
            let open=play.open(t);let n=line.len() as f32;let box_=46.0;let gap=10.0;
            let left=800.0-(n*box_+(n-1.0)*gap)*0.5;let y=784.0;
            let wrong=play.wrong.map_or(0.0,|w|1.0-((t-w)/0.3).clamp(0.0,1.0) as f32);
            let draw=ui.get_window_draw_list();
            for (i,arrow) in line.iter().enumerate() {
                let x=left+i as f32*(box_+gap);let (min,max)=(c.p(x,y),c.p(x+box_,y+box_));
                let done=i<play.entered;
                if done {draw.add_rect([min[0]-3.0*s,min[1]-3.0*s],[max[0]+3.0*s,max[1]+3.0*s],col(AMBER,0.25)).filled(true).rounding(10.0*s).build();}
                draw.add_rect(min,max,if done {col(AMBER,0.95)} else {ImColor32::from_rgba(24,20,16,if open{220}else{120})}).filled(true).rounding(8.0*s).build();
                draw.add_rect(min,max,if wrong>0.0 {col(RED,wrong)} else {col(WHITE,if open{0.3}else{0.12})}).thickness(1.6*s).rounding(8.0*s).build();
                arrow_glyph(&draw,c.p(x+box_*0.5,y+box_*0.5),14.0*s,*arrow,if done {col(INK,1.0)} else {col(WHITE,if open{0.95}else{0.35})});
            }
        }
        // ── 判定字：弹在发光条上方、角色脚边 ──
        if let Some((judge,at_))=play.last {
            let age=t-at_;
            if age<0.7 {
                let k=ease(age,0.0,0.12);let a=1.0-((age-0.35)/0.35).clamp(0.0,1.0) as f32;
                c.text(800.0,690.0-k*12.0,judge.name(),fade(judge_color(judge),a),30.0+10.0*(1.0-k),0.5);
                if judge.keeps_combo()&&play.combo>=2 {c.text(800.0,730.0-k*6.0,&format!("{} 连击",play.combo),fade(WHITE,a),16.0,0.5);}
            }
        }
        ui.set_window_font_scale(1.0);
    });
}

/// 舞会的名字（还没定，先用占位；名牌贴图本身不带字）
const TITLE:&str="舞会";
const PINK:[f32;4]=[0.91,0.38,0.63,1.0];
/// 粉色梦幻风的跳舞界面（贴图见 dance_art）：顶部蕾丝名牌、右上分数，脚下粉色玻璃条——上半是玫粉轨道、星星光标与爱心判定点，
/// 下半是这一句的宝石箭头（按对的亮粉、没按的压暗）；判定字是带小牌子的贴图，弹在玻璃条上方
fn stage(ui:&Ui,play:&Play,t:f64) {
    use crate::dance_art::{texture,aspect};
    window(ui,"##dance-stage",||{
        let c=Canvas::new(ui);let s=c.s;let song=&play.song;let beat=song.beat();
        let image=|name:&str,x:f32,y:f32,w:f32,h:f32,tint:[f32;4]| {
            if let Some(id)=texture(name) {ui.get_window_draw_list().add_image(id,c.p(x,y),c.p(x+w,y+h)).col(col(tint,1.0)).build();}
        };
        let outlined=|x:f32,y:f32,value:&str,color:[f32;4],px:f32,align:f32| {
            // 白色粗描边的粉字（底下的 text 自带一层深色描边，这里先垫一圈白）
            for (dx,dy) in [(-2.0,0.0),(2.0,0.0),(0.0,-2.0),(0.0,2.0),(-1.5,-1.5),(1.5,1.5),(-1.5,1.5),(1.5,-1.5)] {c.text(x+dx,y+dy,value,[1.0,1.0,1.0,color[3]],px,align);}
            c.text(x,y,value,color,px,align);
        };
        let pulse=0.5+0.5*((t/beat*std::f64::consts::TAU).cos() as f32);
        // ── 顶部名牌与分数 ──
        let pw=320.0;image("plate",800.0-pw*0.5,6.0,pw,pw*aspect("plate"),WHITE_FULL);
        c.text(800.0,36.0,&format!("{TITLE} · {}",song.name),PINK,20.0,0.5);
        outlined(1560.0,14.0,&format!("{:.0}",play.score),PINK,30.0,1.0);
        // ── 预备：倒数与说明 ──
        let first=song.phrase_start(0);
        if t<first {
            outlined(800.0,880.0,"方向键按出箭头，星星走到爱心时按空格　·　手柄：十字键 + A　·　Esc 放弃",[0.91,0.38,0.63,0.95],15.0,0.5);
            let left=((first-t)/beat).ceil() as i32;
            if left<=4 {
                let k=1.0-(((first-t)/beat).fract() as f32);
                outlined(800.0,330.0-k*10.0,if left<=1 {"Dance!"} else {["","","3","2","1"][left.clamp(0,4) as usize]},fade(PINK,1.0-k*0.5),if left<=1{60.0}else{76.0},0.5);
            }
        }
        // ── 玻璃条 ──
        let (bw,bx)=(660.0f32,470.0f32);let bh=bw*aspect("bar");let by=878.0-bh;
        image("bar",bx,by,bw,bh,WHITE_FULL);
        let k=play.phrase.min(song.phrases.saturating_sub(1));
        let (start,target)=(song.phrase_start(k),song.target(k));let span=f64::from(song.phrase_beats)*beat;
        let (x0,x1)=(bx+bw*0.14,bx+bw*0.86);let ty=by+bh*0.33;
        let at=|time:f64|x0+(x1-x0)*((time-start)/span).clamp(0.0,1.0) as f32;
        {let draw=ui.get_window_draw_list();
            draw.add_line(c.p(x0,ty),c.p(x1,ty),col(WHITE,0.75)).thickness(8.0*s).build();
            draw.add_line(c.p(x0,ty),c.p(x1,ty),col(PINK,1.0)).thickness(3.5*s).build();
            for b in 0..song.phrase_beats {let x=at(start+f64::from(b)*beat);draw.add_circle(c.p(x,ty),4.5*s,col(WHITE,1.0)).filled(true).build();draw.add_circle(c.p(x,ty),3.0*s,col(PINK,1.0)).filled(true).build();}
            // 完美窗口：轨道上一段加亮
            let tx=at(target);let px=(x1-x0)/span as f32;
            draw.add_line(c.p(tx-0.06*px,ty),c.p(tx+0.06*px,ty),col([1.0,0.82,0.92,1.0],0.9)).thickness(7.0*s).build();
            drop(draw);
            let hs=60.0+6.0*pulse;image("target",tx-hs*0.5,ty-hs*0.5,hs,hs,WHITE_FULL);
            if t>=start-0.05 {let cs=46.0;image("cursor",at(t)-cs*0.5,ty-cs*0.5,cs,cs,WHITE_FULL);}
        }
        // ── 宝石箭头 ──
        if let Some(line)=play.current() {
            let open=play.open(t);let n=line.len() as f32;let size=(bh*0.44).min(560.0/n);let gap=4.0;
            let left=800.0-(n*size+(n-1.0)*gap)*0.5;let ay=by+bh*0.46;
            let wrong=play.wrong.map_or(0.0,|w|1.0-((t-w)/0.3).clamp(0.0,1.0) as f32);
            for (i,arrow) in line.iter().enumerate() {
                let dir=match arrow {Arrow::Up=>"up",Arrow::Down=>"down",Arrow::Left=>"left",Arrow::Right=>"right"};
                let done=i<play.entered;
                let tint=if wrong>0.0 {[1.0,0.55+0.45*(1.0-wrong),0.6+0.4*(1.0-wrong),1.0]} else if done {WHITE_FULL} else if open {[0.82,0.74,0.84,1.0]} else {[0.82,0.74,0.84,0.5]};
                let grow=if done {4.0} else {0.0};
                image(&format!("arrow_{}_{dir}",if done{"lit"}else{"dim"}),left+i as f32*(size+gap)-grow*0.5,ay-grow*0.5,size+grow,size+grow,tint);
            }
        }
        // ── 判定字（贴图）与连击 ──
        if let Some((judge,at_))=play.last {
            let age=t-at_;
            if age<0.75 {
                let pop=ease(age,0.0,0.14);let a=1.0-((age-0.4)/0.35).clamp(0.0,1.0) as f32;
                let w=230.0*(1.25-0.25*pop);let h=w*aspect("judge_perfect");
                let name=match judge {Judge::Perfect=>"judge_perfect",Judge::Great=>"judge_great",Judge::Cool=>"judge_cool",Judge::Bad=>"judge_bad",Judge::Miss=>"judge_miss"};
                image(name,800.0-w*0.5,by-h-8.0-(1.0-pop)*10.0,w,h,[1.0,1.0,1.0,a]);
                if judge.keeps_combo()&&play.combo>=2 {outlined(800.0,by-6.0,&format!("{} COMBO",play.combo),fade(PINK,a),18.0,0.5);}
            }
        }
        ui.set_window_font_scale(1.0);
    });
}
const WHITE_FULL:[f32;4]=[1.0,1.0,1.0,1.0];

fn result(ui:&Ui,play:&Play,quit:bool,age:f64)->bool {
    let size=ui.io().display_size;
    {let bg=ui.get_background_draw_list();bg.add_rect([0.0,0.0],size,ImColor32::from_rgba(5,4,3,(190.0*ease(age,0.0,0.3)) as u8)).filled(true).build();}
    let ready=age>=0.6;
    let mut close=ready&&[Key::Enter,Key::Space,Key::GamepadFaceDown,Key::Escape,Key::GamepadFaceRight].iter().any(|k|ui.is_key_pressed_no_repeat(*k));
    let size=ui.io().display_size;
    ui.window("##dance-result").position([0.0,0.0],Condition::Always).size(size,Condition::Always)
        .flags(WindowFlags::NO_DECORATION|WindowFlags::NO_BACKGROUND|WindowFlags::NO_MOVE|WindowFlags::NO_NAV|WindowFlags::NO_SCROLL_WITH_MOUSE).build(||{
        let c=Canvas::new(ui);let s=c.s;let a=ease(age,0.0,0.35);let rise=(1.0-a)*20.0;
        let (left,top,right,bottom)=(500.0,180.0+rise,1100.0,720.0+rise);
        {let draw=ui.get_window_draw_list();crate::law_ui::panel(&draw,c.p(left,top),c.p(right,bottom),s);
            crate::law_ui::ornament(&draw,c.p(800.0,top+118.0),140.0*s*ease(age,0.1,0.4),s);}
        c.text(800.0,top+30.0,"癫火舞会 · 结算",fade(MUTED,a),18.0,0.5);
        c.text(800.0,top+58.0,&play.song.name,fade(WHITE,a),30.0,0.5);
        let grade=play.grade();let pop=ease(age,0.25,0.35);
        c.text(800.0,top+140.0+(1.0-pop)*20.0,grade.name(),fade(grade_color(grade),pop),110.0,0.5);
        c.text(800.0,top+268.0,&format!("{:.0}",play.score),fade(WHITE,a),36.0,0.5);
        c.text(800.0,top+314.0,&format!("满分 {:.0}　·　最高连击 {}",play.max_score(),play.max_combo),fade(MUTED,a),16.0,0.5);
        for (i,judge) in [Judge::Perfect,Judge::Great,Judge::Cool,Judge::Bad,Judge::Miss].into_iter().enumerate() {
            let x=560.0+i as f32*120.0;
            c.text(x,top+360.0,judge.name(),fade(judge_color(judge),a),18.0,0.0);
            c.text(x,top+388.0,&play.counts[judge as usize].to_string(),fade(WHITE,a),24.0,0.0);
        }
        if quit {c.text(800.0,top+440.0,"中途放弃，按已跳的部分结算",fade(RED,a),16.0,0.5);}
        // 确认按钮
        let (bx,by,bw)=(680.0,bottom-80.0,240.0);
        ui.set_cursor_screen_pos(c.p(bx,by));ui.set_window_font_scale(s);
        if ui.invisible_button("##dance-ok",[bw*s,52.0*s])&&ready {close=true;}
        let hover=ui.is_item_hovered();
        {let draw=ui.get_window_draw_list();let f=ease(age,0.4,0.3);
            draw.add_rect(c.p(bx,by),c.p(bx+bw,by+52.0),col(AMBER,f)).filled(true).build();
            if hover {draw.add_rect(c.p(bx,by),c.p(bx+bw,by+52.0),col([1.0,0.86,0.5,1.0],0.5*f)).filled(true).build();}}
        c.text(800.0,by+12.0,"确定",fade(INK,ease(age,0.4,0.3)),24.0,0.5);
        ui.set_window_font_scale(1.0);
    });
    close
}
