//! Read-only local Codex JSONL discovery and transcript handoff.
//! Never reads tool output into ChatGPT; never modifies ~/.codex.
use serde_json::{json,Value};
use std::{fs,io::{BufRead,BufReader},path::{Path,PathBuf},time::SystemTime};

#[derive(Clone,Debug)]
pub struct Session {pub id:String,pub path:PathBuf,pub title:String,pub cwd:String,pub modified:SystemTime}

pub fn codex_home(override_path:Option<&str>)->PathBuf {
    if let Some(p)=override_path {return PathBuf::from(p)}
    if let Some(p)=std::env::var_os("CODEX_HOME"){return PathBuf::from(p)}
    let root=std::env::var_os("USERPROFILE").or_else(||std::env::var_os("HOME")).unwrap_or_else(||".".into());
    PathBuf::from(root).join(".codex")
}
fn files(root:&Path,out:&mut Vec<PathBuf>,depth:usize){
    if depth>9 || out.len()>=20000 {return}
    let Ok(dir)=fs::read_dir(root) else{return};
    for item in dir.flatten(){
        if out.len()>=20000 {break}
        let Ok(ty)=item.file_type() else{continue};
        if ty.is_symlink(){continue}
        let p=item.path();
        if ty.is_dir(){files(&p,out,depth+1)}
        else if ty.is_file() && p.extension().is_some_and(|s|s=="jsonl") {out.push(p)}
    }
}
fn payload_text(payload:&Value)->String {
    let mut out=String::new();
    if let Some(content)=payload.get("content").and_then(Value::as_array){
        for v in content {
            if let Some(kind)=v.get("type").and_then(Value::as_str){
                if !["input_text","output_text","text"].contains(&kind){continue}
            }
            if let Some(text)=v.get("text").and_then(Value::as_str){
                if !out.is_empty(){out.push('\n')};out.push_str(text);
            }
        }
    }else if let Some(content)=payload.get("content").and_then(Value::as_str){out.push_str(content)}
    out
}
fn text_role(record:&Value)->Option<(String,String)> {
    let kind=record.get("type")?.as_str()?;
    let p=record.get("payload")?;
    match kind {
        "response_item" if p.get("type")?.as_str()?=="message" => {
            let role=p.get("role")?.as_str()?;
            if !["assistant","user"].contains(&role){return None}
            let text=payload_text(p);
            if text.trim().is_empty(){None}else{Some((role.to_string(),text))}
        },
        "event_msg" => {
            let role=match p.get("type")?.as_str()?{"user_message"=>"user","agent_message"=>"assistant",_=>return None};
            let text=p.get("message")?.as_str()?.to_string();
            if text.trim().is_empty(){None}else{Some((role.into(),text))}
        },
        _=>None
    }
}
fn each_record<F:FnMut(&Value)>(path:&Path,limit:usize,mut f:F)->Result<(),String>{
    let file=fs::File::open(path).map_err(|e|e.to_string())?;
    let mut reader=BufReader::new(file);
    let mut line=String::new();let mut count=0usize;
    loop {
        line.clear();let n=reader.read_line(&mut line).map_err(|e|e.to_string())?;
        if n==0||count>=limit{break};count+=1;
        // An unusually large tool result must not allocate unbounded JSON memory.
        if n>2_000_000{continue}
        if let Ok(v)=serde_json::from_str::<Value>(&line){f(&v)}
    }
    Ok(())
}
fn fallback_id(path:&Path)->String {
    let stem=path.file_stem().and_then(|s|s.to_str()).unwrap_or("session");
    stem.rsplit_once("rollout-").map(|(_,s)|s).unwrap_or(stem).to_string()
}
fn preview(path:&Path)->Session {
    let mut id=fallback_id(path);let mut title=String::new();let mut cwd=String::new();
    let _=each_record(path,100,|v|{
        if v["type"]=="session_meta" {
            if let Some(s)=v["payload"]["id"].as_str(){id=s.to_string()}
            if let Some(s)=v["payload"]["cwd"].as_str(){cwd=s.to_string()}
        }
        if title.is_empty(){if let Some((role,text))=text_role(v){if role=="user" {title=text.lines().find(|s|!s.trim().is_empty()).unwrap_or("").chars().take(85).collect();}}}
    });
    let modified=fs::metadata(path).and_then(|m|m.modified()).unwrap_or(SystemTime::UNIX_EPOCH);
    if title.is_empty(){title="(untitled Codex session)".into()}
    Session{id,path:path.to_owned(),title,cwd,modified}
}
pub fn discover(home:&Path)->Vec<Session>{
    let mut paths=Vec::new();files(&home.join("sessions"),&mut paths,0);
    files(&home.join("archived_sessions"),&mut paths,0);
    // Avoid reading thousands of potentially enormous transcript files just to
    // show the picker. Sort inexpensive filesystem metadata first.
    paths.sort_by_key(|p|std::cmp::Reverse(fs::metadata(p).and_then(|m|m.modified()).unwrap_or(SystemTime::UNIX_EPOCH)));
    paths.truncate(300);
    paths.iter().map(|p|preview(p)).collect()
}
pub fn find<'a>(sessions:&'a [Session],needle:&str)->Result<&'a Session,String>{
    let matches:Vec<&Session>=sessions.iter().filter(|s|s.id==needle || s.id.starts_with(needle) || s.path.file_name().is_some_and(|p|p.to_string_lossy().contains(needle))).collect();
    match matches.len(){0=>Err(format!("No local Codex session matches {needle}")),1=>Ok(matches[0]),_=>Err(format!("Ambiguous Codex session ID: {needle} ({} matches)",matches.len()))}
}
#[derive(Debug)]pub struct Import {pub messages:Vec<(String,String)>,pub handoff:String,pub omitted:usize}
pub fn import(session:&Session)->Result<Import,String>{
    let mut responses=Vec::new();let mut events=Vec::new();
    each_record(&session.path,250_000,|record|{
        if let Some((role,text))=text_role(record){
            let entry=(role,text.chars().take(25_000).collect::<String>());
            if record["type"]=="response_item"{responses.push(entry)}else{events.push(entry)}
        }
    })?;
    // response_item messages are canonical; event_msg is often a duplicate stream.
    let messages=if responses.is_empty(){events}else{responses};
    if messages.is_empty(){return Err("Selected Codex session contains no user/assistant transcript".into())}
    let omitted=messages.len().saturating_sub(100);
    let retained:Vec<(String,String)>=messages.into_iter().rev().take(100).collect::<Vec<_>>().into_iter().rev().collect();
    let mut handoff=String::from("Imported read-only from a local Codex CLI session into WebCodex. This is a NEW ChatGPT conversation, not the original Codex execution environment. Use the current project files as authoritative; previous tool results and commands were intentionally NOT imported. Prior discussion (most recent excerpts):\n");
    let max=18_000usize;
    // Preserve the earliest user goal as an anchor when a long session exceeds
    // the limited handoff window. The actual project files remain authoritative.
    if let Some((_,goal))=retained.iter().find(|(role,_)|role=="user") {
        let intro:String=goal.chars().take(1200).collect();
        handoff.push_str(&format!("Original user goal (may have evolved): {intro}\n"));
    }
    let mut remaining=max.saturating_sub(handoff.len());
    let mut excerpts=Vec::new();
    for (role,text) in retained.iter().rev() {
        if remaining<160{break}
        let excerpt:String=text.chars().take(1800.min(remaining.saturating_sub(100))).collect();
        if excerpt.is_empty(){continue}
        let row=format!("\n{}: {}\n",role.to_uppercase(),excerpt);
        remaining=remaining.saturating_sub(row.len());excerpts.push(row);
    }
    excerpts.reverse();for row in excerpts{handoff.push_str(&row)}
    Ok(Import{messages:retained,handoff,omitted})
}
pub fn state(handoff:&str,source:&Session)->Value {
    json!({"started":false,"sent_hash":{},"sent_paths":[],"not_sent":[],"notes":[],
        "codex_import":{"id":source.id,"source_path":source.path.to_string_lossy()},"handoff":handoff})
}
#[cfg(test)]mod tests {
    use super::*;
    #[test]fn parses_only_user_and_assistant_content(){
        let u=json!({"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"Fix this"}]}});
        let a=json!({"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"Done"}]}});
        let tool=json!({"type":"response_item","payload":{"type":"function_call","name":"exec","arguments":"secret"}});
        assert_eq!(text_role(&u),Some(("user".into(),"Fix this".into())));
        assert_eq!(text_role(&a),Some(("assistant".into(),"Done".into())));
        assert!(text_role(&tool).is_none());
    }
    #[test]fn imports_session_without_tools_or_duplicate_events(){
        let root=std::env::temp_dir().join(format!("webcodex-import-test-{}",uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();let p=root.join("rollout-abc.jsonl");
        let content=[json!({"type":"session_meta","payload":{"id":"abc","cwd":"/tmp/project"}}),
            json!({"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"hello"}]}}),
            json!({"type":"event_msg","payload":{"type":"user_message","message":"hello"}}),
            json!({"type":"response_item","payload":{"type":"function_call_output","output":"PRIVATE TOOL SECRET"}}),
            json!({"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"world"}]}})]
            .iter().map(Value::to_string).collect::<Vec<_>>().join("\n");
        fs::write(&p,content).unwrap();let session=preview(&p);let loaded=import(&session).unwrap();
        assert_eq!(loaded.messages.len(),2);assert!(!loaded.handoff.contains("PRIVATE TOOL SECRET"));
        fs::remove_dir_all(root).unwrap();
    }
}
