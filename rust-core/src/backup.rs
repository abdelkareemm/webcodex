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
fn copy_staged(src:&Path,dst:&Path)->Result<(),String>{
    copy(src,dst)?;
    // Windows FlushFileBuffers requires a writable handle.
    // Opening an existing file for writing does not truncate it.
    let file=fs::OpenOptions::new()
        .write(true)
        .open(dst)
        .map_err(|e|format!("Cannot open staged recovery file {}: {e}",dst.display()))?;
    file.sync_all()
        .map_err(|e|format!("Cannot synchronize staged recovery file {}: {e}",dst.display()))
}

fn manifest(dir:&Path)->Result<Value,String>{let bytes=fs::read(dir.join("manifest.json")).map_err(|e|e.to_string())?;serde_json::from_slice(&bytes).map_err(|e|e.to_string())}

struct UndoItem {
    rel: String,
    target: PathBuf,
    replacement: Option<PathBuf>,
    original: Option<PathBuf>,
}

fn write_recovery_journal(stage: &Path, items: &[UndoItem]) -> Result<(), String> {
    use std::io::Write;

    let entries: Vec<Value> = items.iter().map(|item| {
        json!({
            "rel": item.rel,
            "had_original": item.original.is_some(),
            "has_replacement": item.replacement.is_some()
        })
    }).collect();

    let contents = serde_json::to_vec(&json!({
        "version": 1,
        "entries": entries
    })).map_err(|e| e.to_string())?;

    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(stage.join("journal.json"))
        .map_err(|e| format!("Cannot create recovery journal: {e}"))?;

    file.write_all(&contents)
        .map_err(|e| format!("Cannot write recovery journal: {e}"))?;

    file.sync_all()
        .map_err(|e| format!("Cannot synchronize recovery journal: {e}"))?;

    Ok(())
}

fn apply_restore(
    root: &Path,
    dir: &Path,
    man: &Value,
    fail_after: Option<usize>,
) -> Result<Vec<String>,String> {
    let stage=dir.join(".undo-recovery");
    fs::create_dir(&stage).map_err(|e|format!("Cannot initialize undo recovery: {e}"))?;

    let prepared=(|| -> Result<Vec<UndoItem>,String> {
        let mut items=Vec::new();
        for kind in ["created","modified","deleted"] {
            for entry in man[kind].as_array().ok_or("Invalid backup manifest")? {
                let rel=entry.as_str().ok_or("Invalid backup path")?;
                let target=safe_path(root,rel)?;
                let index=items.len();

                let replacement=if kind=="created" {
                    None
                } else {
                    let staged=stage.join(format!("new-{index}"));
                    copy_staged(&dir.join(rel),&staged)?;
                    Some(staged)
                };

                let original=if target.exists() {
                    let staged=stage.join(format!("old-{index}"));
                    copy_staged(&target,&staged)?;
                    Some(staged)
                } else {
                    None
                };

                items.push(UndoItem {
                    rel:rel.to_string(),
                    target,
                    replacement,
                    original,
                });
            }
        }
        Ok(items)
    })();

    let items=match prepared {
        Ok(items)=>items,
        Err(error)=>{
            let _=fs::remove_dir_all(&stage);
            return Err(format!("Undo preparation failed; project unchanged: {error}"));
        }
    };

    if let Err(error)=write_recovery_journal(&stage,&items) {
        let cleanup=fs::remove_dir_all(&stage);
        return Err(match cleanup {
            Ok(())=>format!("Undo journal preparation failed; project unchanged: {error}"),
            Err(reason)=>format!(
                "Undo journal preparation failed: {error}. Project unchanged; recovery directory retained because cleanup failed: {reason}"
            ),
        });
    }

    let mut attempted=Vec::new();
    for (index,item) in items.iter().enumerate() {
        let result=if fail_after==Some(index) {
            Err("Injected undo failure".to_string())
        } else {
            attempted.push(index);
            match &item.replacement {
                Some(source)=>copy(source,&item.target),
                None=>{
                    if item.target.exists() {
                        fs::remove_file(&item.target).map_err(|e|e.to_string())
                    } else {
                        Ok(())
                    }
                }
            }
        };

        if let Err(error)=result {
            let mut rollback_errors=Vec::new();

            for &done in attempted.iter().rev() {
                let previous=&items[done];
                let rollback=match &previous.original {
                    Some(saved)=>copy(saved,&previous.target),
                    None=>{
                        if previous.target.exists() {
                            fs::remove_file(&previous.target).map_err(|e|e.to_string())
                        } else {
                            Ok(())
                        }
                    }
                };
                if let Err(reason)=rollback {
                    rollback_errors.push(format!("{}: {reason}",previous.rel));
                }
            }

            if !rollback_errors.is_empty() {
                return Err(format!(
                    "Undo failed: {error}. Rollback incomplete: {}. Recovery files retained in {}",
                    rollback_errors.join("; "),
                    stage.display()
                ));
            }

            if let Err(cleanup)=fs::remove_dir_all(&stage) {
                return Err(format!(
                    "Undo failed: {error}. Project rollback completed, but recovery cleanup failed: {cleanup}"
                ));
            }

            return Err(format!("Undo failed and project rollback completed: {error}"));
        }
    }

    // Keep recovery snapshots until the backup has been retired.

    Ok(items.into_iter().map(|item|item.rel).collect())
}

fn recover_pending(root:&Path,dir:&Path)->Result<Vec<String>,String>{
    let stage=dir.join(".undo-recovery");
    let metadata=fs::symlink_metadata(&stage)
        .map_err(|e|format!("Recovery directory unavailable: {e}"))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err("Unsafe recovery directory".into());
    }

    let journal_path=stage.join("journal.json");
    let metadata=fs::symlink_metadata(&journal_path)
        .map_err(|e|format!("Recovery journal unavailable: {e}"))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err("Unsafe recovery journal".into());
    }

    let journal:Value=serde_json::from_slice(
        &fs::read(&journal_path).map_err(|e|e.to_string())?
    ).map_err(|e|format!("Invalid recovery journal: {e}"))?;

    if journal["version"].as_u64()!=Some(1) {
        return Err("Unsupported recovery journal version".into());
    }

    let entries=journal["entries"].as_array()
        .ok_or("Invalid recovery journal entries")?;
    let stage_root=fs::canonicalize(&stage).map_err(|e|e.to_string())?;
    let mut seen=std::collections::HashSet::new();
    let mut plan=Vec::new();

    for (index,entry) in entries.iter().enumerate() {
        let rel=entry["rel"].as_str().ok_or("Invalid recovery path")?;
        let had_original=entry["had_original"].as_bool()
            .ok_or("Invalid recovery original flag")?;
        entry["has_replacement"].as_bool()
            .ok_or("Invalid recovery replacement flag")?;

        let target=safe_path(root,rel)?;
        if !seen.insert(target.clone()) {
            return Err(format!("Duplicate recovery destination: {rel}"));
        }
        if target.exists() && !target.is_file() {
            return Err(format!("Recovery destination is not a file: {rel}"));
        }

        let mut parent=target.parent();
        while let Some(directory)=parent {
            if directory.exists() {
                if !directory.is_dir() {
                    return Err(format!("Invalid recovery parent: {rel}"));
                }
                break;
            }
            parent=directory.parent();
        }

        let original=if had_original {
            let saved=stage.join(format!("old-{index}"));
            let metadata=fs::symlink_metadata(&saved)
                .map_err(|e|format!("Missing recovery original for {rel}: {e}"))?;
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err(format!("Invalid recovery original: {rel}"));
            }
            let resolved=fs::canonicalize(&saved).map_err(|e|e.to_string())?;
            if resolved.parent()!=Some(stage_root.as_path()) {
                return Err(format!("Recovery original escapes storage: {rel}"));
            }
            Some(saved)
        } else {
            None
        };

        plan.push((rel.to_string(),target,original));
    }

    for (rel,target,original) in &plan {
        let result=match original {
            Some(saved)=>copy(saved,target),
            None=>{
                if target.exists() {
                    fs::remove_file(target).map_err(|e|e.to_string())
                } else {
                    Ok(())
                }
            }
        };
        if let Err(error)=result {
            return Err(format!(
                "Recovery incomplete at {rel}: {error}. Snapshots retained for retry."
            ));
        }
    }

    // Keep snapshots and journal until the backup is retired from active storage.
    Ok(plan.into_iter().map(|(rel,_,_)|rel).collect())
}

pub fn handle(op:&str,req:&Value)->Result<Value,String>{
    let root=PathBuf::from(req["root"].as_str().ok_or("missing root")?);
    match op {
        "backup_recover"=>{
            let base=root.join(".webcodex").join("backups");
            let root_real=fs::canonicalize(&root).map_err(|e|e.to_string())?;
            let base_real=fs::canonicalize(&base)
                .map_err(|e|format!("Backup storage unavailable: {e}"))?;

            if base_real!=root_real.join(".webcodex").join("backups") {
                return Err("Recovery storage resolves outside project".into());
            }

            let mut sets=Vec::new();
            for item in fs::read_dir(&base).map_err(|e|e.to_string())? {
                let dir=item.map_err(|e|e.to_string())?.path();
                if fs::symlink_metadata(dir.join(".undo-recovery")).is_ok() {
                    sets.push(dir);
                }
            }
            sets.sort();
            if sets.len()>1 {
                return Err(format!(
                    "Multiple pending backup recoveries found ({}). Recovery aborted.",
                    sets.len()
                ));
            }
            let dir=sets.last().ok_or("No pending backup recovery found")?;
            let metadata=fs::symlink_metadata(dir).map_err(|e|e.to_string())?;

            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                return Err("Unsafe backup set; recovery aborted".into());
            }

            let resolved=fs::canonicalize(dir).map_err(|e|e.to_string())?;
            if resolved.parent()!=Some(base_real.as_path()) {
                return Err("Backup set escapes recovery storage".into());
            }

            let recovered=recover_pending(&root,dir)?;
            let name=dir.file_name().ok_or("Invalid backup directory name")?;
            let retired=root.join(".webcodex").join(
                format!(".consumed-backup-{}",name.to_string_lossy())
            );
            if fs::symlink_metadata(&retired).is_ok(){
                return Err(format!(
                    "Files restored, but backup retirement is blocked by {}. Recovery snapshots retained.",
                    retired.display()
                ));
            }
            fs::rename(dir,&retired).map_err(|e|format!(
                "Files restored, but backup retirement failed: {e}. Recovery snapshots retained."
            ))?;
            fs::remove_dir_all(&retired).map_err(|e|format!(
                "Files restored and backup retired, but cleanup failed at {}: {e}",
                retired.display()
            ))?;
            Ok(json!({"recovered":recovered,"nothing":false}))
        },
        "backup_save"=>{
            let rel=req["rel"].as_str().ok_or("missing rel")?;let dest=safe_path(&root,rel)?;
            let backup=PathBuf::from(req["backup_dir"].as_str().ok_or("missing backup dir")?);
            let base=root.join(".webcodex").join("backups");
            if backup.parent()!=Some(base.as_path()) || backup.file_name().is_none() {
                return Err("backup location outside .webcodex/backups".into());
            }
            for candidate in [root.join(".webcodex"),base.clone(),backup.clone()] {
                if let Ok(metadata)=fs::symlink_metadata(&candidate) {
                    if metadata.file_type().is_symlink() {
                        return Err("Symbolic link in backup directory rejected".into());
                    }
                }
            }
            if dest.exists(){
                fs::create_dir_all(&backup).map_err(|e|e.to_string())?;
                let saved=safe_path(&backup,rel)?;
                copy(&dest,&saved)?;
                Ok(json!({"kind":if req["deleting"].as_bool().unwrap_or(false){"deleted"}else{"modified"}}))
            }
            else{Ok(json!({"kind":"created"}))}
        },
        "backup_undo"=>{
            let base=root.join(".webcodex").join("backups");let mut sets=Vec::new();
            if base.exists(){
                for item in fs::read_dir(&base).map_err(|e|e.to_string())?{
                    let entry=item.map_err(|e|e.to_string())?;
                    let dir=entry.path();
                    if fs::symlink_metadata(dir.join(".undo-recovery")).is_ok(){
                        return Err(format!(
                            "Pending undo recovery in {}. Run /recover-files before attempting another undo.",
                            dir.display()
                        ));
                    }
                    if dir.join("manifest.json").is_file(){sets.push(dir);}
                }
            }
            sets.sort();let Some(dir)=sets.last() else{return Ok(json!({"restored":[],"nothing":true}))};
            let metadata=fs::symlink_metadata(dir).map_err(|e|e.to_string())?;
            if metadata.file_type().is_symlink() {
                return Err("Symbolic-link backup set rejected. Undo aborted.".into());
            }
            let base=fs::canonicalize(&base).map_err(|e|e.to_string())?;
            let resolved=fs::canonicalize(dir).map_err(|e|e.to_string())?;
            if resolved.parent()!=Some(base.as_path()) {
                return Err("Backup set escapes backup storage. Undo aborted.".into());
            }
            let man=manifest(dir)?;
            let mut seen=std::collections::HashSet::new();
            for kind in ["created","modified","deleted"] {
                let entries=man[kind].as_array()
                    .ok_or_else(||format!("Invalid backup manifest: {kind}"))?;
                for entry in entries {
                    let rel=entry.as_str().ok_or("Invalid backup path")?;
                    let target=safe_path(&root,rel)?;
                    let mut parent=target.parent();
                    while let Some(directory)=parent {
                        if directory.exists() {
                            if !directory.is_dir() {
                                return Err(format!("Backup destination parent is not a directory: {rel}. Undo aborted."));
                            }
                            break;
                        }
                        parent=directory.parent();
                    }
                    if !seen.insert(target.clone()) {
                        return Err(format!("Duplicate backup manifest destination: {rel}. Undo aborted."));
                    }
                    if target.exists() && !target.is_file() {
                        return Err(format!("Backup destination is not a regular file: {rel}. Undo aborted."));
                    }
                    if kind!="created" {
                        let source=dir.join(rel);
                        if !source.is_file() {
                            return Err(format!("Missing backup file: {rel}. Undo aborted."));
                        }
                        let backup_root=fs::canonicalize(dir).map_err(|e|e.to_string())?;
                        let resolved=fs::canonicalize(&source).map_err(|e|e.to_string())?;
                        if !resolved.starts_with(&backup_root) {
                            return Err(format!("Backup source escapes its directory: {rel}. Undo aborted."));
                        }
                    }
                }
            }
            let retired=root.join(".webcodex").join(format!(".consumed-backup-{}",dir.file_name().unwrap().to_string_lossy()));
            if fs::symlink_metadata(&retired).is_ok(){return Err("Backup retirement destination already exists; undo aborted.".into());}
            let changed=apply_restore(&root,dir,&man,None)?;
            fs::rename(dir,&retired).map_err(|e|format!("Undo applied but retirement failed: {e}. Run /recover-files."))?;
            fs::remove_dir_all(&retired).map_err(|e|format!("Retired backup cleanup failed: {e}"))?;
            Ok(json!({"restored":changed,"nothing":false}))
        },
        _=>Err(format!("unknown backup operation: {op}"))
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_traversal(){
        let root=std::env::temp_dir();
        assert!(safe_path(&root,"../bad").is_err());
        assert!(safe_path(&root,".webcodex/evil").is_err());
    }

    #[cfg(unix)]
    #[test]
    fn backup_symlink_escape_is_rejected(){
        use std::os::unix::fs::symlink;

        let root=std::env::temp_dir().join(format!("webcodex-backup-link-{}",std::process::id()));
        let _=fs::remove_dir_all(&root);
        let backup=root.join(".webcodex/backups/0001");
        fs::create_dir_all(&backup).unwrap();
        fs::write(root.join("config.txt"),"current").unwrap();
        fs::write(root.join("external.txt"),"external secret").unwrap();
        symlink(root.join("external.txt"),backup.join("config.txt")).unwrap();

        let data=json!({"created":[],"modified":["config.txt"],"deleted":[]});
        fs::write(backup.join("manifest.json"),data.to_string()).unwrap();

        let request=json!({"root":root.to_str().unwrap()});
        let result=handle("backup_undo",&request);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("escapes its directory"));
        assert_eq!(fs::read_to_string(root.join("config.txt")).unwrap(),"current");
        assert!(backup.join("manifest.json").exists());

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn backup_save_rejects_destination_traversal(){
        let root=std::env::temp_dir().join(format!("webcodex-backup-destination-{}",std::process::id()));
        let _=fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("file.txt"),"original").unwrap();

        let base=root.join(".webcodex/backups");
        let valid=base.join("0001");
        let request=json!({
            "root":root.to_str().unwrap(),
            "rel":"file.txt",
            "backup_dir":valid.to_str().unwrap()
        });
        assert!(handle("backup_save",&request).is_ok());
        assert_eq!(fs::read_to_string(valid.join("file.txt")).unwrap(),"original");

        let escaped=base.join("..").join("escaped");
        let invalid=json!({
            "root":root.to_str().unwrap(),
            "rel":"file.txt",
            "backup_dir":escaped.to_str().unwrap()
        });
        let result=handle("backup_save",&invalid);
        assert!(result.is_err());
        assert!(!root.join(".webcodex/escaped/file.txt").exists());

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn file_parent_prevents_partial_undo(){
        let root=std::env::temp_dir().join(format!("webcodex-undo-parent-{}",std::process::id()));
        let _=fs::remove_dir_all(&root);
        let backup=root.join(".webcodex/backups/0001");
        fs::create_dir_all(backup.join("blocked")).unwrap();
        fs::write(root.join("first.txt"),"current").unwrap();
        fs::write(root.join("blocked"),"not a directory").unwrap();
        fs::write(backup.join("first.txt"),"previous").unwrap();
        fs::write(backup.join("blocked/child.txt"),"old child").unwrap();

        let data=json!({
            "created":[],
            "modified":["first.txt","blocked/child.txt"],
            "deleted":[]
        });
        fs::write(backup.join("manifest.json"),data.to_string()).unwrap();

        let request=json!({"root":root.to_str().unwrap()});
        let result=handle("backup_undo",&request);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("parent is not a directory"));
        assert_eq!(fs::read_to_string(root.join("first.txt")).unwrap(),"current");
        assert_eq!(fs::read_to_string(root.join("blocked")).unwrap(),"not a directory");
        assert!(backup.join("manifest.json").exists());

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn unsafe_recovery_path_prevents_partial_recovery(){
        let root=std::env::temp_dir().join(format!(
            "webcodex-recover-traversal-{}",std::process::id()
        ));
        let _=fs::remove_dir_all(&root);
        let stage=root.join(".webcodex/backups/0001/.undo-recovery");
        fs::create_dir_all(&stage).unwrap();

        fs::write(root.join("valid.txt"),"keep current").unwrap();
        fs::write(stage.join("old-0"),"previous version").unwrap();

        let journal=json!({
            "version":1,
            "entries":[
                {
                    "rel":"valid.txt",
                    "had_original":true,
                    "has_replacement":false
                },
                {
                    "rel":"../outside.txt",
                    "had_original":false,
                    "has_replacement":true
                }
            ]
        });
        fs::write(stage.join("journal.json"),journal.to_string()).unwrap();

        let result=handle("backup_recover",&json!({
            "root":root.to_str().unwrap()
        }));

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("unsafe relative path"));
        assert_eq!(
            fs::read_to_string(root.join("valid.txt")).unwrap(),
            "keep current"
        );
        assert!(stage.join("old-0").exists());
        assert!(stage.join("journal.json").exists());

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn missing_recovery_original_prevents_partial_recovery(){
        let root=std::env::temp_dir().join(format!(
            "webcodex-recover-preflight-{}",std::process::id()
        ));
        let _=fs::remove_dir_all(&root);
        let backup=root.join(".webcodex/backups/0001");
        let stage=backup.join(".undo-recovery");
        fs::create_dir_all(&stage).unwrap();

        fs::write(root.join("first.txt"),"keep first").unwrap();
        fs::write(root.join("second.txt"),"keep second").unwrap();
        fs::write(stage.join("old-0"),"previous first").unwrap();

        let items=vec![
            UndoItem {
                rel:"first.txt".into(),
                target:root.join("first.txt"),
                replacement:None,
                original:Some(stage.join("old-0")),
            },
            UndoItem {
                rel:"second.txt".into(),
                target:root.join("second.txt"),
                replacement:None,
                original:Some(stage.join("old-1")),
            },
        ];
        write_recovery_journal(&stage,&items).unwrap();

        let result=handle("backup_recover",&json!({
            "root":root.to_str().unwrap()
        }));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Missing recovery original"));
        assert_eq!(fs::read_to_string(root.join("first.txt")).unwrap(),"keep first");
        assert_eq!(fs::read_to_string(root.join("second.txt")).unwrap(),"keep second");
        assert!(stage.join("journal.json").exists());

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn pending_recovery_restores_pre_undo_state(){
        let root=std::env::temp_dir().join(format!(
            "webcodex-recover-success-{}",std::process::id()
        ));
        let _=fs::remove_dir_all(&root);
        let backup=root.join(".webcodex/backups/0001");
        let stage=backup.join(".undo-recovery");
        fs::create_dir_all(&stage).unwrap();

        fs::write(root.join("modified.txt"),"restored old version").unwrap();
        fs::write(stage.join("old-0"),"current original").unwrap();
        fs::write(root.join("newly-restored.txt"),"should disappear").unwrap();

        let items=vec![
            UndoItem {
                rel:"modified.txt".into(),
                target:root.join("modified.txt"),
                replacement:Some(stage.join("new-0")),
                original:Some(stage.join("old-0")),
            },
            UndoItem {
                rel:"newly-restored.txt".into(),
                target:root.join("newly-restored.txt"),
                replacement:Some(stage.join("new-1")),
                original:None,
            },
        ];
        write_recovery_journal(&stage,&items).unwrap();

        let result=handle("backup_recover",&json!({
            "root":root.to_str().unwrap()
        })).unwrap();

        assert_eq!(result["recovered"].as_array().unwrap().len(),2);
        assert_eq!(
            fs::read_to_string(root.join("modified.txt")).unwrap(),
            "current original"
        );
        assert!(!root.join("newly-restored.txt").exists());
        assert!(!stage.exists());

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn multiple_pending_recoveries_are_rejected(){
        let root=std::env::temp_dir().join(format!(
            "webcodex-multiple-recoveries-{}",std::process::id()
        ));
        let _=fs::remove_dir_all(&root);
        let first=root.join(".webcodex/backups/0001/.undo-recovery");
        let second=root.join(".webcodex/backups/0002/.undo-recovery");
        fs::create_dir_all(&first).unwrap();
        fs::create_dir_all(&second).unwrap();

        fs::write(root.join("example.txt"),"current contents").unwrap();
        fs::write(first.join("old-0"),"first original").unwrap();
        fs::write(second.join("old-0"),"second original").unwrap();

        let request=json!({"root":root.to_str().unwrap()});
        let result=handle("backup_recover",&request);

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Multiple pending backup recoveries"));
        assert_eq!(
            fs::read_to_string(root.join("example.txt")).unwrap(),
            "current contents"
        );
        assert!(first.join("old-0").exists());
        assert!(second.join("old-0").exists());

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn blocked_retirement_preserves_pending_recovery(){
        let root=std::env::temp_dir().join(format!(
            "webcodex-retire-blocked-{}",std::process::id()
        ));
        let _=fs::remove_dir_all(&root);
        let backup=root.join(".webcodex/backups/0001");
        let stage=backup.join(".undo-recovery");
        fs::create_dir_all(&stage).unwrap();
        fs::write(root.join("example.txt"),"interrupted undo").unwrap();
        fs::write(stage.join("old-0"),"pre-undo contents").unwrap();

        let items=vec![UndoItem{
            rel:"example.txt".into(),
            target:root.join("example.txt"),
            replacement:None,
            original:Some(stage.join("old-0")),
        }];
        write_recovery_journal(&stage,&items).unwrap();
        fs::create_dir_all(root.join(".webcodex/.consumed-backup-0001")).unwrap();

        let request=json!({"root":root.to_str().unwrap()});
        let result=handle("backup_recover",&request);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("retirement is blocked"));
        assert_eq!(fs::read_to_string(root.join("example.txt")).unwrap(),"pre-undo contents");
        assert!(stage.join("journal.json").exists());
        assert!(stage.join("old-0").exists());

        let undo=handle("backup_undo",&request);
        assert!(undo.is_err());
        assert!(undo.unwrap_err().contains("Pending undo recovery"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn older_pending_recovery_blocks_newer_undo(){
        let root=std::env::temp_dir().join(format!(
            "webcodex-pending-older-backup-{}",std::process::id()
        ));
        let _=fs::remove_dir_all(&root);

        let base=root.join(".webcodex/backups");
        let older=base.join("0001");
        let newer=base.join("0002");
        fs::create_dir_all(older.join(".undo-recovery")).unwrap();
        fs::create_dir_all(&newer).unwrap();

        fs::write(root.join("example.txt"),"current contents").unwrap();
        fs::write(newer.join("example.txt"),"previous contents").unwrap();

        let manifest=json!({
            "created":[],
            "modified":["example.txt"],
            "deleted":[]
        });
        fs::write(newer.join("manifest.json"),manifest.to_string()).unwrap();

        let request=json!({"root":root.to_str().unwrap()});
        let result=handle("backup_undo",&request);

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Pending undo recovery"));
        assert_eq!(
            fs::read_to_string(root.join("example.txt")).unwrap(),
            "current contents"
        );
        assert!(newer.join("manifest.json").exists());
        assert!(older.join(".undo-recovery").exists());

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn recovered_backup_cannot_be_undone_again(){
        let root=std::env::temp_dir().join(format!(
            "webcodex-recover-consumed-{}",std::process::id()
        ));
        let _=fs::remove_dir_all(&root);
        let backup=root.join(".webcodex/backups/0001");
        let stage=backup.join(".undo-recovery");
        fs::create_dir_all(&stage).unwrap();

        fs::write(root.join("example.txt"),"interrupted undo").unwrap();
        fs::write(backup.join("example.txt"),"older backup").unwrap();
        fs::write(stage.join("old-0"),"pre-undo contents").unwrap();
        fs::write(
            backup.join("manifest.json"),
            json!({
                "created":[],
                "modified":["example.txt"],
                "deleted":[]
            }).to_string()
        ).unwrap();

        let items=vec![UndoItem{
            rel:"example.txt".into(),
            target:root.join("example.txt"),
            replacement:Some(stage.join("new-0")),
            original:Some(stage.join("old-0")),
        }];
        write_recovery_journal(&stage,&items).unwrap();

        let request=json!({"root":root.to_str().unwrap()});
        let result=handle("backup_recover",&request).unwrap();
        assert_eq!(result["recovered"][0],"example.txt");
        assert_eq!(
            fs::read_to_string(root.join("example.txt")).unwrap(),
            "pre-undo contents"
        );
        assert!(!backup.exists());
        assert!(
            !root.join(".webcodex/.consumed-backup-0001").exists(),
            "Consumed backup directory must be cleaned up after recovery"
        );

        let subsequent=handle("backup_undo",&request).unwrap();
        assert_eq!(subsequent["nothing"],true);
        assert_eq!(
            fs::read_to_string(root.join("example.txt")).unwrap(),
            "pre-undo contents"
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn staged_recovery_copy_preserves_binary_contents(){
        let root=std::env::temp_dir().join(format!(
            "webcodex-staged-copy-{}",std::process::id()
        ));
        let _=fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();

        let source=root.join("source.bin");
        let destination=root.join("snapshot.bin");
        let contents:Vec<u8>=(0..65536)
            .map(|index|(index%251) as u8)
            .collect();

        fs::write(&source,&contents).unwrap();
        copy_staged(&source,&destination).unwrap();

        assert_eq!(fs::read(&destination).unwrap(),contents);
        assert_eq!(
            fs::metadata(&destination).unwrap().len(),
            contents.len() as u64
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn recovery_journal_records_original_file_state(){
        let stage=std::env::temp_dir().join(format!(
            "webcodex-journal-test-{}",std::process::id()
        ));
        let _=fs::remove_dir_all(&stage);
        fs::create_dir_all(&stage).unwrap();

        let items=vec![
            UndoItem {
                rel:"created.txt".into(),
                target:stage.join("created.txt"),
                replacement:None,
                original:Some(stage.join("old-0")),
            },
            UndoItem {
                rel:"nested/deleted.txt".into(),
                target:stage.join("nested/deleted.txt"),
                replacement:Some(stage.join("new-1")),
                original:None,
            },
        ];

        write_recovery_journal(&stage,&items).unwrap();

        let contents=fs::read(stage.join("journal.json")).unwrap();
        let journal:Value=serde_json::from_slice(&contents).unwrap();

        assert_eq!(journal["version"],1);
        assert_eq!(journal["entries"].as_array().unwrap().len(),2);
        assert_eq!(journal["entries"][0]["rel"],"created.txt");
        assert_eq!(journal["entries"][0]["had_original"],true);
        assert_eq!(journal["entries"][0]["has_replacement"],false);
        assert_eq!(journal["entries"][1]["rel"],"nested/deleted.txt");
        assert_eq!(journal["entries"][1]["had_original"],false);
        assert_eq!(journal["entries"][1]["has_replacement"],true);

        assert!(write_recovery_journal(&stage,&items).is_err());

        fs::remove_dir_all(stage).unwrap();
    }

    #[test]
    fn pending_recovery_directory_blocks_new_undo(){
        let root=std::env::temp_dir().join(format!("webcodex-undo-pending-{}",std::process::id()));
        let _=fs::remove_dir_all(&root);
        let backup=root.join(".webcodex/backups/0001");
        let recovery=backup.join(".undo-recovery");
        fs::create_dir_all(&recovery).unwrap();

        fs::write(root.join("created.txt"),"current").unwrap();
        fs::write(recovery.join("old-0"),"preserved recovery").unwrap();

        let data=json!({
            "created":["created.txt"],
            "modified":[],
            "deleted":[]
        });

        let result=apply_restore(&root,&backup,&data,None);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Cannot initialize undo recovery"));

        assert_eq!(fs::read_to_string(root.join("created.txt")).unwrap(),"current");
        assert_eq!(fs::read_to_string(recovery.join("old-0")).unwrap(),"preserved recovery");

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn failed_undo_rolls_back_overwritten_and_removed_files(){
        let root=std::env::temp_dir().join(format!("webcodex-undo-deep-rollback-{}",std::process::id()));
        let _=fs::remove_dir_all(&root);
        let backup=root.join(".webcodex/backups/0001");
        fs::create_dir_all(&backup).unwrap();

        fs::write(root.join("created.txt"),"created current").unwrap();
        fs::write(root.join("modified.txt"),"modified current").unwrap();
        fs::write(backup.join("modified.txt"),"modified previous").unwrap();
        fs::write(backup.join("deleted.txt"),"deleted previous").unwrap();

        let data=json!({
            "created":["created.txt"],
            "modified":["modified.txt"],
            "deleted":["deleted.txt"]
        });
        fs::write(backup.join("manifest.json"),data.to_string()).unwrap();

        let result=apply_restore(&root,&backup,&data,Some(2));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("rollback completed"));

        assert_eq!(fs::read_to_string(root.join("created.txt")).unwrap(),"created current");
        assert_eq!(fs::read_to_string(root.join("modified.txt")).unwrap(),"modified current");
        assert!(!root.join("deleted.txt").exists());

        assert_eq!(fs::read_to_string(backup.join("modified.txt")).unwrap(),"modified previous");
        assert_eq!(fs::read_to_string(backup.join("deleted.txt")).unwrap(),"deleted previous");
        assert!(backup.join("manifest.json").exists());
        assert!(!backup.join(".undo-recovery").exists());

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn failed_undo_rolls_back_completed_operations(){
        let root=std::env::temp_dir().join(format!("webcodex-undo-rollback-{}",std::process::id()));
        let _=fs::remove_dir_all(&root);
        let backup=root.join(".webcodex/backups/0001");
        fs::create_dir_all(&backup).unwrap();

        fs::write(root.join("created.txt"),"keep this").unwrap();
        fs::write(root.join("modified.txt"),"current version").unwrap();
        fs::write(backup.join("modified.txt"),"previous version").unwrap();

        let data=json!({
            "created":["created.txt"],
            "modified":["modified.txt"],
            "deleted":[]
        });
        fs::write(backup.join("manifest.json"),data.to_string()).unwrap();

        let result=apply_restore(&root,&backup,&data,Some(1));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("rollback completed"));
        assert_eq!(fs::read_to_string(root.join("created.txt")).unwrap(),"keep this");
        assert_eq!(fs::read_to_string(root.join("modified.txt")).unwrap(),"current version");
        assert!(backup.join("manifest.json").exists());
        assert!(!backup.join(".undo-recovery").exists());

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn successful_undo_restores_all_manifest_categories(){
        let root=std::env::temp_dir().join(format!("webcodex-undo-success-{}",std::process::id()));
        let _=fs::remove_dir_all(&root);
        let backup=root.join(".webcodex/backups/0001");
        fs::create_dir_all(backup.join("nested")).unwrap();
        fs::write(root.join("created.txt"),"new file").unwrap();
        fs::write(root.join("modified.txt"),"current version").unwrap();
        fs::write(backup.join("modified.txt"),"previous version").unwrap();
        fs::write(backup.join("nested/deleted.txt"),"deleted contents").unwrap();

        let data=json!({
            "created":["created.txt"],
            "modified":["modified.txt"],
            "deleted":["nested/deleted.txt"]
        });
        fs::write(backup.join("manifest.json"),data.to_string()).unwrap();

        let request=json!({"root":root.to_str().unwrap()});
        let result=handle("backup_undo",&request).unwrap();

        assert_eq!(result["nothing"],false);
        assert_eq!(result["restored"].as_array().unwrap().len(),3);
        assert!(!root.join("created.txt").exists());
        assert_eq!(fs::read_to_string(root.join("modified.txt")).unwrap(),"previous version");
        assert_eq!(fs::read_to_string(root.join("nested/deleted.txt")).unwrap(),"deleted contents");
        assert!(!backup.exists());
        assert_eq!(handle("backup_undo",&request).unwrap()["nothing"],true);

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn directory_destination_prevents_partial_undo(){
        let root=std::env::temp_dir().join(format!("webcodex-undo-directory-{}",std::process::id()));
        let _=fs::remove_dir_all(&root);
        let backup=root.join(".webcodex/backups/0001");
        fs::create_dir_all(&backup).unwrap();
        fs::create_dir_all(root.join("blocked")).unwrap();
        fs::write(root.join("first.txt"),"current").unwrap();
        fs::write(backup.join("first.txt"),"previous").unwrap();
        fs::write(backup.join("blocked"),"previous blocked").unwrap();

        let manifest_data=json!({
            "created":[],
            "modified":["first.txt","blocked"],
            "deleted":[]
        });
        fs::write(backup.join("manifest.json"),manifest_data.to_string()).unwrap();

        let request=json!({"root":root.to_str().unwrap()});
        let result=handle("backup_undo",&request);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not a regular file"));
        assert_eq!(fs::read_to_string(root.join("first.txt")).unwrap(),"current");
        assert!(root.join("blocked").is_dir());
        assert!(backup.join("manifest.json").exists());

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn duplicate_manifest_entries_prevent_undo(){
        let root=std::env::temp_dir().join(format!("webcodex-duplicate-backup-{}",std::process::id()));
        let _=fs::remove_dir_all(&root);
        let backup=root.join(".webcodex/backups/0001");
        fs::create_dir_all(&backup).unwrap();
        fs::write(root.join("config.txt"),"current version").unwrap();
        fs::write(backup.join("config.txt"),"previous version").unwrap();
        let data=json!({"created":["config.txt"],"modified":["config.txt"],"deleted":[]});
        fs::write(backup.join("manifest.json"),data.to_string()).unwrap();

        let request=json!({"root":root.to_str().unwrap()});
        let result=handle("backup_undo",&request);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Duplicate backup manifest destination"));
        assert_eq!(fs::read_to_string(root.join("config.txt")).unwrap(),"current version");
        assert!(backup.join("manifest.json").exists());
        assert_eq!(fs::read_to_string(backup.join("config.txt")).unwrap(),"previous version");

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn missing_backup_file_prevents_partial_undo(){
        let root=std::env::temp_dir().join(format!("webcodex-backup-preflight-{}",std::process::id()));
        let _=fs::remove_dir_all(&root);
        let backup=root.join(".webcodex/backups/0001");
        fs::create_dir_all(&backup).unwrap();
        fs::write(root.join("created.txt"),"preserve created").unwrap();
        fs::write(root.join("modified.txt"),"preserve modified").unwrap();
        let data=json!({"created":["created.txt"],"modified":["modified.txt"],"deleted":[]});
        fs::write(backup.join("manifest.json"),data.to_string()).unwrap();

        let request=json!({"root":root.to_str().unwrap()});
        let result=handle("backup_undo",&request);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Missing backup file"));
        assert_eq!(fs::read_to_string(root.join("created.txt")).unwrap(),"preserve created");
        assert_eq!(fs::read_to_string(root.join("modified.txt")).unwrap(),"preserve modified");
        assert!(backup.join("manifest.json").exists());

        fs::remove_dir_all(root).unwrap();
    }
}
