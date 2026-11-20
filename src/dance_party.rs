use serde::{Deserialize,Serialize};

#[derive(Clone,Copy,Debug,Default,PartialEq,Eq,Serialize,Deserialize)]
pub struct Report {pub choice:Option<u8>,pub fingerprint:Option<u64>,pub finished:bool,pub failed:bool}
#[derive(Clone,Debug,PartialEq,Eq,Serialize,Deserialize)]
pub enum Command {Prepare{song:u8,players:Vec<u64>},Start,Complete,Cancel}
#[derive(Default)]
pub struct Round {epoch:u64,phase:u8,players:Vec<u64>,since:u64}
impl Round {
    pub fn poll(&mut self,epoch:u64,now:u64,host:u64,rows:&[(u64,Option<Report>)],eligible:bool)->Option<Command>{
        if self.epoch!=epoch{*self=Self{epoch,..Self::default()};}
        let players:Vec<_>=rows.iter().map(|r|r.0).collect();
        if matches!(self.phase,1|2)&&(!eligible||players!=self.players||rows.iter().any(|r|r.1.is_some_and(|r|r.failed))){self.phase=3;return Some(Command::Cancel);}
        if self.phase==0&&eligible {
            let song=rows.iter().find(|r|r.0==host)?.1?.choice?;
            if song>=2{return None;}
            self.phase=1;self.players=players.clone();self.since=now;
            return Some(Command::Prepare{song,players});
        }
        if self.phase==1 {
            if now.saturating_sub(self.since)>30_000{self.phase=3;return Some(Command::Cancel);}
            let fingerprints:Option<Vec<_>>=rows.iter().map(|r|r.1?.fingerprint).collect();
            if let Some(f)=fingerprints.filter(|f|!f.is_empty()){
                self.phase=if f.iter().all(|n|*n!=0&&*n==f[0]){2}else{3};
                return Some(if self.phase==2{Command::Start}else{Command::Cancel});
            }
        }
        if self.phase==2&&rows.iter().all(|r|r.1.is_some_and(|r|r.finished)){self.phase=3;return Some(Command::Complete);}
        None
    }
}

#[cfg(test)]mod tests {
    use super::*;
    #[test]fn waits_for_all_audio_and_only_host_selects(){
        let mut round=Round::default();let mut rows=vec![(1,Some(Report::default())),(2,Some(Report{choice:Some(1),..Report::default()}))];
        assert_eq!(round.poll(7,0,1,&rows,true),None);
        rows[0].1.as_mut().unwrap().choice=Some(0);
        assert_eq!(round.poll(7,1,1,&rows,true),Some(Command::Prepare{song:0,players:vec![1,2]}));
        rows[0].1.as_mut().unwrap().fingerprint=Some(123);
        assert_eq!(round.poll(7,2,1,&rows,true),None);
        rows[1].1.as_mut().unwrap().fingerprint=Some(123);
        assert_eq!(round.poll(7,3,1,&rows,true),Some(Command::Start));
        assert_eq!(round.poll(7,4,1,&rows,true),None);
    }
    #[test]fn mismatched_audio_disconnect_and_timeout_cancel(){
        for case in 0..3{
            let mut round=Round::default();let mut rows=vec![(1,Some(Report{choice:Some(0),fingerprint:Some(1),..Report::default()})),(2,Some(Report::default()))];
            assert!(matches!(round.poll(1,0,1,&rows,true),Some(Command::Prepare{..})));
            if case==0{rows[1].1.as_mut().unwrap().fingerprint=Some(2);}else if case==1{rows.pop();}
            assert_eq!(round.poll(1,if case==2{30_001}else{1},1,&rows,true),Some(Command::Cancel));
            assert_eq!(round.poll(1,40_000,1,&rows,true),None);
        }
    }
}
