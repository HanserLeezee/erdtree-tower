use eldenring::cs::{EquipParamGoods, MapItemMan, SoloParamRepository};
use fromsoftware_shared::FromStatic;
use std::{collections::HashMap, ffi::c_void, sync::{LazyLock, Mutex}};

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetModuleHandleW(name: *const u16) -> *mut c_void;
}

/// 已重定位的 RVA 缓存（同 1.17.2 的 2.7.2.1 / 日版等存在偏移漂移时避免重复扫描）
static RELOCATED: LazyLock<Mutex<HashMap<usize, usize>>> = LazyLock::new(Default::default);

/// 解析当前主模块 PE 里的 `.text` 代码段范围 (start_rva, length)
unsafe fn pe_text_range(base: usize) -> Option<(usize, usize)> {
    let dos = unsafe { std::slice::from_raw_parts(base as *const u8, 0x40) };
    if &dos[0..2] != b"MZ" { return None; }
    let pe_off = u32::from_le_bytes(dos[0x3c..0x40].try_into().ok()?) as usize;
    let nt = unsafe { std::slice::from_raw_parts((base + pe_off) as *const u8, 0x18) };
    if &nt[0..4] != b"PE\0\0" { return None; }
    let sections = u16::from_le_bytes(nt[6..8].try_into().ok()?) as usize;
    let opt_size = u16::from_le_bytes(nt[20..22].try_into().ok()?) as usize;
    let table = base + pe_off + 24 + opt_size;
    for i in 0..sections {
        let hdr = unsafe { std::slice::from_raw_parts((table + i * 40) as *const u8, 40) };
        if &hdr[0..5] == b".text" {
            let vsize = u32::from_le_bytes(hdr[8..12].try_into().ok()?) as usize;
            let vaddr = u32::from_le_bytes(hdr[12..16].try_into().ok()?) as usize;
            if vsize > 0 { return Some((vaddr, vsize)); }
        }
    }
    None
}

/// 在给定字节切片内查找唯一匹配；返回 None 表示未命中或命中多处（存在歧义，拒绝盲跳）
fn find_unique(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() { return None; }
    let first = needle[0];
    let mut found = None;
    let mut i = 0;
    let end = haystack.len() - needle.len();
    while i <= end {
        if haystack[i] == first && &haystack[i..i + needle.len()] == needle {
            if found.is_some() { return None; }
            found = Some(i);
        }
        i += 1;
    }
    found
}

pub fn checked_function(rva: usize, expected: &[u8]) -> Result<usize, String> {
    let base = unsafe { GetModuleHandleW(std::ptr::null()) } as usize;
    if base == 0 {
        return Err("游戏模块未就绪".into());
    }
    let direct = base + rva;
    if unsafe { std::slice::from_raw_parts(direct as *const u8, expected.len()) } == expected {
        return Ok(direct);
    }
    // 2.7.2.1 等同版本小修订若有轻微偏移漂移：仅对 ≥12 字节特征码在 .text 内做唯一匹配回退
    if expected.len() >= 12 {
        if let Ok(cache) = RELOCATED.lock()
            && let Some(&cached_rva) = cache.get(&rva)
        {
            let addr = base + cached_rva;
            if unsafe { std::slice::from_raw_parts(addr as *const u8, expected.len()) } == expected {
                return Ok(addr);
            }
        }
        if let Some((text_rva, text_len)) = unsafe { pe_text_range(base) } {
            let text = unsafe { std::slice::from_raw_parts((base + text_rva) as *const u8, text_len) };
            // 先在原 RVA 附近 ±256 KB 搜索，未命中再扫完整 .text 段（两种范围都必须严格唯一命中）
            let rel_center = rva.saturating_sub(text_rva).min(text_len);
            let win_start = rel_center.saturating_sub(0x40000);
            let win_end = (rel_center + 0x40000).min(text_len);
            let matched = find_unique(&text[win_start..win_end], expected)
                .map(|off| text_rva + win_start + off)
                .or_else(|| find_unique(text, expected).map(|off| text_rva + off));
            if let Some(found_rva) = matched {
                if let Ok(mut cache) = RELOCATED.lock() {
                    if cache.insert(rva, found_rva).is_none() {
                        let delta = found_rva as isize - rva as isize;
                        super::log(format!("RVA_RELOCATED 0x{rva:x} -> 0x{found_rva:x} (delta={delta:+#x})"));
                    }
                }
                return Ok(base + found_rva);
            }
        }
    }
    Err(format!("游戏接口版本不匹配（RVA 0x{rva:x}），未修改游戏状态"))
}

/// 同一函数内部的子偏移校验：跟随函数入口的重定位偏移并逐字节核验
pub fn checked_relative(anchor_rva: usize, anchor_addr: usize, target_rva: usize, expected: &[u8]) -> Result<usize, String> {
    let delta = target_rva as isize - anchor_rva as isize;
    let addr = anchor_addr.checked_add_signed(delta).ok_or_else(|| "游戏接口偏移溢出".to_string())?;
    if unsafe { std::slice::from_raw_parts(addr as *const u8, expected.len()) } != expected {
        return Err(format!("游戏接口内部偏移不匹配（target 0x{target_rva:x}），未修改游戏状态"));
    }
    Ok(addr)
}

pub fn give_flasks(red: bool, blue: bool) -> Result<(), String> {
    let params = unsafe { SoloParamRepository::instance() }.map_err(|_| "物品参数未就绪")?;
    if params.get::<EquipParamGoods>(1000).is_none()
        || params.get::<EquipParamGoods>(1050).is_none()
    {
        return Err("未找到本体圣杯瓶参数".into());
    }
    let entries: Vec<_> = [(red, 0x400003e8), (blue, 0x4000041a)]
        .into_iter()
        .filter(|e| e.0)
        .map(|(_, id)| (id, 1, -1))
        .collect();
    give(&entries)
}

pub fn give(entries: &[(i32, i32, i32)]) -> Result<(), String> {
    if entries.is_empty() {
        return Ok(());
    }
    if entries.len() > 2 || entries.iter().any(|(_, qty, _)| *qty <= 0) {
        return Err("无效物品请求".into());
    }
    let manager = unsafe { MapItemMan::instance_mut() }.map_err(|_| "物品系统未就绪")?;
    // Verified against the launcher's SHA-256-pinned 2.7.2.0 executable and CT ItemGive ABI.
    let expected = [
        0x40, 0x55, 0x56, 0x57, 0x41, 0x54, 0x41, 0x55, 0x41, 0x56, 0x41, 0x57,
    ];
    let function = checked_function(0x5618b0, &expected)?;
    let mut items = [0i32; 9];
    for &(id, qty, gem) in entries {
        let offset = 1 + items[0] as usize * 4;
        items[offset..offset + 4].copy_from_slice(&[id, qty, -1, gem]);
        items[0] += 1;
    }
    let mut output = [0u32; 8];
    let give: unsafe extern "C" fn(*mut MapItemMan, *const i32, *mut u32, i32) =
        unsafe { std::mem::transmute(function) };
    unsafe {
        give(manager, items.as_ptr(), output.as_mut_ptr(), 0);
    }
    super::log(format!("Item grant requested: {entries:?}"));
    super::spell_affixes::granted(entries);
    Ok(())
}

fn reward_item(id:u32)->Result<(eldenring::cs::ItemId,&'static str),String> {
    use eldenring::cs::{ItemCategory,ItemId};
    let reward=erdtree_tower::rewards::lookup(id).ok_or("未知奖励物品")?;
    let category=match id>>28 {0=>ItemCategory::Weapon,1=>ItemCategory::Protector,2=>ItemCategory::Accessory,4=>ItemCategory::Goods,_=>return Err("无效奖励类型".into())};
    let item=ItemId::new(category,id&0x0fffffff).map_err(|e|e.to_string())?;
    let params=unsafe {SoloParamRepository::instance()}.map_err(|_|"奖励物品参数未就绪")?;
    if params.get_equip_param(item).is_none(){return Err("奖励物品参数不存在".into());}
    Ok((item,reward.name))
}
fn held(data:&eldenring::cs::PlayerGameData,item:eldenring::cs::ItemId)->u64 {super::loot::held(data,|e|same_reward_item(e,item))}
pub fn same_reward_item(a:eldenring::cs::ItemId,b:eldenring::cs::ItemId)->bool {
    a.category()==b.category()&&if a.category()==eldenring::cs::ItemCategory::Weapon {a.param_id()/100==b.param_id()/100}else{a==b}
}
pub fn check_reward(data:&eldenring::cs::PlayerGameData,id:u32)->Result<u64,String> {
    let (item,name)=reward_item(id)?;let before=held(data,item);
    if item.category()==eldenring::cs::ItemCategory::Goods&&before>0 {return Err(format!("已拥有{name}，请选择其他奖励"));}
    Ok(before)
}
pub fn give_reward(data:&mut eldenring::cs::PlayerGameData,id:u32)->Result<(),String> {
    give_reward_with_ash(data,id,-1)
}
pub fn give_reward_with_ash(data:&mut eldenring::cs::PlayerGameData,id:u32,gem:i32)->Result<(),String> {
    let before=check_reward(data,id)?;
    let granted=super::smith::reward_id(data,id)?;
    super::random_ashes::give(&[(granted as i32,1,gem)])?;
    if held(data,reward_item(id)?.0)<=before {return Err("奖励未进入背包或仓库，请检查容量后重试".into());}
    Ok(())
}

#[test]
fn reward_confirmation_counts_weapon_levels_but_keeps_affixes_affinity_and_spirits_distinct() {
    use eldenring::cs::{ItemId,ItemCategory};
    let weapon=|id|ItemId::new(ItemCategory::Weapon,id).unwrap();
    assert!(same_reward_item(weapon(160001215),weapon(160001200)));
    assert!(!same_reward_item(weapon(160001215),weapon(160001300)));
    assert!(!same_reward_item(weapon(160001215),weapon(160011200)));
    assert!(!same_reward_item(weapon(160001215),ItemId::new(ItemCategory::Protector,160001215).unwrap()));
    assert!(!same_reward_item(ItemId::new(ItemCategory::Goods,2200006).unwrap(),ItemId::new(ItemCategory::Goods,2200000).unwrap()));
    let wanted=weapon(160001200);
    let inventory=[weapon(160001203),weapon(160001215),weapon(160011215)];
    assert_eq!(inventory.iter().filter(|&&id|same_reward_item(id,wanted)).count(),2);
}

pub fn give_stack(data:&mut eldenring::cs::PlayerGameData,id:u32)->Result<(),String> {
    let count=|data:&eldenring::cs::PlayerGameData|super::loot::held(data,|e|e.category()==eldenring::cs::ItemCategory::Goods&&e.param_id()==id);
    let before=count(data);give(&[((0x40000000|id) as i32,1,-1)])?;
    if count(data)<=before{return Err("材料未进入背包或仓库，请清理容量后重试".into());}Ok(())
}

pub fn stack_count(data:&eldenring::cs::PlayerGameData,id:u32)->u64 {
    super::loot::held(data,|e|e.category()==eldenring::cs::ItemCategory::Goods&&e.param_id()==id)
}

#[test]
fn find_unique_accepts_only_single_match() {
    let hay = b"xxABCDEFGHIJKLyy";
    assert_eq!(find_unique(hay, b"ABCDEFGHIJKL"), Some(2));
    assert_eq!(find_unique(hay, b"ZZZZZZZZZZZZ"), None);
    let dup = b"ABCDEFGHIJKL__ABCDEFGHIJKL";
    assert_eq!(find_unique(dup, b"ABCDEFGHIJKL"), None);
}
