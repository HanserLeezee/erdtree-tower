use eldenring::cs::PlayerGameData;
use std::{collections::{HashMap,HashSet}, fs::OpenOptions, io::{ErrorKind, Write}, sync::{LazyLock,Mutex}};

type Identity=(u32,u32);
type Owner=(u32,i32);
type Key=(u32,u32);
#[derive(Default)]
struct Bindings { owner:Option<Owner>, seeds:HashMap<Key,Identity> }
static BINDINGS:LazyLock<Mutex<Bindings>>=LazyLock::new(Default::default);

fn variant(id:u32)->u32 {if id>>28==0 {id/10000*10000} else {id}}
fn parse(text:&str)->Result<HashMap<Key,Identity>,String> {
    let mut seeds=HashMap::new();
    for line in text.lines().filter(|l|!l.is_empty()) {
        let row:Vec<_>=line.split_whitespace().map(str::parse::<u32>).collect::<Result<_,_>>().map_err(|_|"词条存档格式错误")?;
        let [id,sort,a,b]=row[..] else{return Err("词条存档记录不完整".into())};
        seeds.insert((id,sort),(a,b));
    }
    Ok(seeds)
}
pub fn select(owner:Owner)->Result<(),String> {
    let mut state=BINDINGS.lock().map_err(|_|"词条存档锁不可用")?;
    if state.owner==Some(owner){return Ok(());}
    let path=super::hub::path(owner,"affix-rolls");
    let seeds=match std::fs::read_to_string(path) {
        Ok(text)=>parse(&text)?,
        Err(e) if e.kind()==ErrorKind::NotFound=>HashMap::new(),
        Err(e)=>return Err(format!("读取词条存档失败：{e}")),
    };
    *state=Bindings{owner:Some(owner),seeds};Ok(())
}
pub fn identity(character:u32,item:u32,sort:u32)->Identity {
    BINDINGS.lock().ok().and_then(|state|state.owner.filter(|o|o.0==character)
        .and_then(|_|state.seeds.get(&(variant(item),sort)).copied())).unwrap_or((character,sort))
}
fn copies(data:&PlayerGameData,id:u32)->HashSet<u32> {
    std::iter::once(&data.equipment.equip_inventory_data).chain(data.storage.iter().map(|s|&**s))
        .flat_map(|inv|inv.items_data.normal_entries().iter().filter_map(|e|e.as_option()))
        .filter(|e|e.quantity>0 && variant(e.item_id.into_inner())==id).map(|e|e.sort_id).collect()
}
pub fn grant(data:&mut PlayerGameData,item:u32,seed:Option<Identity>,give:impl FnOnce(&mut PlayerGameData)->Result<(),String>)->Result<(),String> {
    let Some((a,b))=seed else{return give(data)};
    let owner=BINDINGS.lock().map_err(|_|"词条存档锁不可用")?.owner.filter(|o|o.0==data.character_id).ok_or("角色词条存档尚未绑定")?;
    let id=variant(item);
    let mut file=OpenOptions::new().create(true).append(true).open(super::hub::path(owner,"affix-rolls")).map_err(|e|format!("词条存档不可写：{e}"))?;
    let before=copies(data,id);
    give(data)?;
    let after=copies(data,id);
    let mut added=after.difference(&before).copied();
    if let Some(sort)=added.next().filter(|_|added.next().is_none()) {
        BINDINGS.lock().unwrap_or_else(|p|p.into_inner()).seeds.insert((id,sort),(a,b));
        if let Err(e)=writeln!(file,"{id} {sort} {a} {b}").and_then(|_|file.sync_data()) {super::log(format!("CARD_AFFIX persistence failed item={id} sort={sort}: {e}"));}
    } else {super::log(format!("CARD_AFFIX granted item={id}, new inventory entry ambiguous"));}
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stored_card_identity_survives_reload_and_reinforcement() {
        let run=erdtree_tower::Run::new_personal(19,erdtree_tower::Class::Samurai,25);
        let restored=erdtree_tower::Run::decode(&run.encode().unwrap()).unwrap();
        for item in [160030000,537970914] {
            let card=|r,slot|erdtree_tower::weapon_rolls::card_identity(r,3,slot,item).unwrap();
            let (a,b)=card(&run,0);
            assert_eq!(card(&restored,0),(a,b));
            assert_ne!(card(&run,1),(a,b));
            let entries=parse(&format!("{item} 2598 {a} {b}\n")).unwrap();
            assert_eq!(entries.get(&(variant(if item>>28==0{item+1225}else{item}),2598)),Some(&(a,b)));
            assert!(!entries.contains_key(&(item,2599)));
        }
    }
}
