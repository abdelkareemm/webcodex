use std::{fs,path::Path};

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
enum Level {
    Patch,
    Minor,
    Major,
}

fn classify(reply:&str)->Level {
    let mut level=Level::Patch;
    let mut fence:Option<(char,usize)>=None;

    for line in reply.lines(){
        let trimmed=line.trim();
        let marker=trimmed.chars().next().unwrap_or(' ');
        let count=if marker=='`'||marker=='~' {
            trimmed.chars().take_while(|c|*c==marker).count()
        }else{0};

        if let Some((opened,length))=fence {
            if marker==opened && count>=length && trimmed[count..].trim().is_empty(){
                fence=None;
            }
            continue;
        }
        if count>=3 {
            fence=Some((marker,count));
            continue;
        }

        match trimmed {
            "VERSION: major"=>level=Level::Major,
            "VERSION: minor" if level!=Level::Major=>level=Level::Minor,
            _=>{}
        }
    }
    level
}

fn package_version(contents:&str)->Result<String,String>{
    let mut package=false;
    for line in contents.lines(){
        let line=line.trim();
        if line=="[package]" || line=="[project]"{
            package=true;
            continue;
        }
        if package && line.starts_with('['){break;}
        if package {
            if let Some(version)=line.strip_prefix("version = "){
                return Ok(version.trim().trim_matches('"').to_string());
            }
        }
    }
    Err("Package version not found".into())
}

fn increment(version:&str,level:Level)->Result<String,String>{
    let parts=version.split('.')
        .map(str::parse::<u64>)
        .collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())?;
    if parts.len()!=3{return Err("Expected major.minor.patch version".into());}

    let (major,minor,patch)=(parts[0],parts[1],parts[2]);
    let (major,minor,patch)=match level {
        Level::Patch=>(major,minor,patch.checked_add(1).ok_or("Patch overflow")?),
        Level::Minor=>(major,minor.checked_add(1).ok_or("Minor overflow")?,0),
        Level::Major=>(major.checked_add(1).ok_or("Major overflow")?,0,0),
    };
    Ok(format!("{major}.{minor}.{patch}"))
}

pub fn bump(root:&Path,reply:&str)->Result<Option<String>,String>{
    let core=root.join("rust-core/Cargo.toml");
    let tui=root.join("rust-tui/Cargo.toml");
    let native=root.join("rust-core/src/bin/webcodex-native.rs");

    if !core.is_file() || !tui.is_file() || !native.is_file(){
        return Ok(None);
    }

    let original_core=fs::read_to_string(&core).map_err(|e|e.to_string())?;
    let original_tui=fs::read_to_string(&tui).map_err(|e|e.to_string())?;
    let python=root.join("pyproject.toml");
    let init=root.join("webcodex/__init__.py");
    let core_lock=root.join("rust-core/Cargo.lock");
    let tui_lock=root.join("rust-tui/Cargo.lock");
    let extra_paths=[&python,&init,&core_lock,&tui_lock];
    let mut extras=Vec::new();
    for path in extra_paths {
        if path.is_file(){
            extras.push(Some(fs::read_to_string(path).map_err(|e|e.to_string())?));
        }else{
            extras.push(None);
        }
    }
    let current=package_version(&original_core)?;

    if package_version(&original_tui)?!=current{
        return Err("Core and TUI versions differ; automatic bump refused".into());
    }
    if let Some(source)=&extras[0] {
        if package_version(source)?!=current {
            return Err("Python and Rust package versions differ".into());
        }
    }
    if let Some(source)=&extras[1] {
        if !source.contains(&format!("__version__ = \"{current}\"")) {
            return Err("Python __version__ differs from Cargo".into());
        }
    }
    for (index,name) in [(2,"webcodex-core"),(3,"webcodex-tui")] {
        if let Some(source)=&extras[index] {
            let entry=format!("[[package]]\nname = \"{name}\"\nversion = \"{current}\"");
            if !source.replace("\r\n","\n").contains(&entry) {
                return Err(format!("{name} lockfile version differs from Cargo"));
            }
        }
    }

    let next=increment(&current,classify(reply))?;
    let old=format!("version = \"{current}\"");
    let new=format!("version = \"{next}\"");

    if !original_core.contains(&old) || !original_tui.contains(&old){
        return Err("Cannot locate version declarations".into());
    }

    let updated_core=original_core.replacen(&old,&new,1);
    let updated_tui=original_tui.replacen(&old,&new,1);
    let mut changes=vec![
        (core.clone(),original_core.clone(),updated_core),
        (tui.clone(),original_tui.clone(),updated_tui),
    ];

    for (index,path) in extra_paths.iter().enumerate(){
        let Some(original)=extras[index].as_ref() else {continue};
        let (before,after)=match index {
            0=>(old.clone(),new.clone()),
            1=>(
                format!("__version__ = \"{current}\""),
                format!("__version__ = \"{next}\""),
            ),
            2|3=>{
                let name=if index==2 {"webcodex-core"}else{"webcodex-tui"};
                (
                    format!("[[package]]\nname = \"{name}\"\n{old}"),
                    format!("[[package]]\nname = \"{name}\"\n{new}"),
                )
            },
            _=>unreachable!(),
        };
        let normalized=original.replace("\r\n","\n");
        if normalized.matches(&before).count()!=1{
            return Err(format!("Invalid version declaration in {}",path.display()));
        }
        let updated=normalized.replacen(&before,&after,1);
        let updated=if original.contains("\r\n"){
            updated.replace('\n',"\r\n")
        }else{updated};
        changes.push(((*path).clone(),original.clone(),updated));
    }

    for index in 0..changes.len(){
        let (path,_,updated)=&changes[index];
        if let Err(error)=fs::write(path,updated){
            let mut failures=Vec::new();
            for (previous,original,_) in changes[..=index].iter().rev(){
                if let Err(rollback)=fs::write(previous,original){
                    failures.push(format!("{}: {rollback}",previous.display()));
                }
            }
            return Err(if failures.is_empty(){
                format!("Version update failed: {error}; files restored")
            }else{
                format!("Version update failed: {error}; rollback failed: {}",failures.join("; "))
            });
        }
    }

    Ok(Some(format!("{current} -> {next}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_increments(){
        assert_eq!(increment("1.3.0",Level::Patch).unwrap(),"1.3.1");
        assert_eq!(increment("1.3.0",Level::Minor).unwrap(),"1.4.0");
        assert_eq!(increment("1.3.0",Level::Major).unwrap(),"2.0.0");
    }

    #[test]
    fn bump_keeps_both_crates_synchronized(){
        let root=std::env::temp_dir().join(format!(
            "webcodex-versioning-test-{}",std::process::id()
        ));
        let _=fs::remove_dir_all(&root);

        let core=root.join("rust-core");
        let tui=root.join("rust-tui");
        let native=core.join("src/bin/webcodex-native.rs");

        fs::create_dir_all(native.parent().unwrap()).unwrap();
        fs::create_dir_all(&tui).unwrap();
        fs::write(&native,"// test fixture").unwrap();

        fs::create_dir_all(root.join("webcodex")).unwrap();
        for (directory,name) in [(&core,"webcodex-core"),(&tui,"webcodex-tui")] {
            fs::write(
                directory.join("Cargo.toml"),
                format!("[package]\nname = \"{name}\"\nversion = \"1.3.0\"\n"),
            ).unwrap();
            fs::write(
                directory.join("Cargo.lock"),
                format!("[[package]]\nname = \"{name}\"\nversion = \"1.3.0\"\n"),
            ).unwrap();
        }
        fs::write(root.join("pyproject.toml"),
            "[project]\nversion = \"1.3.0\"\n").unwrap();
        fs::write(root.join("webcodex/__init__.py"),
            "__version__ = \"1.3.0\"\n").unwrap();

        assert_eq!(
            bump(&root,"VERSION: patch").unwrap().as_deref(),
            Some("1.3.0 -> 1.3.1")
        );
        assert_eq!(
            bump(&root,"VERSION: minor").unwrap().as_deref(),
            Some("1.3.1 -> 1.4.0")
        );
        assert_eq!(
            bump(&root,"VERSION: major").unwrap().as_deref(),
            Some("1.4.0 -> 2.0.0")
        );

        for directory in [&core,&tui] {
            let contents=fs::read_to_string(directory.join("Cargo.toml")).unwrap();
            assert_eq!(package_version(&contents).unwrap(),"2.0.0");
        }

        for path in [
            "pyproject.toml",
            "rust-core/Cargo.lock",
            "rust-tui/Cargo.lock",
        ] {
            assert!(fs::read_to_string(root.join(path)).unwrap()
                .contains("version = \"2.0.0\""));
        }
        assert_eq!(
            fs::read_to_string(root.join("webcodex/__init__.py")).unwrap().trim(),
            "__version__ = \"2.0.0\""
        );

        let _=fs::remove_dir_all(&root);
    }

    #[test]
    fn classification_ignores_fenced_text(){
        let reply=String::from("```")+"\nVERSION: major\n"
            +"```\nVERSION: minor";
        assert_eq!(classify(&reply),Level::Minor);
        assert_eq!(classify("DONE: complete"),Level::Patch);
    }
}
