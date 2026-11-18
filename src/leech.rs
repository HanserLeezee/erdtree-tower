use serde::{Deserialize,Serialize};

#[derive(Clone,Debug,PartialEq,Eq,Serialize,Deserialize)]
pub struct Hits {pub key:(u64,u8,usize),pub counts:Vec<(u32,u64)>}
#[derive(Default)]
pub struct Deltas {seen:Vec<(u64,u32,u64)>}
impl Deltas {
    pub fn take(&mut self,key:(u64,u8,usize),player:u64,hits:&Hits)->Vec<(u32,u64)> {
        if hits.key!=key {return vec![];}
        let mut fresh=Vec::new();
        for &(entity,count) in hits.counts.iter().take(32) {
            let i=self.seen.iter().position(|&(p,e,_)|p==player&&e==entity).unwrap_or_else(||{
                self.seen.push((player,entity,0));self.seen.len()-1
            });
            let previous=&mut self.seen[i].2;let delta=count.saturating_sub(*previous);*previous=(*previous).max(count);
            if delta>0 {fresh.push((entity,delta));}
        }
        fresh
    }
}
