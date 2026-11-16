use std::sync::atomic::{AtomicUsize,Ordering};
use eldenring::cs::{ChrIns,FieldInsHandle};

const SITE:usize=0x448b34;
const SIGNATURE:&[u8]=&[0x41,0x8b,0x96,0x28,0x02,0x00,0x00,0x45,0x0f,0xb6,0xcc];
static ORIGINAL:AtomicUsize=AtomicUsize::new(0);
static FILTER:AtomicUsize=AtomicUsize::new(0);
static LINES:AtomicUsize=AtomicUsize::new(0);
static TRACE:std::sync::LazyLock<bool>=std::sync::LazyLock::new(||std::env::var("ERDTREE_TOWER_DAMAGE_TRACE").as_deref()==Ok("1"));

#[derive(Clone,Copy,Debug)]
pub struct Hit {
    pub target:FieldInsHandle,
    pub target_address:usize,
    pub hp_before:i32,
    /// Physical, magic, fire, lightning, holy; attack power before native defense calculation.
    pub power:[f32;5],
    pub percent:[f32;5],
    /// Native final HP damage, after defense and other native modifiers.
    pub damage:i32,
}
impl Hit {
    pub fn pure_type(&self)->Option<usize> {pure_type(self.power,self.percent)}
}
/// The callback must not access or mutate engine objects. None leaves damage unchanged.
pub type Filter=fn(&Hit)->Option<i32>;
pub fn set_filter(filter:Option<Filter>) {FILTER.store(filter.map_or(0,|f|f as usize),Ordering::Release);}
fn pure_type(power:[f32;5],percent:[f32;5])->Option<usize> {
    if power.iter().chain(&percent).any(|v|!v.is_finite()||*v<0.0){return None;}
    let mut present=(0..5).filter(|&i|power[i]>0.0||percent[i]>0.0);
    let first=present.next()?;
    present.next().is_none().then_some(first)
}
fn reduced(original:i32,requested:Option<i32>)->i32 {
    requested.filter(|&value|original>0&&value>=1&&value<=original).unwrap_or(original)
}
unsafe extern "C" fn before_hp(module:usize,packet:usize,hp:i32) {
    let filter=FILTER.load(Ordering::Acquire);
    if module==0||packet==0||hp<=0||filter==0&&!*TRACE {return;}
    let target=unsafe {((module+8) as *const *const ChrIns).read_unaligned()};
    let Some(chr)=(unsafe {target.as_ref()}) else{return;};
    let value=unsafe {((packet+0x228) as *const i32).read_unaligned()};
    if value<=0 {return;}
    let hit=Hit{target:chr.field_ins_handle,target_address:target as usize,hp_before:hp,
        power:unsafe {(packet as *const [f32;5]).read_unaligned()},
        percent:unsafe {((packet+0x1e8) as *const [f32;5]).read_unaligned()},damage:value};
    let request=if filter==0 {None}else{
        let callback:Filter=unsafe {std::mem::transmute(filter)};
        std::panic::catch_unwind(||callback(&hit)).ok().flatten()
    };
    let damage=reduced(value,request);
    if damage!=value {unsafe {((packet+0x228) as *mut i32).write_unaligned(damage)};}
    if *TRACE&&LINES.fetch_add(1,Ordering::Relaxed)<256 {
        super::log(format!("DAMAGE_HIT target={} model={} hp={} power={:?} percent={:?} pure={:?} final={} applied={damage}",
            hit.target,chr.npc_id,hp,hit.power,hit.percent,hit.pure_type(),value));
    }
}

#[unsafe(naked)]
unsafe extern "C" fn entry() {
    core::arch::naked_asm!(
        "pushfq","sub rsp, 0xc8",
        "mov [rsp+0x20], rax","mov [rsp+0x28], rcx","mov [rsp+0x30], rdx",
        "mov [rsp+0x38], r8","mov [rsp+0x40], r9","mov [rsp+0x48], r10","mov [rsp+0x50], r11",
        "movdqu [rsp+0x60], xmm0","movdqu [rsp+0x70], xmm1","movdqu [rsp+0x80], xmm2",
        "movdqu [rsp+0x90], xmm3","movdqu [rsp+0xa0], xmm4","movdqu [rsp+0xb0], xmm5",
        "mov rcx, r15","mov rdx, r14","mov r8d, ebx","call {}",
        "movdqu xmm0, [rsp+0x60]","movdqu xmm1, [rsp+0x70]","movdqu xmm2, [rsp+0x80]",
        "movdqu xmm3, [rsp+0x90]","movdqu xmm4, [rsp+0xa0]","movdqu xmm5, [rsp+0xb0]",
        "mov rax, [rsp+0x20]","mov rcx, [rsp+0x28]","mov rdx, [rsp+0x30]",
        "mov r8, [rsp+0x38]","mov r9, [rsp+0x40]","mov r10, [rsp+0x48]","mov r11, [rsp+0x50]",
        "add rsp, 0xc8","popfq","jmp qword ptr [rip+{}]",
        sym before_hp,sym ORIGINAL);
}
pub unsafe fn install()->Result<(),String> {
    use hudhook::mh::{MhHook,MH_EnableHook};
    // Validate the containing function, owner accessor and HP-deduction call as well as the hook site.
    super::items::checked_function(0x4489f0,&[0x4c,0x8b,0xdc,0x55,0x53,0x56,0x57,0x41,0x56,0x41,0x57,0x49,0x8d,0x6b,0x88,0x48,0x81,0xec,0x48,0x01,0,0])?;
    super::items::checked_function(0x43d2e0,&[0x48,0x8b,0x41,0x08,0xc3])?;
    super::items::checked_function(0x448b6c,&[0xf7,0xda,0xf3,0x0f,0x11,0x4c,0x24,0x28,0x88,0x4c,0x24,0x20,0x49,0x8b,0xcd,0xe8,0xf0,0xdf,0xfe,0xff])?;
    let address=super::items::checked_function(SITE,SIGNATURE)? as *mut std::ffi::c_void;
    let hook=unsafe {MhHook::new(address,entry as *mut std::ffi::c_void)}.map_err(|e|format!("Damage hook: {e:?}"))?;
    ORIGINAL.store(hook.trampoline() as usize,Ordering::Release);
    unsafe {MH_EnableHook(address)}.ok().map_err(|e|format!("Damage hook enable: {e:?}"))?;
    super::log("DAMAGE_HIT pre-HP interface installed; filter unset; channel order physical/magic/fire/lightning/holy");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mixed_percent_and_invalid_channels_never_become_pure() {
        for i in 0..5 {
            let mut pure=[0.0;5];pure[i]=0.01;
            assert_eq!(pure_type(pure,[0.0;5]),Some(i));
            assert_eq!(pure_type([0.0;5],pure),Some(i));
            for j in 0..5 {if i!=j {let mut extra=[0.0;5];extra[j]=1.0;assert_eq!(pure_type(pure,extra),None);}}
        }
        for value in [f32::NAN,f32::INFINITY,-1.0] {assert_eq!(pure_type([value,1.0,0.0,0.0,0.0],[0.0;5]),None);}
        assert_eq!(pure_type([0.0;5],[0.0;5]),None);
    }
    #[test]
    fn filters_can_only_reduce_positive_damage_without_healing_or_immunity() {
        for request in [None,Some(-1),Some(0),Some(101),Some(i32::MAX)] {assert_eq!(reduced(100,request),100);}
        assert_eq!(reduced(100,Some(50)),50);assert_eq!(reduced(1,Some(1)),1);
        assert_eq!(reduced(0,Some(1)),0);assert_eq!(reduced(-10,Some(1)),-10);
    }
}
