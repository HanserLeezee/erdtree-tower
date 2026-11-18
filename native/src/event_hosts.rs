use erdtree_tower::events::Draw;
pub struct Host {pub draw:Draw,pub entity:u32,pub flag:u32,pub npc:i32,pub name_id:i32}
pub static HOSTS:std::sync::LazyLock<Vec<Host>>=std::sync::LazyLock::new(||include_str!("../../scene-assets/event-hosts.txt").lines().map(|line|{
    let p:Vec<_>=line.split('|').collect();
    let draw=match p[0]{"Trial"=>Draw::Trial,"Chests"=>Draw::Chests,"Altar"=>Draw::Altar,"Locked"=>Draw::Locked,"Scarabs"=>Draw::Scarabs,"Spring"=>Draw::Spring,"Dice"=>Draw::Dice,"Mystery"=>Draw::Mystery,"Reforge"=>Draw::Reforge,_=>panic!("Invalid event host")};
    let flag:u32=p[4].parse().unwrap();
    Host{draw,entity:p[1].parse().unwrap(),flag,npc:95016000+(flag-45011840) as i32,name_id:p[5].parse().unwrap()}
}).collect());
/// 重练炉借用重铸台守炉人；灵庙使用祭坛引导者，包括死亡重试清空事件进度后。
pub fn index(draw:Draw)->usize {
    let draw=match draw {
        Draw::Furnace=>Draw::Reforge,Draw::Mausoleum=>Draw::Altar,
        Draw::Trial|Draw::Chests|Draw::Altar|Draw::Locked|Draw::Scarabs|Draw::Spring|Draw::Dice|Draw::Mystery|Draw::Reforge=>draw,
    };
    HOSTS.iter().position(|h|h.draw==draw).expect("Event host catalog incomplete")
}
pub fn service(index:usize)->Vec<(u32,i32,i32,u32)> {let host=&HOSTS[index];vec![(host.entity,0,host.npc,6)]}
pub fn visibility_flag(entity:u32)->u32 {HOSTS.iter().find(|h|h.entity==entity).map_or(entity+70,|h|h.flag)}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]fn all_event_draws_resolve_even_after_regional_progress_is_cleared(){
        for draw in [Draw::Trial,Draw::Chests,Draw::Altar,Draw::Locked,Draw::Scarabs,Draw::Spring,Draw::Dice,Draw::Mystery,Draw::Reforge,Draw::Mausoleum,Draw::Furnace] {
            assert!(!service(index(draw)).is_empty());
        }
        assert_eq!(index(Draw::Mausoleum),index(Draw::Altar));
        assert_eq!(index(Draw::Furnace),index(Draw::Reforge));
    }
    #[test]fn event_hosts_do_not_share_roderikas_name_parameter(){
        let parameters:std::collections::HashSet<_>=(0..HOSTS.len()).map(|i|service(i)[0].2).collect();
        assert_eq!(parameters.len(),HOSTS.len());
        assert!(!parameters.contains(&523200079));
    }
    #[test]fn host_entities_stay_in_the_character_id_range(){
        for host in HOSTS.iter(){assert!(host.entity%10000<4000,"invalid character target: {}",host.entity);}
    }
    #[test]fn event_visibility_uses_an_allocated_group(){
        // Group 45011 exists in captured game flag tables; 45016 does not.
        for host in HOSTS.iter(){assert_eq!(visibility_flag(host.entity)/1000,45011);}
    }
    #[test]fn other_services_keep_their_existing_flags(){
        for id in [45011800,45011801,45021810]{assert_eq!(visibility_flag(id),id+70);}
    }
}
