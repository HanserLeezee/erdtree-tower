use std::{path::Path,time::Instant,sync::{Mutex,atomic::{AtomicBool,AtomicU8,Ordering}}};
use erdtree_tower::{Run,suspension::{Point,Snapshot}};
pub static REQUEST:AtomicBool=AtomicBool::new(false);
pub static RESTART_CONTEXT:std::sync::atomic::AtomicU64=std::sync::atomic::AtomicU64::new(0);
pub static RESTART_REQUEST:std::sync::atomic::AtomicU64=std::sync::atomic::AtomicU64::new(0);
pub static ROLE:AtomicU8=AtomicU8::new(0);
static NOTICE:Mutex<(String,Option<Instant>)>=Mutex::new((String::new(),None));
pub fn notice(text:impl Into<String>) {
    let next=text.into();
    if let Ok(mut slot)=NOTICE.lock() {
        if slot.0!=next {
            slot.1=(!next.trim().is_empty()).then(Instant::now);
            slot.0=next;
        }
    }
}
fn notice_persistent(text:&str)->bool {
    if text=="已取消等待暂退。"{return false;}
    text.contains("等待")||text.contains("正在")||text.contains("分离迷雾")
        ||text.contains("已返回大赐福")||text.contains("请勿强退")||text.contains("请勿强制关闭")
}
fn notice_alpha(text:&str,elapsed_secs:f32)->f32 {
    if text.trim().is_empty()||!elapsed_secs.is_finite()||elapsed_secs<0.0 {return 0.0;}
    if notice_persistent(text) {
        (elapsed_secs/0.18).min(1.0)
    } else if elapsed_secs<0.18 {
        elapsed_secs/0.18
    } else if elapsed_secs<5.2 {
        1.0
    } else if elapsed_secs<5.9 {
        (5.9-elapsed_secs)/0.7
    } else {
        0.0
    }
}
pub fn active_notice()->Option<(String,f32)> {
    let (text,since)=NOTICE.lock().ok().map(|s|s.clone())?;
    let alpha=notice_alpha(&text,since?.elapsed().as_secs_f32());
    (alpha>0.01).then_some((text,alpha))
}
#[derive(Default)]
pub struct LinkState {
    pub queued:bool,pub ticket:Option<u64>,pub saved:bool,pub committed:bool,pub started:Option<Instant>,
    pub leaving:bool,pub leave_ready:bool,pub departed:Vec<u64>,
    pub resume:Option<Point>,pub resume_ready:bool,
}
#[derive(Default)]
pub struct Flow {pub requested:bool,pub prepared:Option<Run>,pub returning:bool,pub leave:bool,pub committed:Option<Instant>}
fn atomic_write(path:&Path,text:&str)->Result<(),String> {
    use std::io::Write;
    let tmp=path.with_extension("pause-tmp");
    let mut file=std::fs::File::create(&tmp).map_err(|e|e.to_string())?;
    file.write_all(text.as_bytes()).and_then(|_|file.sync_all()).map_err(|e|e.to_string())?;drop(file);
    std::fs::rename(tmp,path).map_err(|e|e.to_string())
}
pub fn save(owner:(u32,i32),run:&Run)->Result<(),String> {
    let path=super::hub::path(owner,"paused");
    save_at(&path,owner,run)?;
    let _=clear_auto(owner);
    Ok(())
}
type AutoKey=((u32,i32),u64,u8,usize,u32,String,bool);
static LAST_AUTO:Mutex<Option<AutoKey>>=Mutex::new(None);
/// 联机每回到路线图时自动写一份防房主闪退检查点（独立于 F6 的 paused 文件，不影响客机热重连）。
pub fn save_auto_if_changed(owner:(u32,i32),run:&Run)->Result<(),String> {
    let path=super::hub::path(owner,"autosave");
    let key=(owner,run.seed,run.act,run.visited.len(),run.completed_nodes,run.law.code(),run.endless);
    if let Ok(last)=LAST_AUTO.lock() && last.as_ref()==Some(&key) && path.exists() {return Ok(());}
    save_auto_at(&path,owner,run)?;
    if let Ok(mut last)=LAST_AUTO.lock(){*last=Some(key);}
    super::log(format!("PARTY_AUTOSAVE saved owner={owner:?} act={} visited={} completed={}",run.act,run.visited.len(),run.completed_nodes));
    Ok(())
}
fn save_auto_at(path:&Path,owner:(u32,i32),run:&Run)->Result<(),String> {
    let text=Snapshot::encode(owner,run)?;
    atomic_write(path,&text)?;
    Snapshot::decode(&std::fs::read_to_string(path).map_err(|e|e.to_string())?,owner)?;Ok(())
}
pub fn load_auto(owner:(u32,i32))->Result<Option<Run>,String> {
    let path=super::hub::path(owner,"autosave");
    if !path.exists(){return Ok(None);}
    let text=std::fs::read_to_string(&path).map_err(|e|e.to_string())?;
    if text.trim().is_empty(){return Ok(None);}
    match Snapshot::decode(&text,owner) {
        Ok(run)=>Ok(Some(run)),
        Err(error)=>{
            super::log(format!("PARTY_AUTOSAVE corrupted, ignoring: {error}"));
            let _=std::fs::remove_file(&path);
            Ok(None)
        }
    }
}
pub fn clear_auto(owner:(u32,i32))->Result<(),String> {
    if let Ok(mut last)=LAST_AUTO.lock() && last.as_ref().is_some_and(|k|k.0==owner) {*last=None;}
    let path=super::hub::path(owner,"autosave");
    match std::fs::remove_file(path) {Ok(())=>Ok(()),Err(e) if e.kind()==std::io::ErrorKind::NotFound=>Ok(()),Err(e)=>Err(e.to_string())}
}
fn save_at(path:&Path,owner:(u32,i32),run:&Run)->Result<(),String> {
    let text=Snapshot::encode(owner,run)?;
    if path.exists(){
        let old=std::fs::read_to_string(path).map_err(|e|e.to_string())?;
        if !old.trim().is_empty(){Snapshot::decode(&old,owner)?;std::fs::copy(path,path.with_extension("paused-backup")).map_err(|e|e.to_string())?;}
    }
    atomic_write(&path,&text)?;
    Snapshot::decode(&std::fs::read_to_string(path).map_err(|e|e.to_string())?,owner)?;Ok(())
}
pub fn load(owner:(u32,i32))->Result<Option<Run>,String> {
    let path=super::hub::path(owner,"paused");
    if !path.exists(){return Ok(None);}
    let text=std::fs::read_to_string(&path).map_err(|e|e.to_string())?;
    if text.trim().is_empty(){return Ok(None);}
    Snapshot::decode(&text,owner).map(Some)
}
pub fn clear(owner:(u32,i32))->Result<(),String> {
    let path=super::hub::path(owner,"paused");
    if path.exists(){
        if std::fs::read_to_string(&path).ok().is_some_and(|text|Snapshot::decode(&text,owner).is_ok()) {
            std::fs::copy(&path,path.with_extension("paused-backup")).map_err(|e|e.to_string())?;
        }
        std::fs::remove_file(path).map_err(|e|e.to_string())?;
    }
    clear_auto(owner)?;
    Ok(())
}
pub fn backup_restart(owner:(u32,i32))->Result<(),String> {
    let files=["paused","autosave","json","reconnect"].map(|ext|super::hub::path(owner,ext));
    if !files.iter().any(|p|p.exists()){return Ok(());}
    let stamp=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|e|e.to_string())?.as_nanos();
    let folder=files[0].parent().ok_or("旅程目录无效")?.join("restart-backups").join(format!("{}-{}-{stamp}",owner.0,owner.1));
    backup_files(&files,&folder)?;
    super::log(format!("PARTY_RESTART backup={}",folder.display()));Ok(())
}
fn backup_files(files:&[std::path::PathBuf],folder:&Path)->Result<(),String> {
    std::fs::create_dir_all(folder).map_err(|e|e.to_string())?;
    for file in files {if file.exists(){std::fs::copy(file,folder.join(file.file_name().ok_or("旅程文件名无效")?)).map_err(|e|e.to_string())?;}}
    Ok(())
}
pub fn draw(ui:&hudhook::imgui::Ui,view:Option<&super::runtime::View>) {
    use hudhook::imgui::Key;
    let eligible=!ui.io().want_text_input
        &&view.and_then(|v|v.run.as_ref()).is_some_and(|r|!matches!(r.phase,erdtree_tower::Phase::Dead|erdtree_tower::Phase::Won));
    if eligible&&ui.is_key_pressed_no_repeat(Key::F6){REQUEST.store(true,Ordering::Release);}
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn suspension_disk_roundtrip_preserves_backup_and_rejects_corruption() {
        let dir=std::env::temp_dir().join(format!("tower-pause-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir(&dir).unwrap();let path=dir.join("session.paused");let owner=(7,0);
        let mut run=Run::new(42,erdtree_tower::Class::Samurai);run.gold=123;
        save_at(&path,owner,&run).unwrap();run.gold=456;save_at(&path,owner,&run).unwrap();
        assert_eq!(Snapshot::decode(&std::fs::read_to_string(&path).unwrap(),owner).unwrap().gold,456);
        let backup=path.with_extension("paused-backup");assert_eq!(Snapshot::decode(&std::fs::read_to_string(&backup).unwrap(),owner).unwrap().gold,123);
        let restart=dir.join("restart");backup_files(&[path.clone()],&restart).unwrap();
        let preserved=restart.join("session.paused");assert_eq!(Snapshot::decode(&std::fs::read_to_string(&preserved).unwrap(),owner).unwrap().gold,456);
        std::fs::write(&path,"broken").unwrap();assert!(save_at(&path,owner,&run).is_err());
        assert_eq!(Snapshot::decode(&std::fs::read_to_string(&backup).unwrap(),owner).unwrap().gold,123);
        std::fs::remove_file(preserved).unwrap();std::fs::remove_dir(restart).unwrap();
        std::fs::remove_file(path).unwrap();std::fs::remove_file(backup).unwrap();std::fs::remove_dir(dir).unwrap();
    }
    #[test]
    fn suspension_notice_stays_visible_while_waiting_and_fades_when_transient() {
        assert_eq!(notice_alpha("",0.5),0.0);
        assert!(notice_persistent("等待当前房间结束并完成领奖，随后保存暂退。"));
        assert!(notice_persistent("离队已确认。请使用“分离迷雾”断开无缝房间，随后自动返回大赐福。"));
        assert!(notice_persistent("已返回大赐福。请从游戏菜单正常退出；重进后与小红帽交互继续旅程。"));
        assert_eq!(notice_alpha("等待当前房间结束并完成领奖，随后保存暂退。",30.0),1.0);
        assert!(!notice_persistent("已取消等待暂退。"));
        assert_eq!(notice_alpha("已取消等待暂退。",2.0),1.0);
        assert_eq!(notice_alpha("已取消等待暂退。",6.0),0.0);
    }
    #[test]
    fn auto_checkpoint_roundtrips_without_touching_explicit_pause_backup() {
        let dir=std::env::temp_dir().join(format!("tower-auto-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir(&dir).unwrap();let path=dir.join("session.autosave");let owner=(9,1);
        let mut run=Run::new(77,erdtree_tower::Class::Samurai);run.gold=321;
        save_auto_at(&path,owner,&run).unwrap();
        let restored=Snapshot::decode(&std::fs::read_to_string(&path).unwrap(),owner).unwrap();
        assert_eq!(restored.gold,321);assert_eq!(restored.seed,77);
        assert!(!path.with_extension("paused-backup").exists());
        std::fs::remove_file(path).unwrap();std::fs::remove_dir(dir).unwrap();
    }
}
