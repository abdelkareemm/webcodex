//! Native WebCodex operations worker. Browser transport stays in Python.
//! Protocol: one JSON request per stdin line, one JSON response per stdout line.
//! No diagnostics or progress are ever written to stdout.
mod store;
mod project;
mod backup;
mod planner;
mod reply;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{io::{self, BufRead, Write}, process::{Command, Stdio}, thread, time::{Duration, Instant}};

#[derive(Debug, Deserialize)]
struct Request {
    op: String,
    #[serde(default)] text: Option<String>,
    #[serde(flatten)] extras: std::collections::HashMap<String, Value>,
    #[serde(default)] blocks: Vec<EditBlock>,
    #[serde(default)] command: String,
    #[serde(default)] root: String,
    #[serde(default = "default_timeout")] timeout: f64,
}
fn default_timeout() -> f64 { 60.0 }
#[derive(Clone, Debug, Deserialize)]
struct EditBlock { path: String, search: String, replace: String }
#[derive(Debug, Serialize)]
struct PatchResult { text: Option<String>, errors: Vec<String>, applied: usize }

fn trim_blank_edges(s: &str) -> Vec<&str> {
    let mut lines: Vec<&str> = s.split('\n').collect();
    while lines.first().is_some_and(|v| v.trim().is_empty()) { lines.remove(0); }
    while lines.last().is_some_and(|v| v.trim().is_empty()) { lines.pop(); }
    lines
}
fn indent(s: &str) -> &str { &s[..s.len() - s.trim_start().len()] }

fn locate(text: &str, search: &str) -> Result<(usize, usize, Option<(String,String)>), String> {
    let occurrences: Vec<usize> = text.match_indices(search).map(|(i,_)| i).collect();
    if occurrences.len() == 1 { let s=occurrences[0];return Ok((s,s+search.len(),None)); }
    if occurrences.len()>1 { return Err(format!("SEARCH text matches {} places; include more surrounding lines so it is unique",occurrences.len())); }
    let sl=trim_blank_edges(search);
    if sl.is_empty() { return Err("SEARCH text is empty or whitespace only".into()); }
    let tl:Vec<&str>=text.split('\n').collect();
    for stripped in [false,true] {
        if tl.len()<sl.len() { continue; }
        let hits:Vec<usize>=(0..=tl.len()-sl.len()).filter(|&i| {
            (0..sl.len()).all(|j| if stripped {tl[i+j].trim()==sl[j].trim()} else {tl[i+j].trim_end()==sl[j].trim_end()})
        }).collect();
        if hits.len()>1 {return Err(format!("SEARCH text (ignoring whitespace) matches {} places; make it more specific",hits.len()));}
        if let Some(&i)=hits.first() {
            let start=tl[..i].iter().map(|s|s.len()+1).sum::<usize>();
            let end=start+tl[i..i+sl.len()].iter().map(|s|s.len()+1).sum::<usize>()-1;
            let ri=if stripped && indent(sl[0]) != indent(tl[i]) {
                Some((indent(sl[0]).into(),indent(tl[i]).into()))
            } else {None};
            return Ok((start,end,ri));
        }
    }
    let prefix=sl.first().unwrap().trim().chars().take(80).collect::<String>();
    Err(format!("SEARCH text not found in file (it began with: {prefix:?})"))
}
fn apply(text:Option<String>, blocks:&[EditBlock])->PatchResult {
    let mut current=text.clone();
    let mut errors=Vec::new();
    let mut applied=0;
    for b in blocks {
        if b.search.trim().is_empty() {
            let mut replacement=b.replace.clone();
            if !replacement.is_empty() && !replacement.ends_with('\n') {replacement.push('\n');}
            current=Some(replacement); applied+=1; continue;
        }
        let Some(ref existing)=current else {
            errors.push(format!("{}: file does not exist; to create it, leave SEARCH empty",b.path));continue;
        };
        match locate(existing,&b.search) {
            Ok((start,end,ri))=>{
                let replacement=if let Some((from,to))=ri {
                    b.replace.split('\n').map(|line|{
                        if !line.trim().is_empty() && line.starts_with(&from) {format!("{}{}",to,&line[from.len()..])} else {line.to_string()}
                    }).collect::<Vec<_>>().join("\n")
                } else {b.replace.clone()};
                let mut next=existing.clone(); next.replace_range(start..end,&replacement);
                current=Some(next); applied+=1;
            },
            Err(msg)=>errors.push(format!("{}: {}",b.path,msg)),
        }
    }
    if !errors.is_empty() {
        return PatchResult{text,errors,applied:0};
    }
    PatchResult{text:current,errors,applied}
}
fn run_command(root:&str, cmd:&str, timeout:f64)->Value {
    if !timeout.is_finite() || timeout<=0.0 || timeout>86400.0 { return json!({"error":"invalid timeout"}); }
    let mut command=if cfg!(windows) {let mut c=Command::new("cmd");c.args(["/D","/S","/C",cmd]);c}
        else {let mut c=Command::new("sh");c.args(["-c",cmd]);c};
    command.current_dir(root).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child=match command.spawn() {Ok(c)=>c,Err(e)=>return json!({"error":e.to_string()})};
    let start=Instant::now();
    // Drain both pipes concurrently so large command output can't deadlock the process.
    let out=child.stdout.take().unwrap();
    let err=child.stderr.take().unwrap();
    let out_thread=thread::spawn(move||{let mut b=Vec::new();let mut r=out;io::Read::read_to_end(&mut r,&mut b).map(|_|b)});
    let err_thread=thread::spawn(move||{let mut b=Vec::new();let mut r=err;io::Read::read_to_end(&mut r,&mut b).map(|_|b)});
    let mut timed_out=false;
    let code=loop {
        match child.try_wait() {
            Ok(Some(status))=>break status.code().unwrap_or(-1),
            Ok(None) if start.elapsed().as_secs_f64()>=timeout => {
                timed_out=true;let _=child.kill();let _=child.wait();break -1;
            },
            Ok(None)=>thread::sleep(Duration::from_millis(30)),
            Err(_)=>{let _=child.kill();let _=child.wait();break -1;}
        }
    };
    let o=out_thread.join().ok().and_then(Result::ok).unwrap_or_default();
    let e=err_thread.join().ok().and_then(Result::ok).unwrap_or_default();
    if timed_out { return json!({"code":-1,"output":format!("(timed out after {timeout}s)")}); }
    let mut combined=String::from_utf8_lossy(&o).to_string();combined.push_str(&String::from_utf8_lossy(&e));
    json!({"code":code,"output":combined})
}
fn respond(req:Request, store_cache:&mut std::collections::HashMap<String, store::Store>)->Value {
    let mut full=serde_json::Map::new();
    for (key,val) in &req.extras {full.insert(key.clone(),val.clone());}
    full.insert("root".into(),Value::String(req.root.clone()));
    let data=Value::Object(full);
    if req.op.starts_with("store_") {
        let Some(db)=data["db_path"].as_str() else{return json!({"error":"missing db_path"})};
        if !store_cache.contains_key(db) {
            match store::Store::open(std::path::Path::new(db)) {
                Ok(store)=>{store_cache.insert(db.to_string(),store);},
                Err(e)=>return json!({"error":e}),
            }
        }
        return match store_cache.get_mut(db).unwrap().handle(&req.op,&data) {Ok(v)=>v,Err(e)=>json!({"error":e})};
    }
    if req.op.starts_with("project_") {return match project::handle(&req.op,&data) {Ok(v)=>v,Err(e)=>json!({"error":e})};}
    if req.op.starts_with("backup_") {return match backup::handle(&req.op,&data) {Ok(v)=>v,Err(e)=>json!({"error":e})};}
    if req.op=="parse_reply" {return reply::parse(&req.text.unwrap_or_default());}
    if req.op=="plan_message" {return match planner::handle(&data) {Ok(v)=>v,Err(e)=>json!({"error":e})};}

    match req.op.as_str() {
        "ping"=>json!({"version":"1.2.0","worker":"webcodex-core"}),
        "patch"=>serde_json::to_value(apply(req.text,&req.blocks)).unwrap_or(Value::Null),
        "run"=>run_command(&req.root,&req.command,req.timeout),
        _=>json!({"error":format!("unknown operation: {}",req.op)}),
    }
}
fn main() {
    let stdin=io::stdin();let mut stdout=io::stdout().lock();
    let mut store_cache=std::collections::HashMap::new();
    for line in stdin.lock().lines() {
        let Ok(line)=line else {break};
        let response=match serde_json::from_str::<Request>(&line) {
            Ok(req)=>respond(req,&mut store_cache),Err(e)=>json!({"error":format!("invalid request: {e}")}),
        };
        if serde_json::to_writer(&mut stdout,&response).is_err() {break;}
        if stdout.write_all(b"\n").is_err() || stdout.flush().is_err() {break;}
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn block(s:&str,r:&str)->EditBlock {EditBlock{path:"a.txt".into(),search:s.into(),replace:r.into()}}
    #[test] fn exact_patch(){let r=apply(Some("abc\ndef\n".into()),&[block("def","XYZ")]);assert_eq!(r.text.as_deref(),Some("abc\nXYZ\n"));assert_eq!(r.applied,1);}
    #[test] fn create_file(){let r=apply(None,&[block("","hello")]);assert_eq!(r.text.as_deref(),Some("hello\n"));}
    #[test] fn reject_ambiguous(){let r=apply(Some("x\nx".into()),&[block("x","y")]);assert_eq!(r.applied,0);assert_eq!(r.errors.len(),1);}
    #[test] fn reject_missing(){let r=apply(None,&[block("x","y")]);assert_eq!(r.applied,0);}
    #[test] fn failed_batch_preserves_original(){
        let original="first\nsecond\n".to_string();
        let result=apply(Some(original.clone()),&[
            block("first","modified"),
            block("does-not-exist","unexpected")
        ]);
        assert_eq!(result.applied,0);
        assert_eq!(result.text,Some(original));
        assert_eq!(result.errors.len(),1);
    }
    #[test] fn failed_batch_does_not_create_partial_file(){
        let result=apply(None,&[
            block("","new content"),
            block("does-not-exist","unexpected")
        ]);
        assert_eq!(result.applied,0);
        assert_eq!(result.text,None);
        assert_eq!(result.errors.len(),1);
    }
    #[test] fn whitespace_fallback(){let r=apply(Some("  alpha  \n  beta\n".into()),&[block("alpha\nbeta","gamma")]);assert_eq!(r.applied,1);}
}
