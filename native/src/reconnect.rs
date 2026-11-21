use std::time::{Duration,Instant};
use erdtree_tower::{Run,party_roster::Roster,party_handshake::{Payload,Handshake},party_reconnect::{Ticket,Message,Checkpoint},party_compat::Compatibility};

pub fn save(owner:(u32,i32),link:&super::party::Link,run:&Run)->Result<(),String> {
    if link.host()||link.roster().is_none(){return Ok(());}
    let ticket=Ticket{version:erdtree_tower::party_handshake::VERSION,owner,player:link.config.player,host:link.config.host,session:link.config.session,seed:run.initial_seed.unwrap_or(run.seed),generation:link.reconnect_generation()};
    let path=super::hub::path(owner,"reconnect");
    let text=ticket.encode()?;
    static LAST:std::sync::Mutex<Option<(std::path::PathBuf,String)>>=std::sync::Mutex::new(None);
    let mut last=LAST.lock().map_err(|_|"重连存档锁不可用")?;
    if last.as_ref()==Some(&(path.clone(),text.clone()))&&path.exists(){return Ok(());}
    atomic_write(&path,&text)?;*last=Some((path,text));Ok(())
}
fn atomic_write(path:&std::path::Path,text:&str)->Result<(),String> {
    use std::io::Write;
    let tmp=path.with_extension("reconnect-tmp");
    let mut file=std::fs::File::create(&tmp).map_err(|e|e.to_string())?;
    file.write_all(text.as_bytes()).and_then(|_|file.sync_all()).map_err(|e|e.to_string())?;drop(file);
    std::fs::rename(tmp,path).map_err(|e|e.to_string())
}
pub fn clear(owner:(u32,i32))->Result<(),String> {
    match std::fs::remove_file(super::hub::path(owner,"reconnect")) {Ok(())=>Ok(()),Err(e) if e.kind()==std::io::ErrorKind::NotFound=>Ok(()),Err(e)=>Err(e.to_string())}
}
pub fn load(owner:(u32,i32),roster:&Roster)->Result<Option<(Ticket,Run)>,String> {
    let path=super::hub::path(owner,"reconnect");
    if !path.exists()||roster.local==roster.host||super::hub::path(owner,"paused").exists(){return Ok(None);}
    let text=std::fs::read_to_string(path).map_err(|e|e.to_string())?;
    let ticket=Ticket::decode(&text)?;
    let path=super::hub::path(owner,"json");if !path.exists(){return Ok(None);}
    let run=Run::decode(&std::fs::read_to_string(path).map_err(|e|e.to_string())?)?;
    if !ticket.matches(owner,roster.local,roster.host,&run){return Ok(None);}
    if super::hub::path(owner,"party-award").exists(){return Err("上次闪退发生在领奖事务中，需先核对领奖记录；已停止自动重连，避免重复发奖".into());}
    Ok(Some((ticket,run)))
}

pub struct Pending {
    pub roster:Roster,pub owner:(u32,i32),ticket:Ticket,run:Run,profile:Compatibility,
    transport:super::steam_messages::Transport,nonce:u64,sent:Instant,
    offer:Option<(u64,Run,bool)>,members:Vec<u64>,packets:Vec<(u64,Payload)>,started:Instant,ready:bool,
    block:Option<i32>,landed:Option<Instant>,
}
pub enum Status {Waiting,Ready,FreshLobby}
impl Pending {
    pub fn new(roster:Roster,ticket:Ticket,run:Run,profile:Compatibility)->Result<Self,String> {
        let transport=super::steam_messages::Transport::load(roster.clone())?;
        Ok(Self{roster,owner:ticket.owner,ticket,run,profile,transport,nonce:super::party::random_entropy()?.max(1),sent:Instant::now()-Duration::from_secs(1),offer:None,members:vec![],packets:vec![],started:Instant::now(),ready:false,block:None,landed:None})
    }
    pub fn poll(&mut self)->Result<Status,String> {
        if self.started.elapsed()>Duration::from_secs(180){return Err("重连等待超过 3 分钟，请退出房间后重新加入；本机进度已保留".into());}
        if self.sent.elapsed()>=Duration::from_secs(1) {
            self.sent=Instant::now();
            let message=if let Some((epoch,_,_))=&self.offer {Message::Ack{nonce:self.nonce,epoch:*epoch}}
                else{Message::Request{nonce:self.nonce,seed:self.ticket.seed,generation:self.ticket.generation,profile:self.profile.clone(),dlc:super::options::local_dlc()}};
            self.transport.send(self.roster.host,&Payload::Reconnect{session:self.ticket.session,message}.encode()?)?;
        }
        for (peer,bytes) in self.transport.receive()? {
            if peer!=self.roster.host{continue;}
            match Payload::decode(&bytes)? {
                Payload::Reconnect{message:Message::Reject{nonce,reason},..} if nonce==self.nonce=>return Err(reason),
                Payload::Reconnect{session,message} if session==self.ticket.session=>match message {
                    Message::FreshLobby{nonce} if nonce==self.nonce&&self.offer.is_none()=>return Ok(Status::FreshLobby),
                    Message::Offer{nonce,checkpoint,dlc,members,block} if nonce==self.nonce&&self.offer.is_none()=>{
                        if members.len()>erdtree_tower::party_roster::MAX_PLAYERS||members.contains(&0)||!members.contains(&self.roster.local)||!members.contains(&self.roster.host)
                            ||members.windows(2).any(|p|p[0]>=p[1]){return Err("原队伍名单无效".into());}
                        let run=checkpoint.restore(&self.run)?;
                        atomic_write(&super::hub::path(self.owner,"json"),&run.encode()?)?;
                        if block<0{return Err("房主重连场地尚未就绪".into());}
                        self.block=Some(block);self.members=members;self.offer=Some((checkpoint.epoch,run,dlc));self.sent=Instant::now()-Duration::from_secs(1);
                    },
                    Message::Ready{nonce,epoch} if nonce==self.nonce&&self.offer.as_ref().is_some_and(|o|o.0==epoch)=>self.ready=true,
                    Message::Reject{nonce,reason} if nonce==self.nonce=>return Err(reason),
                    _=>{},
                },
                packet@Payload::Order{session,..} if session==self.ticket.session&&self.offer.is_some()=>{
                    if self.packets.len()>=64{return Err("重连消息过多，请重新加入房间".into());}
                    self.packets.push((peer,packet));
                },
                _=>{},
            }
        }
        Ok(if self.ready {Status::Ready}else{Status::Waiting})
    }
    pub fn landed(&mut self,block:i32,grounded:bool)->bool {
        if !self.ready||!grounded||self.block!=Some(block){self.landed=None;return false;}
        self.landed.get_or_insert_with(Instant::now).elapsed()>=Duration::from_secs(2)
    }
    pub fn finish(self)->Result<(super::party::Link,Run),String> {
        let (epoch,run,dlc)=self.offer.ok_or("尚未收到重连进度")?;
        let handshake=Handshake::restored(self.roster.clone(),self.ticket.session,self.ticket.seed,dlc,self.profile)?;
        let pending=super::steam_messages::Pending::restored(self.roster,self.transport,handshake,self.packets);
        let mut link=super::party::Link::from_steam(pending,self.ticket.session,self.ticket.seed)?;
        link.epoch=epoch;link.gate.epoch=epoch;
        link.restore_members(self.members,self.ticket.generation);
        Ok((link,run))
    }
}

pub struct Request {pub peer:u64,pub nonce:u64,pub offer:Option<Checkpoint>,pub block:i32,pub admitted:bool,pub touched:Instant}
#[derive(Default)]
pub struct Host {pub known:Vec<u64>,pub revoked:Vec<u64>,pub requests:Vec<Request>,pub generation:u64}
impl Host {
    pub fn accepts(&self,peer:u64,nonce:u64,session:u64,seed:u64,generation:u64,config:&erdtree_tower::party_game::Config)->bool {
        session==config.session&&seed==config.seed&&generation==self.generation&&nonce!=0
            &&peer!=config.host&&self.known.contains(&peer)&&!self.revoked.contains(&peer)
    }
    pub fn permits(&self,old:&Roster,next:&Roster)->bool {
        next.local==old.local&&next.host==old.host&&next.members.contains(&next.host)&&next.members.contains(&next.local)
            &&next.members.iter().all(|id|old.members.contains(id)||self.known.contains(id)&&!self.revoked.contains(id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reconnect_roster_accepts_only_original_members_and_same_host() {
        let old=Roster{local:1,host:1,members:vec![1]};
        let mut host=Host{known:vec![1,2,3],..Default::default()};
        let returning=Roster{members:vec![1,2],..old.clone()};
        assert!(host.permits(&old,&returning));
        assert!(host.permits(&old,&Roster{members:vec![1,2,3],..old.clone()}));
        assert!(!host.permits(&old,&Roster{members:vec![1,4],..old.clone()}));
        assert!(!host.permits(&old,&Roster{host:2,..returning.clone()}));
        assert!(!host.permits(&old,&Roster{local:2,..returning.clone()}));
        host.revoked.push(2);assert!(!host.permits(&old,&returning));
    }
    #[test]
    fn reconnect_atomic_write_replaces_complete_checkpoint() {
        let path=std::env::temp_dir().join(format!("tower-reconnect-{}-{}.json",std::process::id(),super::super::party::random_entropy().unwrap()));
        atomic_write(&path,"first").unwrap();atomic_write(&path,"second").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(),"second");
        assert!(!path.with_extension("reconnect-tmp").exists());std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn reconnect_old_run_and_stranger_cannot_use_same_seed() {
        use erdtree_tower::party_game::Config;
        let config=Config{session:99,seed:42,player:1,host:1,address:"127.0.0.1:1".parse().unwrap(),token:String::new(),peers:vec![]};
        let mut host=Host{known:vec![1,2],generation:8,..Default::default()};
        assert!(host.accepts(2,7,99,42,8,&config));
        for (peer,nonce,session,seed,generation) in [(3,7,99,42,8),(2,0,99,42,8),(2,7,100,42,8),(2,7,99,43,8),(2,7,99,42,0),(1,7,99,42,8)] {
            assert!(!host.accepts(peer,nonce,session,seed,generation,&config));
        }
        host.revoked.push(2);assert!(!host.accepts(2,7,99,42,8,&config));
    }
}
