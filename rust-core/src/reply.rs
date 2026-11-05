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
    while i<lines.len(){
        let line=lines[i].trim();
        let Some(path)=line.strip_prefix("FILE:") else {i+=1;continue};
        if i+1>=lines.len()||!marker(lines[i+1],'<',"SEARCH"){i+=1;continue}
        let mut separator=i+2;
        while separator<lines.len()&&!marker(lines[separator],'=',""){separator+=1}
        if separator==lines.len(){i+=1;continue}
        let mut ending=separator+1;
        while ending<lines.len()&&!marker(lines[ending],'>',"REPLACE"){ending+=1}
        if ending==lines.len(){i+=1;continue}
        let search=drop_newline(lines[i+2..separator].concat());
        let replace=drop_newline(lines[separator+1..ending].concat());
        edits.push(json!({"path":clean(path),"search":search,"replace":replace}));
        for j in i..=ending{masked.insert(j);}i=ending+1;
    }
    let mut deletes=Vec::new();let mut needs=Vec::new();let mut runs=Vec::new();
    for (i,line) in lines.iter().enumerate(){if masked.contains(&i){continue}
        let line=line.trim();
        if let Some(rest)=line.strip_prefix("DELETE:"){deletes.push(clean(rest));}
        if let Some(rest)=line.strip_prefix("NEED:") {for part in rest.split(|c:char|c.is_whitespace()||c==',') {let part=clean(part);if !part.is_empty(){needs.push(part)}}}
        if let Some(rest)=line.strip_prefix("RUN:"){let cmd=rest.trim().trim_matches('`').trim();if !cmd.is_empty(){runs.push(cmd.to_string())}}
    }
    json!({"edits":edits,"deletes":deletes,"needs":needs,"runs":runs})
}
#[cfg(test)]mod tests{use super::*;
    #[test]fn edit_and_actions(){let src="````\nFILE: a.txt\n<<<<<<< SEARCH\na\n=======\nb\n>>>>>>> REPLACE\n````\nNEED: x.py, y.rs\nRUN: echo hi\nDELETE: old.txt\n";
        let parsed=parse(src);assert_eq!(parsed["edits"][0]["path"],"a.txt");assert_eq!(parsed["edits"][0]["search"],"a");assert_eq!(parsed["edits"][0]["replace"],"b");assert_eq!(parsed["needs"].as_array().unwrap().len(),2);assert_eq!(parsed["runs"][0],"echo hi");
    }
}
