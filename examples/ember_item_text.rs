use erdtree_tower::{embers::Ember, localization::{to_traditional, translate_en}};
use std::collections::BTreeMap;

fn main() {
    let rows: BTreeMap<_, _> = Ember::ALL.into_iter().map(|ember| {
        let effect = ember.effect();
        let english = translate_en(&effect).into_owned();
        let traditional = to_traditional(&effect).into_owned();
        (9_900_000 + ember.index(), [effect, traditional, english])
    }).collect();
    println!("{}", serde_json::to_string(&rows).unwrap());
}
