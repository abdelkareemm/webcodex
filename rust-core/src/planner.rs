//! Agent prompt planning, change tracking, and context budgeting.
//! Stateless JSON interface: the Python browser adapter only carries planner state.
use serde_json::{json,Value};
use std::collections::{BTreeMap,BTreeSet};
use std::fs;
use std::path::Path;

fn strings(value:&Value)->Vec<String>{value.as_array().map(|a|a.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default()}
fn object_strings(value:&Value)->BTreeMap<String,String>{value.as_object().map(|v|v.iter().filter_map(|(k,v)|v.as_str().map(|s|(k.clone(),s.to_string()))).collect()).unwrap_or_default()}
fn read(root:&Path,rel:&str)->String{
    let p=root.join(rel);match fs::read(p){Ok(data)=>String::from_utf8(data).unwrap_or_default().replace("\r\n","\n"),Err(_)=>String::new()}
}
fn render_tree(entries:&BTreeMap<String,Value>)->String{
    if entries.is_empty(){return "(empty folder)".into()}
    entries.iter().map(|(rel,e)|{
        let note=e["note"].as_str().unwrap_or("");if note.is_empty(){rel.clone()}else{format!("{rel}  ({note})")}
    }).collect::<Vec<_>>().join("\n")
}
fn render_files(root:&Path, included:&[String])->String{
    included.iter().map(|rel|{
        let mut text=read(root,rel);if !text.is_empty()&&!text.ends_with('\n'){text.push('\n')}
        format!("----- BEGIN {rel} -----\n{text}----- END {rel} -----")
    }).collect::<Vec<_>>().join("\n")
}
fn pinned_from_task(task:&str,entries:&BTreeMap<String,Value>)->Vec<String>{
    let mut pins=Vec::new();
    for word in task.split(|ch:char|ch.is_whitespace()||[',',';',':','\'','"'].contains(&ch)) {
        if let Some(word)=word.strip_prefix('@'){
            let mut rel=word.replace('\\',"/");while let Some(trimmed)=rel.strip_prefix("./"){rel=trimmed.to_string()}
            if entries.get(&rel).is_some_and(|e|e["hash"].is_string()){pins.push(rel)}
        }
    }
    pins
}
pub fn handle(req:&Value)->Result<Value,String>{
    let root=Path::new(req["root"].as_str().ok_or("missing project root")?);
    let entries:BTreeMap<String,Value>=serde_json::from_value(req["entries"].clone()).map_err(|e|e.to_string())?;
    let task=req["task"].as_str();let feedback=strings(&req["feedback"]);let notes=strings(&req["notes"]);
    let sent_hash=object_strings(&req["sent_hash"]);let sent_paths:BTreeSet<String>=strings(&req["sent_paths"]).into_iter().collect();
    let mut not_sent:BTreeSet<String>=strings(&req["not_sent"]).into_iter().collect();
    let mut next_hash=sent_hash.clone();let first=!req["started"].as_bool().unwrap_or(false);
    let mut pins=strings(&req["pinned"]);if let Some(task)=task{pins.extend(pinned_from_task(task,&entries))}
    let mut parts=Vec::new();
    if first {
        parts.push(req["protocol"].as_str().unwrap_or("").to_string());
        let name=root.file_name().map(|n|n.to_string_lossy().to_string()).unwrap_or_default();
        parts.push(format!("PROJECT: {name}\n\nFILE TREE:\n{}",render_tree(&entries)));
    }else{
        let new_paths:BTreeSet<String>=entries.keys().cloned().collect();
        let added:Vec<_>=new_paths.difference(&sent_paths).cloned().collect();
        let removed:Vec<_>=sent_paths.difference(&new_paths).cloned().collect();
        if !added.is_empty()||!removed.is_empty(){
            let mut lines=vec!["FILE TREE CHANGES SINCE YOUR LAST MESSAGE:".to_string()];
            lines.extend(added.iter().map(|r|format!("  added: {r}")));
            lines.extend(removed.iter().map(|r|format!("  removed: {r}")));
            parts.push(lines.join("\n"));
        }
    }
    let mut candidates:Vec<String>=if first{entries.iter().filter(|(_,e)|e["hash"].is_string()).map(|(r,_)|r.clone()).collect()}
        else{entries.iter().filter(|(r,e)|e["hash"].is_string()&&!not_sent.contains(*r)&&next_hash.get(*r).map(String::as_str)!=e["hash"].as_str()).map(|(r,_)|r.clone()).collect()};
    candidates.extend(pins.clone());
    let pinned:BTreeSet<_>=pins.iter().cloned().collect();
    let mut sorted:Vec<String>=candidates.into_iter().filter(|r|entries.contains_key(r)).collect();
    sorted.sort_by_key(|rel|{
        const KEYS:&[&str]=&["readme","pyproject.toml","package.json","cargo.toml","go.mod","requirements.txt","makefile","tsconfig.json","pom.xml","build.gradle","manifest.json"];
        let name=Path::new(rel).file_name().map(|v|v.to_string_lossy().to_lowercase()).unwrap_or_default();
        (!pinned.contains(rel),!KEYS.iter().any(|k|name.starts_with(k)),entries[rel]["chars"].as_u64().unwrap_or(0))
    });
    let mut included=Vec::new();let mut omitted=Vec::new();let mut used=0u64;let mut seen=BTreeSet::new();
    let budget=req["max_chars"].as_u64().unwrap_or(60000);let file_max=req["max_file_chars"].as_u64().unwrap_or(20000);
    for rel in sorted {if !seen.insert(rel.clone())||entries[&rel]["hash"].is_null(){continue}
        let chars=entries[&rel]["chars"].as_u64().unwrap_or(0);
        if pinned.contains(&rel)||(chars<=file_max&&used.saturating_add(chars)<=budget){included.push(rel);used=used.saturating_add(chars)}else{omitted.push(rel)}
    }
    included.sort();omitted.sort();
    if first{not_sent=omitted.iter().cloned().collect()}else{for r in &included{not_sent.remove(r);}}
    for r in &included{next_hash.insert(r.clone(),entries[r]["hash"].as_str().unwrap_or("").into());}
    if first {for r in &omitted{next_hash.insert(r.clone(),entries[r]["hash"].as_str().unwrap_or("").into());}}
    if !notes.is_empty(){parts.push(format!("NOTES FROM THE USER'S TOOL:\n{}",notes.iter().map(|n|format!("- {n}")).collect::<Vec<_>>().join("\n")));}
    if !feedback.is_empty(){parts.push(format!("TOOL RESULTS:\n{}",feedback.join("\n")));}
    if !omitted.is_empty(){parts.push(format!("FILES NOT INCLUDED (too large or over the size budget; ask with NEED: if required):\n{}",omitted.iter().map(|r|format!("  {r}")).collect::<Vec<_>>().join("\n")));}
    if !included.is_empty(){
        let label=if first{"FILE CONTENTS:"}else{"UPDATED / REQUESTED FILE CONTENTS (current state):"};
        parts.push(format!("{label}\n{}",render_files(root,&included)));
    }
    if let Some(task)=task.filter(|s|!s.is_empty()){parts.push(format!("TASK:\n{task}"))}
    else if !feedback.is_empty(){parts.push("Continue with the task, using the tool results above.".into())}
    parts.push(req["reminder"].as_str().unwrap_or("").to_string());
    Ok(json!({"message":parts.join("\n\n"),"sent_hash":next_hash,"sent_paths":entries.keys().collect::<Vec<_>>(),"not_sent":not_sent,"included":included,"omitted":omitted,"pins":pins}))
}
#[cfg(test)]mod tests{use super::*;#[test]fn detects_pins(){let mut e=BTreeMap::new();e.insert("src/main.rs".into(),json!({"hash":"abc"}));assert_eq!(pinned_from_task("edit @src/main.rs",&e),vec!["src/main.rs"]);}}
