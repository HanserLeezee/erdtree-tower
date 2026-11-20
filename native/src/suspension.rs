use std::{path::Path,time::Instant,sync::{Mutex,atomic::{AtomicBool,AtomicU8,Ordering}}};
use erdtree_tower::{Run,suspension::{Point,Snapshot}};
pub static REQUEST:AtomicBool=AtomicBool::new(false);
pub static RESTART_CONTEXT:std::sync::atomic::AtomicU64=std::sync::atomic::AtomicU64::new(0);
pub static RESTART_REQUEST:std::sync::atomic::AtomicU64=std::sync::atomic::AtomicU64::new(0);
pub static ROLE:AtomicU8=AtomicU8::new(0);
static NOTICE:Mutex<String>=Mutex::new(String::new());
pub fn notice(text:impl Into<String>){if let Ok(mut s)=NOTICE.lock(){*s=text.into();}}
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
    save_at(&path,owner,run)
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
    }Ok(())
}
pub fn backup_restart(owner:(u32,i32))->Result<(),String> {
    let files=["paused","json","reconnect"].map(|ext|super::hub::path(owner,ext));
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
    let eligible=view.and_then(|v|v.run.as_ref()).is_some_and(|r|!matches!(r.phase,erdtree_tower::Phase::Dead|erdtree_tower::Phase::Won));
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
}
