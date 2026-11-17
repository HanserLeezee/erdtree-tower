use std::{env,fs};
fn main() {
    let args:Vec<_>=env::args().collect();assert!((3..=4).contains(&args.len()),"localize <input-json> <output-json> [zh-TW]");
    let traditional=args.get(3).is_some_and(|s|s=="zh-TW");
    assert!(args.len()==3||traditional,"Unsupported language");
    let input:Vec<String>=serde_json::from_slice(&fs::read(&args[1]).unwrap()).unwrap();
    let translated:Vec<_>=input.iter().map(|text|if traditional {erdtree_tower::localization::to_traditional(text).into_owned()}else{erdtree_tower::localization::translate_en(text).into_owned()}).collect();
    let missing=translated.iter().filter(|s|s.chars().any(|c|('\u{3400}'..='\u{9fff}').contains(&c))).count();
    fs::write(&args[2],serde_json::to_vec_pretty(&translated).unwrap()).unwrap();
    if traditional {eprintln!("Converted {} Traditional Chinese entries",input.len());}
    else {eprintln!("Translated {} entries; {missing} still contain Chinese",input.len());}
}
