use serde::{Deserialize,Serialize};
use crate::RuleResult;

pub const RULES:[(&str,&str,i32);10]=[
    ("allow_invaders","允许入侵",1),("death_debuffs","死亡惩罚",1),("allow_summons","允许召唤",1),("always_spectate_on_death","死亡观战",1),
    ("enemy_health_scaling","敌人生命缩放",1000),("enemy_damage_scaling","敌人伤害缩放",1000),("enemy_posture_scaling","敌人削韧抗性缩放",1000),
    ("boss_health_scaling","Boss 生命缩放",1000),("boss_damage_scaling","Boss 伤害缩放",1000),("boss_posture_scaling","Boss 削韧抗性缩放",1000),
];
#[derive(Clone,Debug,PartialEq,Eq,Serialize,Deserialize)]
pub struct Compatibility {
    pub tower_build:String,pub seamless_version:String,pub seamless_hash:String,pub assets:String,pub rules:[i32;10],
}
impl Compatibility {
    pub fn validate(&self)->RuleResult<()> {
        if [&self.tower_build,&self.seamless_hash,&self.assets].iter().any(|s|s.len()!=16||!s.bytes().all(|b|b.is_ascii_hexdigit()))
            ||self.seamless_version.is_empty()||self.seamless_version.len()>24||!self.seamless_version.bytes().all(|b|b.is_ascii_digit()||b==b'.')
            ||self.rules.iter().zip(RULES).any(|(&v,(_,_,max))|v<0||v>max){return Err("本机的爬塔联机信息异常。请在启动器里点「校验并修复文件」，然后重启游戏。".into());}
        Ok(())
    }
    pub fn check(&self,other:&Self)->RuleResult<()> {
        self.validate()?;other.validate()?;
        // 识别码只留前 4 位，方便玩家互相核对、也方便排查，不把整串哈希丢给玩家
        let short=|code:&str|code.chars().take(4).collect::<String>();
        if self.tower_build!=other.tower_build {
            return Err(format!("你和队友的爬塔版本不同。请所有人在启动器里更新到同一个最新版本，然后重启游戏。（版本码：你 {}，对方 {}）",short(&self.tower_build),short(&other.tower_build)));
        }
        if self.seamless_version!=other.seamless_version {
            return Err(format!("你和队友的无缝联机版本不同（你 {}，对方 {}）。请大家都使用爬塔自带的无缝联机：在启动器里点「校验并修复文件」，然后重启游戏。",self.seamless_version,other.seamless_version));
        }
        if self.seamless_hash!=other.seamless_hash {
            return Err(format!("你和队友的无缝联机文件不一样，可能有人替换过 ersc.dll。请所有人在启动器里点「校验并修复文件」，然后重启游戏。（文件码：你 {}，对方 {}）",short(&self.seamless_hash),short(&other.seamless_hash)));
        }
        if self.assets!=other.assets {
            return Err(format!("你和队友的爬塔场景资源不一致，可能有人的文件缺失或被改动过。请所有人在启动器里点「校验并修复文件」，然后重启游戏。（资源码：你 {}，对方 {}）",short(&self.assets),short(&other.assets)));
        }
        for (index,(_,label,max)) in RULES.iter().enumerate() {
            let (mine,theirs)=(self.rules[index],other.rules[index]);
            if mine==theirs {continue;}
            let shown=|v:i32|if *max==1 {if v==1{"开".to_string()}else{"关".to_string()}}else{v.to_string()};
            return Err(format!("你和队友的联机设置「{label}」不同（你：{}，对方：{}）。这项需要全队一致：请在启动器的联机设置里改成一样，然后重启游戏。",shown(mine),shown(theirs)));
        }
        Ok(())
    }
}
pub fn parse_rules(text:&str)->RuleResult<[i32;10]> {
    let mut found=[None;10];
    for line in text.trim_start_matches('\u{feff}').lines() {
        let line=line.split([';','#']).next().unwrap_or("").trim();
        let Some((key,value))=line.split_once('=') else{continue;};
        let Some(index)=RULES.iter().position(|(name,_,_)|*name==key.trim()) else{continue;};
        let (_,label,max)=RULES[index];
        if found[index].is_some(){return Err(format!("联机设置文件里「{label}」写了两次。请在启动器里重新保存一次联机设置，然后重启游戏。"));}
        let value=value.trim().parse::<i32>().ok().filter(|v|(0..=max).contains(v)).ok_or_else(||format!("联机设置「{label}」的数值无效。请在启动器里重新保存一次联机设置，然后重启游戏。"))?;
        found[index]=Some(value);
    }
    let mut values=[0;10];
    for (i,entry) in found.into_iter().enumerate(){values[i]=entry.ok_or_else(||format!("联机设置里缺少「{}」。请在启动器里重新保存一次联机设置，然后重启游戏。",RULES[i].1))?;}
    Ok(values)
}
