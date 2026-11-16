/// Per-enemy, per-encounter state. Components are physical, magic, fire, lightning, holy.
#[derive(Clone,Copy,Debug,Default,PartialEq,Eq)]
pub struct Adaptation {last:Option<usize>,stacks:u8}

impl Adaptation {
    /// Call once per resolved direct hit, before HP is changed; never infer hits from HP deltas.
    pub fn receive(&mut self,mut damage:[u32;5],direct:bool)->[u32;5] {
        if !direct {return damage;}
        let mut types=damage.iter().enumerate().filter(|(_,value)|**value>0);
        let Some((kind,_))=types.next() else{return damage;};
        if types.next().is_some() {return damage;}
        if self.last==Some(kind) {
            let reduction=u64::from(damage[kind])*u64::from(self.stacks)*10/100;
            damage[kind]-=reduction as u32;
            self.stacks=(self.stacks+1).min(5);
        } else {
            self.last=Some(kind);self.stacks=1;
        }
        damage
    }
}
