//! Native filesystem backup/restore operations. Uses the historical manifest format.
use serde_json::{json,Value};
use std::{fs,path::{Path,PathBuf,Component}};
fn safe_path(root:&Path,rel:&str)->Result<PathBuf,String>{
    let path=Path::new(rel);
    if rel.is_empty()||path.is_absolute()||rel.contains(':')||path.components().any(|c|matches!(c,Component::ParentDir|Component::RootDir|Component::Prefix(_))||matches!(c,Component::Normal(x) if x==".webcodex")) {
        return Err(format!("unsafe relative path: {rel}"));
    }
    let canonical=fs::canonicalize(root).map_err(|e|e.to_string())?;
    // Canonicalize each existing parent to catch symlink escapes, including dangling final paths.
    let mut target=canonical.clone();
    for component in path.components(){if let Component::Normal(name)=component{
        target.push(name);
        if let Ok(metadata)=fs::symlink_metadata(&target) {
            if metadata.file_type().is_symlink() && !target.exists() {
                return Err("dangling symlink path rejected".into());
            }
            target=fs::canonicalize(&target).map_err(|e|e.to_string())?;
            if !target.starts_with(&canonical){return Err("path escapes project".into());}
        }
    }}
    Ok(target)
}
fn copy(src:&Path,dst:&Path)->Result<(),String>{if let Some(parent)=dst.parent(){fs::create_dir_all(parent).map_err(|e|e.to_string())?;}fs::copy(src,dst).map_err(|e|e.to_string())?;Ok(())}
fn manifest(dir:&Path)->Result<Value,String>{let bytes=fs::read(dir.join("manifest.json")).map_err(|e|e.to_string())?;serde_json::from_slice(&bytes).map_err(|e|e.to_string())}
pub fn handle(op:&str,req:&Value)->Result<Value,String>{
    let root=PathBuf::from(req["root"].as_str().ok_or("missing root")?);
    match op {
        "backup_save"=>{
            let rel=req["rel"].as_str().ok_or("missing rel")?;let dest=safe_path(&root,rel)?;
            let backup=PathBuf::from(req["backup_dir"].as_str().ok_or("missing backup dir")?);
            if !backup.starts_with(root.join(".webcodex").join("backups")){return Err("backup location outside .webcodex/backups".into())}
            if dest.exists(){copy(&dest,&backup.join(rel))?;Ok(json!({"kind":if req["deleting"].as_bool().unwrap_or(false){"deleted"}else{"modified"}}))}
            else{Ok(json!({"kind":"created"}))}
        },
        "backup_undo"=>{
            let base=root.join(".webcodex").join("backups");let mut sets=Vec::new();
            if base.exists(){for item in fs::read_dir(&base).map_err(|e|e.to_string())?{
                let entry=item.map_err(|e|e.to_string())?;let dir=entry.path();if dir.join("manifest.json").is_file(){sets.push(dir)}}}
            sets.sort();let Some(dir)=sets.last() else{return Ok(json!({"restored":[],"nothing":true}))};
            let man=manifest(dir)?;let mut changed=Vec::new();
            for rel in man["created"].as_array().into_iter().flatten().filter_map(Value::as_str){let path=safe_path(&root,rel)?;
                if path.exists(){fs::remove_file(path).map_err(|e|e.to_string())?;}changed.push(rel.to_string());}
            for kind in ["modified","deleted"] {for rel in man[kind].as_array().into_iter().flatten().filter_map(Value::as_str){
                let path=safe_path(&root,rel)?;let source=dir.join(rel);if source.is_file(){copy(&source,&path)?;}changed.push(rel.to_string());
            }}
            fs::remove_dir_all(dir).map_err(|e|e.to_string())?;
            Ok(json!({"restored":changed,"nothing":false}))
        },
        _=>Err(format!("unknown backup operation: {op}"))
    }
}
#[cfg(test)]mod tests{use super::*;#[test]fn rejects_traversal(){let root=std::env::temp_dir();assert!(safe_path(&root,"../bad").is_err());assert!(safe_path(&root,".webcodex/evil").is_err());}}
