use erdtree_tower::{
    embers::Ember,
    localization::{to_traditional, translate_en},
    rewards::{CATALOG, ItemKind, SPIRIT_MAX},
};
use std::collections::BTreeMap;
fn main() {
    let mut rows = BTreeMap::new();
    let mut add = |id: i32, zh: &str, en: String| {
        assert!(
            rows.insert(id, [zh.to_owned(), to_traditional(zh).into_owned(), en])
                .is_none()
        );
    };
    let labels = [
        (
            "余烬重铸（选择要烧掉的余烬）",
            "Reforge an ember (choose one to burn)",
        ),
        (
            "灰烬拾遗（2000 卢恩，三选一）",
            "Search the ashes (2,000 runes, choose one of three)",
        ),
        (
            "吞噬强化（先选要强化的骨灰）",
            "Devour (choose the ash to strengthen first)",
        ),
        (
            "双灰融魂（1000 卢恩，选第一份骨灰）",
            "Fuse ashes (1,000 runes, choose the first ash)",
        ),
        (
            "灵灰淬炼（1200 卢恩，强化两级）",
            "Temper an ash (1,200 runes, +2 levels)",
        ),
        ("唤灵拾遗（1000 卢恩）", "Find a Spirit Ash (1,000 runes)"),
        ("燃尽圣杯", "Burn a flask charge"),
        (
            "献血成狂（失去当前生命的 80%，获得余烬和 2000 卢恩）",
            "Offer blood (lose 80% of current HP for an ember and 2,000 runes)",
        ),
        ("癫火烙印", "Accept the Frenzied Brand"),
        (
            "接受挑战：坚持 30 秒",
            "Accept the challenge: survive for 30 seconds",
        ),
        ("放弃挑战（无奖励）", "Abandon the challenge (no reward)"),
        ("选择要消耗的第二份骨灰", "Choose the second ash to consume"),
        (
            "消耗所选的第二份骨灰，使第一份骨灰提升三级（最高 +39）？",
            "Consume the second ash to raise the first by three levels (maximum +39)?",
        ),
        (
            "消耗所选的两份骨灰和 1000 卢恩，融合成一份新的骨灰？",
            "Consume both ashes and 1,000 runes to create a new Spirit Ash?",
        ),
        (
            "花费 1200 卢恩，将所选骨灰提升两级（最高 +39）？",
            "Spend 1,200 runes to raise this ash by two levels (maximum +39)?",
        ),
        (
            "花费 1000 卢恩，获得一份当前进度等级的随机骨灰？",
            "Spend 1,000 runes to gain a random Spirit Ash at your current progression level?",
        ),
        (
            "本局红瓶上限永久减少一瓶，换取传说武器和高阶护符？",
            "Permanently lose one Crimson Flask charge this run for a legendary weapon and a high-tier talisman?",
        ),
        (
            "失去当前生命的 80%（至少保留 1 点），获得 2000 卢恩？",
            "Lose 80% of current HP (retaining at least 1 HP) for 2,000 runes?",
        ),
        (
            "下一场战斗受到的伤害增加 35%，换取 3500 卢恩和一把传说武器？",
            "Take 35% more damage in the next battle for 3,500 runes and a legendary weapon?",
        ),
        (
            "迎战 2 名精英和 6 名杂兵，坚持 30 秒。联机时全队至少一人存活即可。\n奖励为 1500 卢恩和一件装备，每击杀一名敌人再得 250 卢恩。全歼可得传说武器和高阶护符。参加挑战？",
            "Face 2 elites and 6 foes for 30 seconds. In co-op, one survivor is enough.\nEarn 1,500 runes and equipment, plus 250 runes per kill. Defeat all foes for a legendary weapon and a high-tier talisman. Begin?",
        ),
        (
            "放弃本次挑战并离开？将无法领取奖励。",
            "Leave this challenge without a reward?",
        ),
        ("当前没有可用选项", "No options available"),
        ("上一页", "Previous page"),
        ("下一页", "Next page"),
        ("返回上一步", "Go back"),
    ];
    for (index, (zh, en)) in labels.into_iter().enumerate() {
        add(30_000_000 + index as i32, zh, en.to_owned());
    }
    for e in Ember::ALL.into_iter().chain([Ember::Stride]) {
        let i = e.index() as i32;
        let en = translate_en(e.name());
        add(30_000_100 + i, e.name(), en.to_string());
        let effect = e.effect();
        let quality = if e.quality() >= 3 { "传说" } else { "史诗" };
        for (offset, cost, translated) in [
            (
                0,
                "销毁已选的余烬，换取这一枚？",
                "Burn your selected ember to receive this one?",
            ),
            (
                100,
                "花费 2000 卢恩领取？",
                "Spend 2,000 runes to receive this ember?",
            ),
            (
                200,
                "失去当前生命的 80%（至少保留 1 点），领取这枚余烬和 2000 卢恩？",
                "Lose 80% of current HP (retaining at least 1 HP) for this ember and 2,000 runes?",
            ),
        ] {
            add(
                30_000_200 + offset + i,
                &format!("[{quality}] {}\n{}\n{cost}", e.name(), effect),
                format!(
                    "[{}] {en}\n{}\n{translated}",
                    if e.quality() >= 3 {
                        "Legendary"
                    } else {
                        "Epic"
                    },
                    translate_en(&effect)
                ),
            );
        }
    }
    for (i,(zh,en)) in [
        ("复活骨灰（本局当前等级 +1）","Revive a Spirit Ash (current journey level +1)"),
        ("没有可复活的骨灰，或本次篝火服务已用完。","No Spirit Ash can be revived, or this camp service has been used."),
        ("以列表所示等级复活这份骨灰？将用掉本次篝火服务。","Revive this Spirit Ash at the level shown? This uses your camp service."),
        ("骨灰已复活，已放入物品栏。","Spirit Ash revived and returned to your inventory."),
        ("未能完成复活，请检查背包或重新与我交谈。","Revival could not be completed. Check your inventory and speak to me again."),
    ].into_iter().enumerate(){ add(30_001_000+i as i32,zh,en.to_owned()); }
    for item in CATALOG.iter().filter(|i| i.kind == ItemKind::Spirit) {
        for level in 0..=SPIRIT_MAX {
            let suffix = if level == 0 {
                String::new()
            } else {
                format!(" +{level}")
            };
            add(
                100_000_000 + (item.id & 0x0fffffff) as i32 + i32::from(level),
                &format!("{}{suffix}", item.name),
                format!("{}{suffix}", translate_en(item.name)),
            );
        }
    }
    std::fs::write(
        std::env::args_os().nth(1).unwrap(),
        serde_json::to_vec(&rows).unwrap(),
    )
    .unwrap();
}
