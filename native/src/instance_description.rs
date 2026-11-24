use eldenring::cs::{EquipGameData, EquipInventoryData, EquipInventoryDataListEntry, InventoryItemsData, PlayerGameData};
use std::{collections::HashMap, ffi::c_void, mem::{offset_of, size_of}, sync::{LazyLock, Mutex, atomic::{AtomicUsize, Ordering}}};

#[link(name="kernel32")]
unsafe extern "system" {
    fn GetModuleHandleW(name: *const u16) -> *mut c_void;
    fn GetCurrentProcess() -> *mut c_void;
    fn ReadProcessMemory(process: *mut c_void, address: *const c_void, out: *mut c_void, len: usize, got: *mut usize) -> i32;
}

// UI jobs can be replaced while the text renderer is reading a description.
fn read<const N: usize>(address: usize) -> Option<[u8; N]> {
    if address < 0x10000 { return None; }
    let mut bytes = [0; N];
    let mut got = 0;
    let ok = unsafe {ReadProcessMemory(GetCurrentProcess(),address as *const c_void,bytes.as_mut_ptr().cast(),N,&mut got)};
    (ok != 0 && got == N).then_some(bytes)
}
fn ptr(address: usize) -> Option<usize> { Some(usize::from_le_bytes(read(address)?)) }
fn word(address: usize) -> Option<u32> { Some(u32::from_le_bytes(read(address)?)) }

fn selection(base: usize, query: u32) -> Option<(u32,u32,u32)> {
    // Verified retail 2.7.2 equipment list and inventory dialog, including two equal param IDs.
    let menu = ptr(base+0x3d72840)?;
    let popup = ptr(menu+0x80)?;
    let top = ptr(popup+0x10)?;
    if ptr(top)? != base+0x2b02710 {return None;}
    let mut dialog = ptr(top+0x58)?;
    if ptr(dialog)? == base+0x2af2b18 {dialog=ptr(dialog+0x58)?;}
    let ty = ptr(dialog)?;
    if ty != base+0x2af3f10 && ty != base+0x2afccc8 {return None;}
    let detail=dialog+0xe88;
    if ptr(detail)? != base+0x2b24440 {return None;}
    let record = read::<12>(detail+0xec8)?;
    let [slot,category,param] = [0,4,8].map(|i| u32::from_le_bytes([record[i],record[i+1],record[i+2],record[i+3]]));
    let matches = match category {
        0 => query / 10000 == param / 10000,
        0x20000000 => query == param,
        _ => false,
    };
    if !matches { return None; }
    let game=ptr(base+0x3d64fb8)?;
    let player=ptr(game+8)?;
    let bag=player+offset_of!(PlayerGameData,equipment)+offset_of!(EquipGameData,equip_inventory_data)+offset_of!(EquipInventoryData,items_data);
    let normal=slot.checked_sub(word(bag+offset_of!(InventoryItemsData,key_items_capacity))?)?;
    if normal >= word(bag+offset_of!(InventoryItemsData,normal_items_len))? {return None;}
    let entry=ptr(bag+offset_of!(InventoryItemsData,normal_items_head))?+normal as usize*size_of::<EquipInventoryDataListEntry>();
    let id=word(entry+offset_of!(EquipInventoryDataListEntry,item_id))?;
    if id != (category|param) || word(entry+offset_of!(EquipInventoryDataListEntry,quantity))? == 0 {return None;}
    Some((word(player+offset_of!(PlayerGameData,character_id))?,id,word(entry+offset_of!(EquipInventoryDataListEntry,sort_id))?))
}

fn same_description_item(item:u32,query:u32)->bool {
    match item >> 28 {
        2 => item == (0x20000000 | query),
        0 => item / 10000 == query / 10000,
        _ => false,
    }
}
fn unique_copy(rows:&[u8],query:u32)->Option<(u32,u32)> {
    let field=|row:&[u8],offset|u32::from_le_bytes(row[offset..offset+4].try_into().unwrap());
    let mut matched=None;
    for row in rows.chunks_exact(size_of::<EquipInventoryDataListEntry>()) {
        let item=field(row,offset_of!(EquipInventoryDataListEntry,item_id));
        if same_description_item(item,query) {
            match field(row,offset_of!(EquipInventoryDataListEntry,quantity)) {
                0=>continue,
                1 if matched.is_none()=>matched=Some((item,field(row,offset_of!(EquipInventoryDataListEntry,sort_id)))),
                _=>return None,
            }
        }
    }
    matched
}
fn inventory_fallback(base:usize,query:u32)->Option<(u32,u32,u32)> {
    let menu=ptr(base+0x3d72840)?;
    let top=ptr(ptr(menu+0x80)?+0x10)?;
    if ptr(top)?!=base+0x2b02710 {return None;}
    let dialog=ptr(top+0x58)?;
    if ![base+0x2af2b18,base+0x2afccc8,base+0x2af3f10].contains(&ptr(dialog)?) {return None;}
    let game=ptr(base+0x3d64fb8)?;
    let player=ptr(game+8)?;
    let bag=player+offset_of!(PlayerGameData,equipment)+offset_of!(EquipGameData,equip_inventory_data)+offset_of!(EquipInventoryData,items_data);
    let len=word(bag+offset_of!(InventoryItemsData,normal_items_len))?;
    let capacity=word(bag+offset_of!(InventoryItemsData,normal_items_capacity))?;
    if len==0 || len>capacity || len>16384 {return None;}
    let head=ptr(bag+offset_of!(InventoryItemsData,normal_items_head))?;
    let mut rows=vec![0u8;len as usize*size_of::<EquipInventoryDataListEntry>()];
    let mut got=0;
    if unsafe{ReadProcessMemory(GetCurrentProcess(),head as *const c_void,rows.as_mut_ptr().cast(),rows.len(),&mut got)}==0 || got!=rows.len(){return None;}
    let (item,sort)=unique_copy(&rows,query)?;
    if ptr(game+8)?!=player || ptr(bag+offset_of!(InventoryItemsData,normal_items_head))?!=head || word(bag+offset_of!(InventoryItemsData,normal_items_len))?!=len {return None;}
    Some((word(player+offset_of!(PlayerGameData,character_id))?,item,sort))
}

fn replace_note(original: &str, character:u32, id:u32, sort:u32) -> Option<String> {
    let (start,lang)=["随机附加词条 ×","隨機附加詞條 ×","Random affixes: "].iter().enumerate()
        .find_map(|(i,marker)|original.rfind(marker).map(|p|(p,i)))?;
    let lines=roll_lines(character,id,sort,lang)?;
    let suffix=original[start..].find(['<','\n','\r']).map_or("",|end|&original[start+end..]);
    Some(format!("{}{}{}",&original[..start],lines.join("\n"),suffix))
}

pub fn roll_lines(character:u32,id:u32,sort:u32,lang:usize)->Option<Vec<String>> {
    let effects:Vec<u32>=if let Some(v)=erdtree_tower::talisman_passives::variant(id) {
        erdtree_tower::talisman_passives::instance_effects(v,character,sort).map(|e|e as u32).collect()
    } else { erdtree_tower::weapon_rolls::instance(character,sort,id)?.effects };
    static SPECIAL:LazyLock<Vec<Vec<&str>>>=LazyLock::new(||include_str!("../../scene-assets/weapon-trait-text.tsv")
        .lines().map(|l|l.split('\t').collect()).collect());
    let mut lines=Vec::new();
    for effect in effects {
        let label=if effect>=9875100 {
            SPECIAL.get((effect-9875100) as usize)?.get(lang)?.to_string()
        } else {
            let label=*super::reward_ui::TALISMAN_EFFECT_TEXT.get(&(effect as i32))?;
            match lang {1=>erdtree_tower::localization::to_traditional(label).into_owned(),
                        2=>erdtree_tower::localization::translate_en(label).into_owned(), _=>label.into()}
        };
        lines.push(label);
    }
    Some(lines)
}

type Lookup=unsafe extern "C" fn(usize,i32)->*const u16;
static ORIGINAL:AtomicUsize=AtomicUsize::new(0);
static BASE:AtomicUsize=AtomicUsize::new(0);
type TextKey=(usize,u32,u32,u32);
static TEXT:LazyLock<Mutex<HashMap<TextKey,Box<[u16]>>>>=LazyLock::new(Default::default);
unsafe extern "C" fn lookup(table:usize,query:i32)->*const u16 {
    let original:Lookup=unsafe{std::mem::transmute(ORIGINAL.load(Ordering::Acquire))};
    let raw=unsafe{original(table,query)};
    if raw.is_null() || query<0 {return raw;}
    let id=query as u32;
    if erdtree_tower::weapon_rolls::slot_count(id/10000*10000).is_none() && erdtree_tower::talisman_passives::variant(id|0x20000000).is_none() {return raw;}
    let base=BASE.load(Ordering::Relaxed);
    let selected=selection(base,id).or_else(||{
        let len=(0..8192).position(|i|unsafe{*raw.add(i)==0})?;
        let text=String::from_utf16_lossy(unsafe{std::slice::from_raw_parts(raw,len)});
        if !["随机附加词条 ×","隨機附加詞條 ×","Random affixes: "].iter().any(|s|text.contains(s)){return None;}
        inventory_fallback(base,id)
    });
    let Some((character,item,sort))=selected else{return raw};
    let Ok(mut cache)=TEXT.lock() else{return raw};
    let (character,sort)=super::card_affixes::identity(character,item,sort);
    let key=(raw as usize,character,item,sort);
    if let Some(text)=cache.get(&key){return text.as_ptr();}
    // Native FMG entries are NUL-terminated UTF-16 strings.
    let Some(len)=(0..8192).position(|i|unsafe{*raw.add(i)==0}) else{return raw};
    let text=String::from_utf16_lossy(unsafe{std::slice::from_raw_parts(raw,len)});
    let Some(value)=replace_note(&text,character,item,sort) else{return raw};
    if cache.len()>=8192{return raw;}
    if cache.len()<16 {super::log(format!("ITEM_DETAIL item={item} sort={sort} character={character}"));}
    cache.entry(key).or_insert_with(||value.encode_utf16().chain([0]).collect()).as_ptr()
}

pub fn install()->Result<(),String> {
    use hudhook::mh::{MhHook,MH_EnableHook};
    // Leaf FMG lookup, below MsgRepository (also used by transmog).
    let address=super::items::checked_function(0x2671c40,&[0x44,0x8b,0x41,0x0c,0x45,0x33,0xc9,0x41,0xff,0xc8,0x4c,0x8b,0xd9,0x3b,0x51,0x2c])? as *mut c_void;
    let hook=unsafe{MhHook::new(address,lookup as *mut c_void)}.map_err(|e|format!("{e:?}"))?;
    BASE.store(unsafe{GetModuleHandleW(std::ptr::null())} as usize,Ordering::Relaxed);
    ORIGINAL.store(hook.trampoline() as usize,Ordering::Release);
    unsafe{MH_EnableHook(address)}.ok().map_err(|e|format!("{e:?}"))?;
    super::log("Native weapon and talisman descriptions enabled");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unique_archers_talisman_is_resolved_without_guessing_duplicates() {
        let mut row=vec![0u8;size_of::<EquipInventoryDataListEntry>()];
        for (off,value) in [(offset_of!(EquipInventoryDataListEntry,item_id),537971123u32),(offset_of!(EquipInventoryDataListEntry,quantity),1),(offset_of!(EquipInventoryDataListEntry,sort_id),2868)]{
            row[off..off+4].copy_from_slice(&value.to_le_bytes());
        }
        assert_eq!(unique_copy(&row,1100211),Some((537971123,2868)));
        assert_eq!(unique_copy(&row,1100212),None);
        assert_eq!(unique_copy(&row.repeat(2),1100211),None);
        let lines=roll_lines(0,537971123,2868,0).unwrap();
        assert_eq!(lines.len(),3);
        println!("Archer talisman 2868: {}",lines.join(", "));
    }
    #[test]
    fn selected_copy_uses_its_own_roll_and_keeps_native_text() {
        for (item, note) in [(537970914,"能提升血量的上限\n随机附加词条 ×2，装备后查看。"),(160030000,"补正组合：灵巧/智力\n随机附加词条 ×3，装备后查看。</font>")] {
            let a=replace_note(note,0,item,2598).unwrap();
            let b=replace_note(note,0,item,2599).unwrap();
            assert_ne!(a,b);
            assert_eq!(a,replace_note(note,0,item,2598).unwrap());
            assert!(a.starts_with(note.lines().next().unwrap()));
            assert_eq!(a.ends_with("</font>"),note.ends_with("</font>"));
            assert!(!a.contains("装备后查看"));
        }
    }
}
