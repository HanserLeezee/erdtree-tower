#[derive(Default)]
pub struct HitProtection {pub previous:Option<bool>}
impl HitProtection {
    pub fn update(&mut self,holding:bool,alive:bool,current:bool)->Option<bool> {
        if holding&&alive {self.previous.get_or_insert(current);Some(true)}else{self.previous.take()}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preparation_blocks_hits_and_restores_once_when_actions_resume() {
        let mut guard=HitProtection::default();
        assert_eq!(guard.update(true,true,false),Some(true));
        assert_eq!(guard.update(true,true,true),Some(true));
        assert_eq!(guard.update(false,true,true),Some(false));
        assert_eq!(guard.update(false,true,false),None);
    }
    #[test]
    fn existing_protection_is_preserved_and_dead_enemies_are_not_protected() {
        let mut guard=HitProtection::default();
        assert_eq!(guard.update(true,true,true),Some(true));
        assert_eq!(guard.update(false,true,true),Some(true));
        assert_eq!(guard.update(true,false,false),None);
        assert_eq!(guard.update(true,true,false),Some(true));
        assert_eq!(guard.update(true,false,true),Some(false));
    }
}
