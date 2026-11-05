//! Parse WebCodex fenced edit directives and NEED/RUN/DELETE instructions.
//! Keeps the wire format compatible with the Python reply parser.
use serde_json::{json,Value};
use std::collections::HashSet;
fn clean(p:&str)->String{
    let mut v=p.trim().trim_matches(|c|matches!(c,'`'|'*'|'"'|'\''|' '| '\t')).replace('\\',"/");
    while let Some(rest)=v.strip_prefix("./"){v=rest.to_string();}v
}
fn drop_newline(mut v:String)->String {if v.ends_with('\n'){v.pop();}v}
fn marker(line:&str,character:char,word:&str)->bool{
    let trim=line.trim();let count=trim.chars().take_while(|c|*c==character).count();
    (5..=9).contains(&count)&&trim[count..].trim()==word
}
pub fn parse(reply:&str)->Value {
    // Use split_inclusive so SEARCH and REPLACE payloads retain exact internal newlines.
    let lines=reply.split_inclusive('\n').collect::<Vec<_>>();
    let mut edits=Vec::new();let mut masked=HashSet::new();let mut i=0;
    let mut errors=Vec::<String>::new();
    while i<lines.len(){
        let line=lines[i].trim();
        let Some(path)=line.strip_prefix("FILE:") else {i+=1;continue};
        let path=clean(path);
        let valid_open=i>0 && {
            let fence=lines[i-1].trim();
            fence=="````" || fence=="````text"
        };
        if !valid_open || path.is_empty() || i+1>=lines.len()
            || !marker(lines[i+1],'<',"SEARCH"){
            errors.push(format!("Invalid or unfenced edit block: {path}"));
            i+=1;continue;
        }
        let mut separator=i+2;
        while separator<lines.len()
            && !marker(lines[separator],'=',"")
            && lines[separator].trim()!="````" {
            separator+=1;
        }
        if separator==lines.len() || lines[separator].trim()=="````" {
            errors.push(format!("Missing edit separator: {path}"));
            i+=1;continue;
        }
        let mut ending=separator+1;
        while ending<lines.len()
            && !marker(lines[ending],'>',"REPLACE")
            && lines[ending].trim()!="````" {
            ending+=1;
        }
        if ending==lines.len() || lines[ending].trim()=="````"
            || ending+1>=lines.len()
            || lines[ending+1].trim()!="````" {
            errors.push(format!("Incomplete replacement block: {path}"));
            i+=1;continue;
        }
        let search=drop_newline(lines[i+2..separator].concat());
        let replace=drop_newline(lines[separator+1..ending].concat());
        if replace.lines().any(|line|line.trim_start().starts_with("FILE:"))
            || replace.contains("<<<<<<< SEARCH")
            || replace.contains(">>>>>>> REPLACE") {
            errors.push(format!("Nested edit instructions rejected: {path}"));
            i=ending+2;continue;
        }
        edits.push(json!({"path":path,"search":search,"replace":replace}));
        for j in i-1..=ending+1 {masked.insert(j);}
        i=ending+2;
    }
    let mut deletes=Vec::new();let mut needs=Vec::new();let mut runs=Vec::new();
    for (i,line) in lines.iter().enumerate(){if masked.contains(&i){continue}
        let line=line.trim();
        if let Some(rest)=line.strip_prefix("DELETE:"){deletes.push(clean(rest));}
        if let Some(rest)=line.strip_prefix("NEED:") {for part in rest.split(|c:char|c.is_whitespace()||c==',') {let part=clean(part);if !part.is_empty(){needs.push(part)}}}
        if let Some(rest)=line.strip_prefix("RUN:"){let cmd=rest.trim().trim_matches('`').trim();if !cmd.is_empty(){runs.push(cmd.to_string())}}
    }
    json!({"edits":edits,"deletes":deletes,"needs":needs,"runs":runs,"errors":errors})
}
#[cfg(test)]mod tests{use super::*;
    #[test]fn edit_and_actions(){let src="````\nFILE: a.txt\n<<<<<<< SEARCH\na\n=======\nb\n>>>>>>> REPLACE\n````\nNEED: x.py, y.rs\nRUN: echo hi\nDELETE: old.txt\n";
        let parsed=parse(src);assert_eq!(parsed["edits"][0]["path"],"a.txt");assert_eq!(parsed["edits"][0]["search"],"a");assert_eq!(parsed["edits"][0]["replace"],"b");assert_eq!(parsed["needs"].as_array().unwrap().len(),2);assert_eq!(parsed["runs"][0],"echo hi");
        assert!(parsed["errors"].as_array().unwrap().is_empty());
    }
    #[test]fn rejects_unfenced_edit(){
        let src="FILE: a.txt\n<<<<<<< SEARCH\nold\n=======\nnew\n>>>>>>> REPLACE\n";
        let result=parse(src);
        assert!(result["edits"].as_array().unwrap().is_empty());
        assert!(!result["errors"].as_array().unwrap().is_empty());
    }
    #[test]fn rejects_incomplete_edit(){
        let src="````text\nFILE: a.txt\n<<<<<<< SEARCH\nold\n=======\nnew";
        let result=parse(src);
        assert!(result["edits"].as_array().unwrap().is_empty());
        assert!(!result["errors"].as_array().unwrap().is_empty());
    }
    #[test]fn rejects_nested_edit(){
        let src="````text\nFILE: a.txt\n<<<<<<< SEARCH\nold\n=======\nFILE: b.txt\n<<<<<<< SEARCH\nnew\n=======\ncorrupt\n>>>>>>> REPLACE\n````\n";
        let result=parse(src);
        assert!(result["edits"].as_array().unwrap().is_empty());
        assert!(!result["errors"].as_array().unwrap().is_empty());
    }
}
