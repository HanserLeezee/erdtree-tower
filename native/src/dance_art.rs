// 舞会界面贴图（粉色梦幻风，GPT Image 生成后裁切缩放，scene-assets/dance-art/*.rgba）：玻璃条、名牌、亮 / 暗宝石箭头（四个方向）、
// 爱心判定点、星星光标、五个判定字。缺任何一张时 ready() 为 false，跳舞画面退回纯图形绘制。
use std::sync::atomic::{AtomicUsize,Ordering};
use hudhook::{RenderContext,imgui::TextureId};

pub const ASSETS:[(&str,u32,u32);17]=[
    ("bar",1024,262),("plate",768,215),
    ("arrow_lit_up",256,256),("arrow_lit_down",256,256),("arrow_lit_left",256,256),("arrow_lit_right",256,256),
    ("arrow_dim_up",256,256),("arrow_dim_down",256,256),("arrow_dim_left",256,256),("arrow_dim_right",256,256),
    ("target",256,256),("cursor",192,192),
    ("judge_perfect",512,256),("judge_great",512,256),("judge_cool",512,256),("judge_bad",512,256),("judge_miss",512,256),
];
static TEXTURES:[AtomicUsize;17]=[const {AtomicUsize::new(usize::MAX)};17];
pub fn initialize(renderer:&mut dyn RenderContext) {
    for (index,(name,w,h)) in ASSETS.into_iter().enumerate() {
        TEXTURES[index].store(usize::MAX,Ordering::Relaxed);
        let path=crate::LOG_PATH.with_file_name("scene-assets").join("dance-art").join(format!("{name}.rgba"));
        match std::fs::read(&path) {
            Ok(data) if data.len()==w as usize*h as usize*4=>match renderer.load_texture(&data,w,h) {
                Ok(id)=>TEXTURES[index].store(id.id(),Ordering::Relaxed),
                Err(e)=>crate::log(format!("Dance texture upload failed: {name}: {e:?}")),
            },
            _=>crate::log(format!("Dance texture missing or invalid: {name}")),
        }
    }
}
pub fn ready()->bool {TEXTURES.iter().all(|t|t.load(Ordering::Relaxed)!=usize::MAX)}
pub fn texture(name:&str)->Option<TextureId> {
    let index=ASSETS.iter().position(|(n,_,_)|*n==name)?;
    Some(TEXTURES[index].load(Ordering::Relaxed)).filter(|&id|id!=usize::MAX).map(TextureId::new)
}
/// 贴图宽高比（高 / 宽）
pub fn aspect(name:&str)->f32 {ASSETS.iter().find(|(n,_,_)|*n==name).map_or(1.0,|(_,w,h)|*h as f32/ *w as f32)}
