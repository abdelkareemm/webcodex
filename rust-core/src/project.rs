//! Native project scanning, including gitignore, secret-file filtering, and file budgets.
use ignore::WalkBuilder;
use serde_json::{json,Value};
use sha1::{Sha1,Digest};
use std::{collections::{BTreeMap,HashSet},fs,path::{Path,PathBuf}};
const DIRS:&[&str]=&[".git",".hg",".svn","node_modules","__pycache__",".venv","venv","env",".idea",".vscode","dist","build",".next",".nuxt",".cache","target",".webcodex",".pytest_cache",".mypy_cache",".ruff_cache",".gradle",".tox","coverage",".parcel-cache","AppData","Application Data","$RECYCLE.BIN","System Volume Information","Library",".npm",".nuget",".cargo",".rustup",".m2","Pods","site-packages","bower_components"];
const EXT:&[&str]=&[".png",".jpg",".jpeg",".gif",".webp",".bmp",".ico",".tiff",".svgz",".heic",".mp3",".wav",".flac",".ogg",".m4a",".mp4",".mov",".avi",".mkv",".webm",".zip",".tar",".gz",".bz2",".7z",".rar",".xz",".jar",".war",".exe",".dll",".so",".dylib",".bin",".o",".a",".class",".pyc",".pyd",".pdf",".doc",".docx",".xls",".xlsx",".ppt",".pptx",".ttf",".otf",".woff",".woff2",".eot",".flp",".fst",".db",".sqlite",".sqlite3",".psd",".blend",".fbx",".obj",".min.js",".map"];
const NAMES:&[&str]=&["package-lock.json","yarn.lock","pnpm-lock.yaml","poetry.lock","Cargo.lock","composer.lock","Gemfile.lock",".DS_Store","Thumbs.db"];
const SECRETS:&[&str]=&[".npmrc",".pypirc",".netrc",".git-credentials","credentials.json","service-account.json","secrets.json","secrets.yaml","secrets.yml","id_rsa","id_dsa","id_ecdsa","id_ed25519"];
const KEY_FILES:&[&str]=&["readme","pyproject.toml","package.json","cargo.toml","go.mod","requirements.txt","makefile","tsconfig.json","pom.xml","build.gradle","manifest.json"];
fn secret(name:&str)->bool {let n=name.to_lowercase();SECRETS.contains(&n.as_str())||[".pem",".key",".p12",".pfx",".keystore",".jks",".ppk"].iter().any(|s|n.ends_with(s)) || ((n==".env"||n.starts_with(".env.")) && ![".example",".sample",".template",".dist"].iter().any(|s|n.ends_with(s)))}
fn relative(root:&Path,path:&Path)->String {path.strip_prefix(root).unwrap_or(path).to_string_lossy().replace('\\',"/")}
fn text(path:&Path)->Option<String> {let bytes=fs::read(path).ok()?;if bytes.iter().take(8192).any(|b|*b==0){return None};String::from_utf8(bytes).ok().map(|s|s.replace("\r\n","\n"))}
fn scan(root:&Path,max_files:usize)->Result<BTreeMap<String,Value>,String>{
    let mut result=BTreeMap::new();
    let mut wb=WalkBuilder::new(root);wb.hidden(false).require_git(false).git_ignore(true).git_global(false).git_exclude(false).parents(false).add_custom_ignore_filename(".webcodexignore");
    wb.filter_entry(|e|e.depth()==0||!e.file_type().is_some_and(|t|t.is_dir())||!DIRS.contains(&e.file_name().to_string_lossy().as_ref()));
    for entry in wb.build() {
        let e=match entry{Ok(e)=>e,Err(_)=>continue};if !e.file_type().is_some_and(|t|t.is_file()) {continue}
        let file=e.file_name().to_string_lossy().to_string();if NAMES.contains(&file.as_str())||secret(&file){continue}
        if result.len()>=max_files{return Err(format!("More than {max_files} files in {}",root.display()))}
        let rel=relative(root,e.path());let size=e.metadata().map(|m|m.len()).unwrap_or(0);
        let lower=rel.to_lowercase();
        let note=if EXT.iter().any(|ext|lower.ends_with(ext)){"binary"}else if size>300_000{"too large"}else{""};
        let content=if note.is_empty(){text(e.path())}else{None};
        let note=if note.is_empty()&&content.is_none(){"binary"}else{note};
        let hash=content.as_ref().map(|c|format!("{:x}",Sha1::digest(c.as_bytes())));
        let chars=content.as_ref().map(|c|c.chars().count()).unwrap_or(0);
        result.insert(rel.clone(),json!({"rel":rel,"size":size,"hash":hash,"chars":chars,"note":note}));
    }
    Ok(result)
}
fn choose(entries:&BTreeMap<String,Value>,candidates:&[String],budget:usize,max_chars:usize,pinned:&[String])->(Vec<String>,Vec<String>){
    let pin:HashSet<_>=pinned.iter().collect();let mut candidate:Vec<String>=candidates.iter().filter(|c|entries.contains_key(*c)).cloned().collect();
    candidate.sort_by_key(|rel|{let e=&entries[rel];let name=Path::new(rel).file_name().unwrap_or_default().to_string_lossy().to_lowercase();(!pin.contains(rel),!KEY_FILES.iter().any(|k|name.starts_with(k)),e["chars"].as_u64().unwrap_or(0))});
    let(mut included,mut omitted,mut used)=(Vec::new(),Vec::new(),0usize);let mut seen=HashSet::new();
    for rel in candidate {if !seen.insert(rel.clone())||entries[&rel]["hash"].is_null(){continue};let chars=entries[&rel]["chars"].as_u64().unwrap_or(0) as usize;
        if pin.contains(&rel)|| (chars<=max_chars&&used.saturating_add(chars)<=budget){used=used.saturating_add(chars);included.push(rel)}else{omitted.push(rel)}
    }
    included.sort();omitted.sort();(included,omitted)
}
pub fn handle(op:&str,req:&Value)->Result<Value,String> {
    let root=PathBuf::from(req["root"].as_str().ok_or("missing project root")?);
    match op {
      "project_scan"=>Ok(json!({"entries":scan(&root,req["max_files"].as_u64().unwrap_or(4000) as usize)?})),
      "project_read"=>{
        let rel=req["rel"].as_str().ok_or("missing rel")?;
        let relative=Path::new(rel);
        if relative.is_absolute() || relative.components().any(|c|matches!(c,std::path::Component::ParentDir|std::path::Component::RootDir|std::path::Component::Prefix(_))) {return Err("invalid relative path".into())}
        let absolute=std::fs::canonicalize(root.join(relative)).map_err(|e|e.to_string())?;
        if !absolute.starts_with(std::fs::canonicalize(&root).map_err(|e|e.to_string())?) {return Err("file escapes project".into())}
        Ok(json!({"text":text(&absolute).unwrap_or_default()}))
      },
      "project_pick"=>{
        let entries:BTreeMap<String,Value>=serde_json::from_value(req["entries"].clone()).map_err(|e|e.to_string())?;
        let strs=|k:&str|req[k].as_array().map(|a|a.iter().filter_map(Value::as_str).map(String::from).collect::<Vec<_>>()).unwrap_or_default();
        let (included,omitted)=choose(&entries,&strs("candidates"),req["budget"].as_u64().unwrap_or(60000) as usize,req["max_file_chars"].as_u64().unwrap_or(20000) as usize,&strs("pinned"));
        Ok(json!({"included":included,"omitted":omitted}))
      },
      _=>Err(format!("unknown project operation: {op}"))
    }
}
#[cfg(test)] mod tests{use super::*;#[test]fn ignores_secrets(){assert!(secret(".env"));assert!(!secret(".env.example"));assert!(secret("id_rsa"));}}
