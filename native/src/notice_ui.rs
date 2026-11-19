// 联机开局失败的提示与试炼（史东薇尔突围、魔法师塔隐形桥等跑酷路线）完成后的护符赐福结果：
// 与苦难律法、选择职业同一套视觉——压暗原画背景、金边暗色面板、居中大标题与装饰线、琥珀色实心按钮。
// 失败用红色强调（左侧竖条、徽记、原因框），成功用金色并逐行滑入获得的赐福（图标 + 名称 + 效果）。
use hudhook::imgui::{Condition,ImColor32,Key,TextureId,Ui,WindowFlags};

const AMBER:[f32;4]=[0.91,0.71,0.29,1.0];const WHITE:[f32;4]=[0.93,0.92,0.88,1.0];const MUTED:[f32;4]=[0.62,0.6,0.55,1.0];
const INK:[f32;4]=[0.07,0.07,0.06,1.0];const RED:[f32;4]=[0.86,0.27,0.23,1.0];
/// 卡片宽度与左右内边距（1600×900 画布）
const CARD_W:f32=760.0;const PAD:f32=56.0;

pub enum Kind<'a> {
    /// 联机无法开始：原因（可能多行）
    Failure{message:&'a str},
    /// 试炼完成：路线名与新获得的护符赐福编号
    Trial{route:&'a str,items:&'a [u32]},
}
fn col(c:[f32;4],a:f32)->ImColor32 {ImColor32::from_rgba((c[0]*255.0) as u8,(c[1]*255.0) as u8,(c[2]*255.0) as u8,(c[3]*a.clamp(0.0,1.0)*255.0) as u8)}
fn fade(c:[f32;4],k:f32)->[f32;4] {[c[0],c[1],c[2],c[3]*k.clamp(0.0,1.0)]}
fn ease(elapsed:f64,start:f64,len:f64)->f32 {let t=((elapsed-start)/len).clamp(0.0,1.0) as f32;1.0-(1.0-t).powi(3)}

/// 画一帧；返回 true 表示玩家确认关闭。opened 是首次显示的时间（入场动画与防误触）
pub fn draw(ui:&Ui,kind:Kind<'_>,opened:f64,icon:&mut dyn FnMut(u32)->Option<TextureId>)->bool {
    let size=ui.io().display_size;let s=(size[0]/1600.0).min(size[1]/900.0);
    let (ox,oy)=((size[0]-1600.0*s)*0.5,(size[1]-900.0*s)*0.5);let p=|x:f32,y:f32|[ox+x*s,oy+y*s];
    let now=ui.time();let t=now-opened;let time=now as f32;
    let failure=matches!(kind,Kind::Failure{..});
    let accent=if failure{RED}else{AMBER};
    // ── 背景：压暗的原画与缓慢漂移的细斜纹 ──
    let background=ui.get_background_draw_list();crate::oath_art::background(&background,size);
    background.add_rect([0.0,0.0],size,ImColor32::from_rgba(13,13,12,(150.0+70.0*ease(t,0.0,0.3)) as u8)).filled(true).build();
    let drift=(time*9.0*s)%(28.0*s);
    let mut x=-size[1]+drift;while x<size[0] {background.add_line([x,size[1]],[x+size[1],0.0],ImColor32::from_rgba(255,255,255,5)).thickness(1.0).build();x+=28.0*s;}
    drop(background);
    let ready=t>=0.35;
    let mut close=ready&&([Key::Enter,Key::GamepadFaceDown].into_iter().any(|k|ui.is_key_pressed_no_repeat(k))
        ||failure&&[Key::Escape,Key::GamepadFaceRight].into_iter().any(|k|ui.is_key_pressed_no_repeat(k)));
    ui.window("##tower-notice").position([0.0,0.0],Condition::Always).size(size,Condition::Always)
        .flags(WindowFlags::NO_DECORATION|WindowFlags::NO_BACKGROUND|WindowFlags::NO_MOVE|WindowFlags::NO_NAV|WindowFlags::NO_SCROLL_WITH_MOUSE).build(|| {
        // 字体按字号固定，不随字表覆盖情况换（否则同一界面里黑体、宋体混排）：
        // 纯英文用黑体；22 像素及以上（标题、赐福名、按钮）用标题宋体；正文用默认宋体
        let font_for=|value:&str,px:f32|->(Option<hudhook::imgui::FontStackToken<'_>>,f32) {
            use crate::fancy_text as ft;
            if value.is_ascii() {if let Some(font)=ft::button_font() {return (Some(ui.push_font(font)),ft::SANS_PIXELS);}}
            if px>=22.0 {if let Some(font)=ft::title_font() {return (Some(ui.push_font(font)),32.0);}}
            (None,20.0)
        };
        let text=|x:f32,y:f32,value:&str,color:[f32;4],px:f32,align:f32| {
            if color[3]<=0.01 {return;}
            let (_font,base)=font_for(value,px);ui.set_window_font_scale(px/base*s);
            ui.set_cursor_screen_pos([ox+x*s-ui.calc_text_size(value)[0]*align,oy+y*s]);ui.text_colored(color,value);
        };
        let measure=|value:&str,px:f32|->f32 {let (_font,base)=font_for(value,px);ui.set_window_font_scale(px/base*s);ui.calc_text_size(value)[0]/s};
        let fitted=|value:&str,px:f32,width:f32|->f32 {let w=measure(value,px);if w>width {(px*width/w).max(px*0.6)}else{px}};
        let wrap=|value:&str,px:f32,width:f32|->Vec<String> {
            value.split('\n').flat_map(|line|{let lines=crate::fancy_text::wrap_lines(line,width,|piece|measure(piece,px));if lines.is_empty(){vec![String::new()]}else{lines}}).collect()
        };
        let micro=|x:f32,y:f32,value:&str,color:[f32;4],px:f32| {
            let gap=px*0.3;let width=value.chars().map(|c|measure(&c.to_string(),px)+gap).sum::<f32>()-gap;
            let mut cx=x-width*0.5;for c in value.chars() {let piece=c.to_string();text(cx,y,&piece,color,px,0.0);cx+=measure(&piece,px)+gap;}
        };
        // ── 先排版，算出卡片高度 ──
        let inner=CARD_W-PAD*2.0;
        let title=match &kind {Kind::Failure{..}=>crate::i18n::t("暂时无法开始联机旅程"),Kind::Trial{route,..}=>format!("{route} · {}",crate::i18n::t("挑战完成"))};
        let title_px=fitted(&title,40.0,inner);
        enum Block {Reason(Vec<String>),Boon{id:u32,name:String,info:Vec<String>},Line(String)}
        let mut blocks=Vec::new();let mut body=0.0f32;
        match &kind {
            Kind::Failure{message}=>{
                let lines=wrap(message,19.0,inner-48.0);body+=lines.len() as f32*30.0+40.0+18.0;blocks.push(Block::Reason(lines));
                body+=26.0;blocks.push(Block::Line(crate::i18n::t("详细原因已记录到 tower-preview.log")));
            },
            Kind::Trial{items,..}=>{
                body+=40.0;
                for &id in items.iter() {
                    let (name,info)=crate::reward_ui::talisman_description(id);
                    let info=if info.is_empty(){Vec::new()}else{wrap(&info,16.0,inner-92.0)};
                    body+=(36.0+info.len() as f32*24.0).max(68.0)+14.0;blocks.push(Block::Boon{id,name,info});
                }
                if items.is_empty() {let lines=wrap(&crate::i18n::t("已拥有全部可获得的护符赐福，本次没有新增赐福。"),19.0,inner-48.0);body+=lines.len() as f32*30.0+40.0+18.0;blocks.push(Block::Reason(lines));}
            },
        }
        let height=(196.0+body+118.0).min(860.0);
        let rise=(1.0-ease(t,0.0,0.4))*18.0;let a=ease(t,0.0,0.3);
        let (left,top)=(800.0-CARD_W*0.5,450.0-height*0.5+rise);let right=left+CARD_W;let bottom=top+height;
        // ── 卡片：金边面板、左侧强调竖条、顶部一道强调色光晕 ──
        {
            let draw=ui.get_window_draw_list();
            draw.add_rect(p(left+8.0,top+10.0),p(right+8.0,bottom+12.0),ImColor32::from_rgba(0,0,0,(120.0*a) as u8)).filled(true).build();
            crate::law_ui::panel(&draw,p(left,top),p(right,bottom),s);
            draw.add_rect_filled_multicolor(p(left,top),p(right,top+120.0),col(accent,0.16*a),col(accent,0.16*a),col(accent,0.0),col(accent,0.0));
            draw.add_rect(p(left,top),p(left+5.0,bottom),col(accent,0.95*a)).filled(true).build();
            // 徽记：失败是红圈里的感叹号，成功是金色菱形纹章
            let c=p(800.0,top+48.0);let r=19.0*s;
            draw.add_circle(c,r+6.0*s,col(accent,0.25*a)).filled(true).build();
            draw.add_circle(c,r,col(accent,a)).thickness(2.0*s).build();
            if failure {
                draw.add_rect([c[0]-1.8*s,c[1]-10.0*s],[c[0]+1.8*s,c[1]+3.0*s],col(accent,a)).filled(true).build();
                draw.add_circle([c[0],c[1]+8.0*s],2.2*s,col(accent,a)).filled(true).build();
            } else {
                let d=9.0*s;draw.add_polyline(vec![[c[0],c[1]-d],[c[0]+d,c[1]],[c[0],c[1]+d],[c[0]-d,c[1]]],col(accent,a)).filled(true).build();
                let d=4.0*s;draw.add_polyline(vec![[c[0],c[1]-d],[c[0]+d,c[1]],[c[0],c[1]+d],[c[0]-d,c[1]]],col(INK,a)).filled(true).build();
            }
            crate::law_ui::ornament(&draw,p(800.0,top+164.0),150.0*s*ease(t,0.1,0.4),s);
        }
        micro(800.0,top+82.0,if failure{"CO-OP"}else{"TRIAL CLEARED"},fade(accent,a),12.0);
        text(800.0,top+102.0,&title,fade(WHITE,a),title_px,0.5);
        // ── 正文 ──
        let mut y=top+196.0;
        if !failure {text(800.0,y-8.0,&crate::i18n::t("获得护符赐福 · 本局持续生效，不占护符栏位"),fade(MUTED,a),16.0,0.5);y+=40.0;}
        let mut row=0usize;
        for block in &blocks {
            match block {
                Block::Reason(lines)=>{
                    let h=lines.len() as f32*30.0+40.0;
                    {let draw=ui.get_window_draw_list();
                        draw.add_rect(p(left+PAD,y),p(right-PAD,y+h),ImColor32::from_rgba(0,0,0,(110.0*a) as u8)).filled(true).build();
                        draw.add_rect(p(left+PAD,y),p(left+PAD+3.0,y+h),col(accent,0.8*a)).filled(true).build();
                        draw.add_rect(p(left+PAD,y),p(right-PAD,y+h),col(WHITE,0.08*a)).thickness(s).build();}
                    for (i,line) in lines.iter().enumerate() {text(left+PAD+24.0,y+20.0+i as f32*30.0,line,fade(WHITE,a),19.0,0.0);}
                    y+=h+18.0;
                },
                Block::Line(value)=>{text(800.0,y,value,fade(MUTED,a),15.0,0.5);y+=26.0;},
                Block::Boon{id,name,info}=>{
                    let k=ease(t,0.25+row as f64*0.12,0.35);row+=1;
                    let h=(36.0+info.len() as f32*24.0).max(68.0);let dx=(1.0-k)*28.0;
                    let x0=left+PAD+dx;
                    {let draw=ui.get_window_draw_list();
                        draw.add_rect(p(x0,y),p(right-PAD+dx,y+h),ImColor32::from_rgba(255,255,255,(10.0*k) as u8)).filled(true).build();
                        draw.add_rect(p(x0,y),p(x0+64.0,y+64.0),ImColor32::from_rgba(6,6,6,(220.0*k) as u8)).filled(true).build();
                        draw.add_rect(p(x0,y),p(x0+64.0,y+64.0),col(AMBER,0.55*k)).thickness(s).build();
                        match crate::reward_ui::talisman_icon(*id).and_then(&mut *icon) {
                            Some(texture)=>draw.add_image(texture,p(x0+4.0,y+4.0),p(x0+60.0,y+60.0)).col(col(WHITE,k)).build(),
                            None=>{let c=p(x0+32.0,y+32.0);let d=10.0*s;draw.add_polyline(vec![[c[0],c[1]-d],[c[0]+d,c[1]],[c[0],c[1]+d],[c[0]-d,c[1]]],col(AMBER,k)).filled(true).build();},
                        }}
                    text(x0+84.0,y+4.0,name,fade(AMBER,k),22.0,0.0);
                    for (i,line) in info.iter().enumerate() {text(x0+84.0,y+36.0+i as f32*24.0,line,fade(MUTED,k),16.0,0.0);}
                    y+=h+14.0;
                },
            }
        }
        // ── 确认按钮：琥珀色实心，悬停提亮，右下切角 ──
        let foot=ease(t,0.3,0.3);
        let label=&crate::i18n::t(if failure{"确定"}else{"继续选路"});
        let bw=(measure(label,24.0)+130.0).max(240.0);let (bx,by)=(800.0-bw*0.5,bottom-90.0);
        ui.set_cursor_screen_pos(p(bx,by));ui.set_window_font_scale(s);
        if ui.invisible_button("##notice-ok",[bw*s,56.0*s])&&ready {close=true;}
        let hover=ui.is_item_hovered();
        {let draw=ui.get_window_draw_list();
            draw.add_rect(p(bx,by),p(bx+bw,by+56.0),col(AMBER,foot)).filled(true).build();
            if hover {draw.add_rect(p(bx,by),p(bx+bw,by+56.0),col([1.0,0.86,0.5,1.0],0.55*foot)).filled(true).build();}
            draw.add_triangle(p(bx+bw,by+56.0),p(bx+bw-14.0,by+56.0),p(bx+bw,by+42.0),ImColor32::from_rgba(13,13,12,255)).filled(true).build();}
        let badge=crate::pad::badge(ui,ox+(bx+24.0)*s,oy+(by+28.0)*s,26.0*s,crate::pad::CONFIRM,crate::pad::Tone::Dark,foot)/s;
        let tx=bx+24.0+badge+12.0;
        text(tx+(bx+bw-24.0-tx-measure(label,24.0))*0.5,by+13.0,label,fade(INK,foot),24.0,0.0);
        ui.set_window_font_scale(1.0);
    });
    if close {crate::sfx::play("confirm");}
    close
}
