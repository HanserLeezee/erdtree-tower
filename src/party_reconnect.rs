use serde::{Deserialize,Serialize};
use crate::{Run,Phase,RuleResult,party_compat::Compatibility};

#[derive(Clone,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ticket {
    pub version:u32,pub owner:(u32,i32),pub player:u64,pub host:u64,pub session:u64,pub seed:u64,pub generation:u64,
}
impl Ticket {
    pub fn encode(&self)->RuleResult<String> {serde_json::to_string(self).map_err(|e|e.to_string())}
    pub fn decode(text:&str)->RuleResult<Self> {
        if text.len()>4096{return Err("重连凭据过大".into());}
        serde_json::from_str(text).map_err(|_|"重连凭据损坏；原旅程存档仍保留".into())
    }
    pub fn matches(&self,owner:(u32,i32),player:u64,host:u64,run:&Run)->bool {
        self.version==crate::party_handshake::VERSION&&self.owner==owner&&owner.1>=0
            &&self.player==player&&self.host==host&&player!=host&&player!=0&&host!=0&&self.session!=0
            &&self.seed==run.initial_seed.unwrap_or(run.seed)&&!matches!(run.phase,Phase::Dead|Phase::Won)
    }
}

#[derive(Clone,Debug,Serialize,Deserialize)]
pub enum Message {
    Request{nonce:u64,seed:u64,generation:u64,profile:Compatibility,dlc:bool},
    Offer{nonce:u64,checkpoint:Checkpoint,dlc:bool,members:Vec<u64>,block:i32},
    Ack{nonce:u64,epoch:u64},
    Ready{nonce:u64,epoch:u64},
    FreshLobby{nonce:u64},
    Reject{nonce:u64,reason:String},
}

#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct Checkpoint {
    pub epoch:u64,
    seed:u64,initial:u64,act:u8,schema:u32,law:crate::law::Law,
    visited:Vec<usize>,completed_nodes:u32,completed_battles:u32,reached_layer:usize,
}
impl Checkpoint {
    pub fn of(run:&Run,epoch:u64)->RuleResult<Self> {
        run.validate()?;
        if run.phase!=Phase::Map||run.contract_pending()||run.retry_node.is_some()||run.debug {
            return Err("等待房主完成当前房间并回到路线图；调试旅程不支持重连".into());
        }
        Ok(Self{epoch,seed:run.seed,initial:run.initial_seed.unwrap_or(run.seed),act:run.act,schema:run.schema,
            law:run.law,visited:run.visited.clone(),completed_nodes:run.completed_nodes,
            completed_battles:run.completed_battles,reached_layer:run.reached_layer})
    }
    pub fn restore(&self,local:&Run)->RuleResult<Run> {
        local.validate()?;
        if self.initial!=local.initial_seed.unwrap_or(local.seed)||self.law!=local.law||self.act<local.act||self.act>3
            ||self.completed_nodes<local.completed_nodes||self.completed_battles<local.completed_battles
            ||self.act==local.act&&(!self.visited.starts_with(&local.visited)||self.seed!=local.seed)
            ||crate::party_game::act_seed(self.initial,self.act)?!=self.seed
            ||matches!(local.phase,Phase::Dead|Phase::Won)||local.debug {
            return Err("重连旅程不匹配或进度发生回退，已保留本机存档".into());
        }
        let mut next=local.clone();
        for act in local.act+1..=self.act {
            next.act=act;next.seed=crate::party_game::act_seed(self.initial,act)?;
            if let Some(old)=&next.contract {next.contract=Some(old.next_act(next.reward_seed(),next.class,act));}
        }
        next.act=self.act;next.seed=self.seed;next.schema=self.schema;next.initial_seed=Some(self.initial);
        next.nodes=crate::law_graph(self.seed,self.schema,self.law);next.visited=self.visited.clone();
        next.completed_nodes=self.completed_nodes;next.completed_battles=self.completed_battles;next.reached_layer=self.reached_layer;
        next.phase=Phase::Map;next.checkpoint=None;next.retry_node=None;
        next.regional_event=Default::default();next.shop_purchased=0;next.shop_stock=None;next.elite_buffs.clear();
        next.validate()?;Ok(next)
    }
}
