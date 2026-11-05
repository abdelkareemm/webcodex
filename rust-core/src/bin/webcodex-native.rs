//! Rust-primary WebCodex command runner. Python is used only as a Playwright
//! browser subprocess; session, file state, shell execution and approvals are
//! coordinated here. The Python CLI remains a fallback when this binary is not built.
use serde_json::{json,Value};
#[path="webcodex-native/codex_import.rs"] mod codex_import;
#[path="webcodex-native/picker.rs"] mod picker;
use std::{collections::{BTreeMap},env,fs,io::{self,BufRead,BufReader,Write,IsTerminal},net::{TcpListener,TcpStream},path::{Component,Path,PathBuf},process::{Child,ChildStdin,ChildStdout,Command,Stdio},sync::mpsc,time::Duration};

const PROTOCOL:&str=include_str!("../../assets/protocol.txt");
const REMINDER:&str=include_str!("../../assets/reminder.txt");
const HELP:&str="/new  start a new chat; /chat show chat URL; /undo undo last edit set; /yes toggle auto-apply; /run toggle shell commands; /files show visible files; /status show settings; /help; /quit";

fn err<T:ToString>(value:T)->String{value.to_string()}
fn strings(v:&Value)->Vec<String>{v.as_array().map(|a|a.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default()}
fn home()->PathBuf {
    if let Ok(p)=env::var("WEBCODEX_HOME"){return PathBuf::from(p)}
    let home=env::var("USERPROFILE").or_else(|_|env::var("HOME")).unwrap_or_else(|_|".".to_string());
    PathBuf::from(home).join(".webcodex")
}
fn executable(name:&str, folder:&str)->PathBuf {
    let root=Path::new(env!("CARGO_MANIFEST_DIR"));
    root.join(folder).join(name)
}
fn core_executable()->PathBuf {
    let name=if cfg!(windows){"webcodex-core.exe"}else{"webcodex-core"};
    env::var_os("WEBCODEX_RUST_CORE").map(PathBuf::from).unwrap_or_else(||{
        env::current_exe().ok().and_then(|p|p.parent().map(|d|d.join(name))).unwrap_or_else(||executable(name,"target/release"))
    })
}
fn tui_executable()->PathBuf {
    let name=if cfg!(windows){"webcodex-tui.exe"}else{"webcodex-tui"};
    if let Some(path)=env::var_os("WEBCODEX_RUST_TUI"){return PathBuf::from(path);}
    if let Some(path)=env::current_exe().ok().and_then(|p|p.parent().map(|d|d.join(name))){
        if path.is_file(){return path;}
    }
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../rust-tui/target/release").join(name)
}
struct JsonChild {child:Child,stdin:ChildStdin,stdout:BufReader<ChildStdout>}
impl JsonChild {
    fn spawn(mut command:Command)->Result<Self,String> {
        let mut child=command.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().map_err(err)?;
        let stdin=child.stdin.take().ok_or("missing process stdin")?;
        let stdout=BufReader::new(child.stdout.take().ok_or("missing process stdout")?);
        Ok(Self{child,stdin,stdout})
    }
    fn send(&mut self,request:&Value)->Result<(),String>{
        serde_json::to_writer(&mut self.stdin,request).map_err(err)?;
        self.stdin.write_all(b"\n").map_err(err)?;self.stdin.flush().map_err(err)
    }
    fn receive(&mut self)->Result<Value,String>{
        let mut line=String::new();if self.stdout.read_line(&mut line).map_err(err)?==0{return Err("subprocess closed IPC stream".into())}
        serde_json::from_str(&line).map_err(err)
    }
    fn call(&mut self,req:Value)->Result<Value,String>{
        self.send(&req)?;let response=self.receive()?;
        if let Some(s)=response["error"].as_str(){Err(s.to_string())}else{Ok(response)}
    }
}
impl Drop for JsonChild {fn drop(&mut self){let _=self.child.kill();let _=self.child.wait();}}

struct Ui {
    socket:Option<TcpStream>,reader:Option<BufReader<TcpStream>>,process:Option<Child>,approval_all:bool,
}
impl Ui {
    fn new(root:&Path,plain:bool)->Self {
        if !plain && io::stdin().is_terminal() && io::stdout().is_terminal() && tui_executable().is_file(){
            if let Ok(listener)=TcpListener::bind("127.0.0.1:0"){
                let port=listener.local_addr().map(|a|a.port()).unwrap_or(0);
                let mut cmd=Command::new(tui_executable());let display_root=root.to_string_lossy();
                let display_root=display_root.strip_prefix(r"\\?\").unwrap_or(&display_root);
                cmd.arg(port.to_string()).arg(display_root);
                if let Ok(mut process)=cmd.spawn(){
                    // A nonblocking listening socket can produce Winsock 10035 on
                    // accepted connections. Accept on a bounded worker thread using
                    // an entirely blocking socket instead of polling with WouldBlock.
                    let (tx, rx)=mpsc::sync_channel(1);
                    std::thread::spawn(move || {let _=tx.send(listener.accept());});
                    if let Ok(Ok((socket,_)))=rx.recv_timeout(Duration::from_secs(10)) {
                        // Keep both cloned handles in blocking mode on Windows.
                        if socket.set_nonblocking(false).is_ok() {
                            let _=socket.set_nodelay(true);
                            if let Ok(copy)=socket.try_clone() {
                                let _=copy.set_nonblocking(false);
                                return Self{socket:Some(socket),reader:Some(BufReader::new(copy)),process:Some(process),approval_all:false};
                            }
                        }
                    }
                    let _=process.kill();let _=process.wait();
                }
            }
        }
        Self{socket:None,reader:None,process:None,approval_all:false}
    }
    fn event(&mut self,kind:&str,body:&str){
        if let Some(s)=self.socket.as_mut(){
            let line=json!({"kind":kind,"body":body});
            if serde_json::to_writer(&mut *s,&line).is_ok(){let _=s.write_all(b"\n");let _=s.flush();}
        }else if !body.is_empty(){
            match kind {"error"=>eprintln!("{body}"),"activity"=>println!("  {body}"),"assistant"=>println!("\n{body}\n"),"user"|"busy"=>{},_=>println!("{body}")}
        }
    }
    fn receive(&mut self)->Result<Option<String>,String>{
        if let Some(reader)=self.reader.as_mut(){
            let mut line=String::new();
            loop {
                match reader.read_line(&mut line) {
                    Ok(0)=>return Ok(None), Ok(_)=>break,
                    Err(e) if e.kind()==io::ErrorKind::WouldBlock || e.kind()==io::ErrorKind::Interrupted => {
                        std::thread::sleep(Duration::from_millis(15));
                    },
                    Err(e)=>return Err(err(e)),
                }
            }
            let v:Value=serde_json::from_str(&line).map_err(err)?;
            if v["kind"]=="exit" {return Ok(None)}
            if v["kind"]=="task" {return Ok(Some(v["body"].as_str().unwrap_or("").to_string()))}
            return Ok(Some(String::new()));
        }
        print!("\n› ");io::stdout().flush().map_err(err)?;
        let mut line=String::new();if io::stdin().read_line(&mut line).map_err(err)?==0{return Ok(None)}
        Ok(Some(line.trim().to_string()))
    }
    fn confirm(&mut self,question:&str,default_yes:bool)->Result<bool,String>{
        if self.socket.is_some(){
            self.event("approve",question);
            loop{
                let mut line=String::new();
                let Some(reader)=self.reader.as_mut() else{return Err("TUI disconnected".into())};
                match reader.read_line(&mut line) {
                    Ok(0)=>return Err("TUI disconnected".into()),
                    Ok(_)=>{},
                    Err(e) if e.kind()==io::ErrorKind::WouldBlock || e.kind()==io::ErrorKind::Interrupted => {
                        std::thread::sleep(Duration::from_millis(15));continue;
                    },
                    Err(e)=>return Err(err(e)),
                }
                let val:Value=serde_json::from_str(&line).map_err(err)?;
                if val["kind"]=="approve"{
                    if val["body"]=="a"{self.approval_all=true;}
                    return Ok(matches!(val["body"].as_str(),Some("y"|"a")))
                }
                if val["kind"]=="exit"{return Err("approval interrupted".into())}
            }
        }
        print!("{question} [{}] ",if default_yes{"Y/n"}else{"y/N"});io::stdout().flush().map_err(err)?;
        let mut line=String::new();io::stdin().read_line(&mut line).map_err(err)?;
        let answer=line.trim().to_lowercase();
        if answer=="a"{self.approval_all=true;}
        Ok(if answer.is_empty(){default_yes}else{answer=="y"||answer=="yes"||answer=="a"})
    }
}
impl Drop for Ui {
    fn drop(&mut self){
        // Ask the TUI to leave alternate-screen/raw mode before exiting.
        // Killing it first leaves an orphaned terminal frame in PowerShell.
        if let Some(mut socket)=self.socket.take(){
            let _=socket.set_write_timeout(Some(Duration::from_millis(250)));
            let _=socket.write_all(b"{\"kind\":\"exit\",\"body\":\"\"}\n");
            let _=socket.flush();
        }
        self.reader.take();
        if let Some(mut process)=self.process.take(){
            for _ in 0..100 {
                match process.try_wait() {
                    Ok(Some(_))=>return,
                    Err(_)=>break,
                    Ok(None)=>std::thread::sleep(Duration::from_millis(50)),
                }
            }
            let _=process.kill();let _=process.wait();
        }
    }
}

struct Browser{child:JsonChild}
impl Browser {
    fn spawn(config:Value,ui:&mut Ui)->Result<Self,String>{
        ui.event("status","Launching Playwright browser…");
        let python=env::var("WEBCODEX_PYTHON").unwrap_or_else(|_|if cfg!(windows){"python".into()}else{"python3".into()});
        let mut cmd=Command::new(python);cmd.args(["-m","webcodex.browser_bridge"]);
        let mut browser=Self{child:JsonChild::spawn(cmd)?};browser.call(json!({"op":"start","config":config}),Some(ui))?;Ok(browser)
    }
    fn call(&mut self,request:Value,mut ui:Option<&mut Ui>)->Result<Value,String>{
        self.child.send(&request)?;let mut displayed=String::new();
        loop{
            let response=self.child.receive()?;
            if response["kind"]=="status" {
                if let Some(ui)=ui.as_deref_mut(){ui.event("status",response["text"].as_str().unwrap_or("Loading browser…"));}
                continue
            }
            if response["kind"]=="update"{
                if let Some(text)=response["text"].as_str(){
                    if let Some(ui)=ui.as_deref_mut(){
                        if ui.socket.is_some() {
                            ui.event("stream",&clean_reply(text));
                        } else {
                            // Do not leak model edit blocks in plain terminal mode.
                            let visible=clean_reply(text);
                            if visible.starts_with(&displayed) {
                                print!("{}",&visible[displayed.len()..]);let _=io::stdout().flush();
                            }
                            displayed=visible;
                        }
                    }
                }
                continue
            }
            if response["kind"]=="result" {
                if response["ok"]==false{return Err(response["error"].as_str().unwrap_or("browser error").into())}
                return Ok(response)
            }
        }
    }
}

#[derive(Default)]struct Options {
    root:PathBuf,resume:bool,last:bool,thread_id:Option<String>,list:bool,login:bool,plain:bool,
    import_codex:bool,import_list:bool,import_session:Option<String>,codex_home:Option<String>,chatgpt_url:Option<String>,
    yes:bool,allow_run:bool,approve_never:bool,force:bool,interactive:bool,
    max_turns:Option<usize>,max_chars:Option<usize>,model:Option<String>,
    browser:Option<String>,cdp:Option<String>,profile:Option<String>,headless:bool,temporary:bool,
    tasks:Vec<String>,
}
/// Accept a ChatGPT conversation UUID or a canonical chatgpt.com conversation URL.
/// Never navigate to arbitrary hosts or non-conversation paths.
fn chatgpt_resume_url(raw:&str)->Result<String,String>{
    let id=if let Some(tail)=raw.strip_prefix("https://chatgpt.com/c/") {
        tail.strip_suffix('/').unwrap_or(tail)
    } else if let Some(tail)=raw.strip_prefix("https://www.chatgpt.com/c/") {
        tail.strip_suffix('/').unwrap_or(tail)
    } else if raw.contains("://") || raw.contains('/') || raw.contains('?') || raw.contains('#') {
        return Err("resume requires a ChatGPT conversation UUID or https://chatgpt.com/c/<UUID>".into())
    } else { raw };
    let parts:Vec<&str>=id.split('-').collect();
    if parts.len()!=5 || [8,4,4,4,12].iter().enumerate().any(|(i,n)|parts[i].len()!=*n)
        || !id.bytes().all(|b|b==b'-'||b.is_ascii_hexdigit()) {
        return Err("invalid ChatGPT conversation ID: expected a UUID like 6aebe9ae-0a9c-83ea-8e91-da222befc0c1".into());
    }
    Ok(format!("https://chatgpt.com/c/{}",id.to_ascii_lowercase()))
}

fn option_value<I:Iterator<Item=String>>(args:&mut I,flag:&str)->Result<String,String>{
    args.next().ok_or_else(||format!("missing value after {flag}"))
}
fn parse_options()->Result<Options,String>{
    let mut o=Options{root:PathBuf::from("."),..Default::default()};let mut args=env::args().skip(1).peekable();
    while let Some(arg)=args.next(){
        match arg.as_str(){
            "resume"|"--resume"=>o.resume=true,
            "import-codex"=>o.import_codex=true,
            "--last"=>{o.resume=true;o.last=true},
            "--list" if o.import_codex=>o.import_list=true,
            "--session" if o.import_codex=>o.import_session=Some(option_value(&mut args,&arg)?),
            "--codex-home" if o.import_codex=>o.codex_home=Some(option_value(&mut args,&arg)?),
            "-C"|"--dir"=>o.root=PathBuf::from(option_value(&mut args,&arg)?),
            "--thread-id"=>o.thread_id=Some(option_value(&mut args,&arg)?),
            "--list-threads"=>o.list=true,"--login"=>o.login=true,
            "--no-tui"|"--plain"=>o.plain=true,"-y"|"--yes"=>o.yes=true,
            "--allow-run"=>o.allow_run=true,"-i"|"--interactive"=>o.interactive=true,
            "--force"=>o.force=true,"--max-turns"=>o.max_turns=Some(option_value(&mut args,&arg)?.parse().map_err(err)?),
            "--max-chars"=>o.max_chars=Some(option_value(&mut args,&arg)?.parse().map_err(err)?),
            "--model"=>o.model=Some(option_value(&mut args,&arg)?),"--browser"=>o.browser=Some(option_value(&mut args,&arg)?),
            "--cdp"=>o.cdp=Some(option_value(&mut args,&arg)?),"--profile"=>o.profile=Some(option_value(&mut args,&arg)?),
            "--headless"=>o.headless=true,"--temporary"=>o.temporary=true,
            "-s"|"--sandbox"=>{let mode=option_value(&mut args,&arg)?;if mode!="danger-full-access"{return Err(format!("unsupported sandbox mode: {mode}"))}o.allow_run=true},
            "-a"|"--ask-for-approval"=>{let mode=option_value(&mut args,&arg)?;if mode!="never"&&mode!="on-request"{return Err(format!("unsupported approval policy: {mode}"))}o.approve_never=mode=="never"},
            "--version"=>{println!("webcodex 1.2.1 (Rust)");std::process::exit(0)},
            "--help"|"-h"=>{println!("WebCodex Rust CLI\n  webcodex resume [-s danger-full-access -a never] (select saved thread)\n  webcodex resume <ChatGPT-UUID> [-s danger-full-access -a never]\n  webcodex resume https://chatgpt.com/c/<UUID>\n  webcodex resume --last (resume latest without picker)\n  webcodex import-codex [--list | --session <id>] [--codex-home <dir>]\n  -C <folder>  --model <model>  --browser <chrome|msedge|chromium>\n  --thread-id <id>  --list-threads  --login  --max-turns <n>  -i\n  --no-tui  --force  --allow-run  --yes\n\n{HELP}");std::process::exit(0)},
            "--manual"|"--dry-run"=>return Err(format!("{arg} is available through the Python compatibility CLI")),
            "--"=>{o.tasks.extend(args);break},
            _ if arg.starts_with('-')=>return Err(format!("unknown option: {arg}")),
            _ if o.resume && o.chatgpt_url.is_none() && o.tasks.is_empty() => {
                o.chatgpt_url=Some(chatgpt_resume_url(&arg)?);
            },
            _=>o.tasks.push(arg),
        }
    }
    if o.chatgpt_url.is_some() && (o.last || o.thread_id.is_some() || o.import_codex){
        return Err("a ChatGPT conversation ID cannot be combined with --last, --thread-id or import-codex".into());
    }
    Ok(o)
}

struct Agent {root:PathBuf,db:PathBuf,worker:JsonChild,browser:Browser,ui:Ui,
    thread:Option<String>,state:Value,yes:bool,allow_run:bool,approve_never:bool,
    max_turns:usize,max_chars:usize,max_file_chars:usize,max_files:usize}
impl Agent {
    fn worker(&mut self,operation:&str,mut args:Value)->Result<Value,String>{
        args["op"]=json!(operation);self.worker.call(args)
    }
    fn store(&mut self,operation:&str,mut args:Value)->Result<Value,String>{
        args["db_path"]=json!(self.db.to_string_lossy());
        self.worker(&format!("store_{operation}"),args)
    }
    fn ensure_thread(&mut self)->Result<String,String>{
        if let Some(id)=&self.thread{return Ok(id.clone())}
        let root=self.root.to_string_lossy().to_string();
        let result=self.store("create",json!({"project_root":root}))?;
        let id=result["id"].as_str().ok_or("missing saved thread id")?.to_string();
        self.thread=Some(id.clone());Ok(id)
    }
    fn log(&mut self,role:&str,body:&str)->Result<(),String>{
        if body.trim().is_empty(){return Ok(())}
        let tid=self.ensure_thread()?;
        self.store("add_message",json!({"thread_id":tid,"role":role,"content":body}))?;Ok(())
    }
    fn persist(&mut self)->Result<(),String>{
        let tid=self.ensure_thread()?;
        if let Ok(reply)=self.browser.call(json!({"op":"current_url"}),None){
            if let Some(url)=reply["url"].as_str(){self.store("set_url",json!({"thread_id":tid,"url":url}))?;}
        }
        let state=self.state.clone();self.store("save_state",json!({"thread_id":tid,"state":state}))?;
        Ok(())
    }
    fn scan(&mut self)->Result<Value,String>{self.worker("project_scan",json!({"root":self.root.to_string_lossy(),"max_files":self.max_files}))}
    fn message(&mut self,task:Option<&str>,pins:&[String],feedback:&[String])->Result<String,String>{
        let scan=self.scan()?;
        let entries=scan["entries"].clone();
        let next=self.worker("plan_message",json!({
            "root":self.root.to_string_lossy(),"entries":entries,"task":task,"pinned":pins,"feedback":feedback,
            "protocol":PROTOCOL,"reminder":REMINDER,
            "sent_hash":self.state["sent_hash"].clone(),"sent_paths":self.state["sent_paths"].clone(),
            "not_sent":self.state["not_sent"].clone(),"notes":self.state["notes"].clone(),
            "started":self.state["started"].clone(),"max_chars":self.max_chars,"max_file_chars":self.max_file_chars
        }))?;
        for key in ["sent_hash","sent_paths","not_sent"]{self.state[key]=next[key].clone();}
        self.state["started"]=json!(true);self.state["notes"]=json!([]);
        Ok(next["message"].as_str().ok_or("missing planned prompt")?.to_string())
    }
    fn note(&mut self,note:String){if let Some(notes)=self.state["notes"].as_array_mut(){notes.push(json!(note));}}
    fn hash(&mut self,rel:&str)->Result<(),String>{
        let scan=self.scan()?;
        let entries=scan["entries"].clone();
        if let Some(hash)=entries[rel]["hash"].as_str(){self.state["sent_hash"][rel]=json!(hash);}
        else if let Some(map)=self.state["sent_hash"].as_object_mut(){map.remove(rel);}
        Ok(())
    }
    fn validate_path(&self,rel:&str)->Result<PathBuf,String>{
        let path=Path::new(rel);
        if rel.is_empty()||rel.contains(':')||path.is_absolute(){return Err("absolute/empty paths rejected".into())}
        let mut target=self.root.clone();
        for component in path.components(){
            match component {Component::Normal(name)=>{
                if name==".webcodex"{return Err("cannot modify .webcodex".into())}
                target.push(name);
                // symlink_metadata also detects dangling symlinks. A dangling link
                // must never be treated as a new file within the project.
                if let Ok(metadata)=fs::symlink_metadata(&target) {
                    if metadata.file_type().is_symlink() && !target.exists() {
                        return Err("dangling symlink path rejected".into());
                    }
                    let abs=fs::canonicalize(&target).map_err(err)?;
                    if !abs.starts_with(&self.root){return Err("path escapes project".into())}
                    target=abs;
                }
            },_=>return Err("unsafe relative path".into())}
        }
        Ok(target)
    }
    fn backup_dir(&self)->PathBuf {
        let stamp=chrono::Utc::now().format("%Y%m%d-%H%M%S").to_string();let base=self.root.join(".webcodex/backups");
        let mut dest=base.join(&stamp);let mut i=1;
        while dest.exists(){i+=1;dest=base.join(format!("{stamp}-{i}"))}dest
    }
    fn backup(&mut self,dir:&Path,rel:&str,deleting:bool,manifest:&mut BTreeMap<String,Vec<String>>)->Result<(),String>{
        let result=self.worker("backup_save",json!({"root":self.root.to_string_lossy(),"backup_dir":dir.to_string_lossy(),"rel":rel,"deleting":deleting}))?;
        let kind=result["kind"].as_str().ok_or("missing backup kind")?;
        manifest.entry(kind.into()).or_default().push(rel.into());Ok(())
    }
    fn handle_reply(&mut self,reply:&str)->Result<(Vec<String>,Vec<String>),String>{
        let parsed=self.worker("parse_reply",json!({"text":reply}))?;
        let parse_errors=strings(&parsed["errors"]);
        if !parse_errors.is_empty(){
            return Ok((
                parse_errors.into_iter().map(|e|format!("EDIT REJECTED: {e}")).collect(),
                Vec::new()
            ));
        }
        let mut feedback=Vec::new();let mut pins=Vec::new();let backup=self.backup_dir();
        let mut manifest:BTreeMap<String,Vec<String>>=BTreeMap::new();
        let mut grouped:BTreeMap<String,Vec<Value>>=BTreeMap::new();
        for block in parsed["edits"].as_array().into_iter().flatten(){
            if let Some(path)=block["path"].as_str(){grouped.entry(path.into()).or_default().push(block.clone())}
        }
        for (rel,blocks) in grouped {
            let path=match self.validate_path(&rel){Ok(p)=>p,Err(e)=>{feedback.push(format!("EDIT REJECTED {rel}: {e}"));continue}};
            let raw=if path.exists(){match fs::read(&path){Ok(v)=>Some(v),Err(e)=>{feedback.push(format!("EDIT FAILED {rel}: {e}"));continue}}}else{None};
            let crlf=raw.as_ref().is_some_and(|v|v.windows(2).any(|v|v==b"\r\n"));
            let current=match raw.as_ref().map(|v|String::from_utf8(v.clone())){
                Some(Ok(v)) if !v.contains('\0')=>Some(v.replace("\r\n","\n")),
                Some(_)=>{feedback.push(format!("EDIT FAILED {rel}: binary or non-UTF8"));continue},None=>None,
            };
            let result=self.worker("patch",json!({"text":current,"blocks":blocks}))?;
            let errors=strings(&result["errors"]);
            for e in &errors {feedback.push(format!("EDIT FAILED {e}"));pins.push(rel.clone());}
            if result["applied"].as_u64().unwrap_or(0)==0{continue}
            let Some(new)=result["text"].as_str() else{continue};
            if Some(new)==current.as_deref(){continue}
            self.ui.event("activity",&format!("Edit {rel}"));
            if !self.yes && !self.ui.confirm(&format!("Apply changes to {rel}?"),true)?{
                self.note(format!("The user REJECTED your edit to {rel}; the file is unchanged."));continue;
            }
            if self.ui.approval_all{self.yes=true;self.approve_never=true;}
            self.backup(&backup,&rel,false,&mut manifest)?;
            if let Some(parent)=path.parent(){fs::create_dir_all(parent).map_err(err)?}
            let output=if crlf{new.replace('\n',"\r\n")}else{new.into()};
            fs::write(path,output).map_err(err)?;
            if errors.is_empty(){self.hash(&rel)?;}
            else if let Some(hashes)=self.state["sent_hash"].as_object_mut(){hashes.remove(&rel);}
        }
        for rel in strings(&parsed["deletes"]){
            let path=match self.validate_path(&rel){Ok(p)=>p,Err(e)=>{feedback.push(format!("DELETE REJECTED {rel}: {e}"));continue}};
            if !path.is_file(){feedback.push(format!("DELETE FAILED {rel}: no such file"));continue}
            self.ui.event("activity",&format!("Delete {rel}"));
            if !self.yes && !self.ui.confirm(&format!("Delete {rel}?"),true)?{self.note(format!("The user REJECTED deleting {rel}."));continue}
            if self.ui.approval_all{self.yes=true;self.approve_never=true;}
            self.backup(&backup,&rel,true,&mut manifest)?;
            fs::remove_file(path).map_err(err)?;
            if let Some(map)=self.state["sent_hash"].as_object_mut(){map.remove(&rel);}
        }
        if !manifest.is_empty(){
            fs::create_dir_all(&backup).map_err(err)?;
            fs::write(backup.join("manifest.json"),serde_json::to_vec(&manifest).map_err(err)?).map_err(err)?;
        }
        if let Some(needs)=parsed["needs"].as_array(){
            let scan=self.scan()?;
            let entries=scan["entries"].clone();let mut found=false;
            for v in needs {if let Some(rel)=v.as_str(){
                if entries[rel]["hash"].is_string(){pins.push(rel.to_string());found=true}else{feedback.push(format!("NEED {rel}: file not found, binary, or ignored"))}
            }}
            if found{feedback.push("The files you requested are included below.".into())}
        }
        for cmd in strings(&parsed["runs"]){
            if !self.allow_run {self.ui.event("activity",&format!("Skipped shell command: {cmd}"));continue}
            if !self.approve_never && !self.ui.confirm(&format!("Run: {cmd}?"),false)?{self.note(format!("The user declined to run: {cmd}"));continue}
            if self.ui.approval_all{self.yes=true;self.approve_never=true;}
            self.ui.event("activity",&format!("Run {cmd}"));
            let result=self.worker("run",json!({"root":self.root.to_string_lossy(),"command":cmd,"timeout":120}))?;
            let code=result["code"].as_i64().unwrap_or(-1);let output=result["output"].as_str().unwrap_or("");
            if !output.is_empty(){self.ui.event("activity",&tail(output,3000))}
            feedback.push(format!("RUN `{cmd}` exited with code {code}. Output:\n{}",tail(output,8000)));
        }
        Ok((feedback,pins))
    }
    fn task(&mut self,task:&str)->Result<(),String>{
        self.log("user",task)?;
        // A read-only imported Codex transcript is presented as a bounded handoff
        // exactly once when the user sends the first task to the new ChatGPT chat.
        if let Some(handoff)=self.state["handoff"].as_str().map(str::to_string) {
            self.note(handoff);
        }
        let mut prompt=self.message(Some(task),&[],&[])?;
        if self.state["handoff"].is_string() {self.state.as_object_mut().map(|v|v.remove("handoff"));}
        for turn in 0..self.max_turns {
            let state_dir=self.root.join(".webcodex");fs::create_dir_all(&state_dir).map_err(err)?;
            fs::write(state_dir.join("last_prompt.txt"),&prompt).map_err(err)?;
            self.ui.event("activity",&format!("Working · turn {}/{}",turn+1,self.max_turns));
            let answer=self.browser.call(json!({"op":"send","text":prompt}),Some(&mut self.ui))?;
            let reply=answer["text"].as_str().ok_or("missing browser response")?;
            if reply.trim().is_empty(){break}
            self.log("assistant",reply)?;
            let _=self.persist();
            let display=clean_reply(reply);
            if self.ui.socket.is_some() {
                self.ui.event("stream_end",&display);
            } else if !display.is_empty() {
                // The plain terminal already received live streamed text.
                self.ui.event("assistant",&display);
            }
            let (feedback,pins)=self.handle_reply(reply)?;
            if feedback.is_empty(){break}
            prompt=self.message(None,&pins,&feedback)?;
        }
        self.persist()?;Ok(())
    }
    fn command(&mut self,command:&str)->Result<bool,String>{
        match command.split_whitespace().next().unwrap_or(""){
            "/quit"|"/exit"|"/q"=>return Ok(false),
            "/help"=>self.ui.event("system",HELP),
            "/status"=>self.ui.event("system",&format!("project: {}\nthread: {}\nauto apply: {}\nrun commands: {}\napproval never: {}",self.root.display(),self.thread.as_deref().unwrap_or("new"),self.yes,self.allow_run,self.approve_never)),
            "/chat"=>{let url=self.browser.call(json!({"op":"current_url"}),None)?;self.ui.event("system",url["url"].as_str().unwrap_or("No chat URL yet"))},
            "/files"=>{let scan=self.scan()?;let entries=scan["entries"].clone();let mut names=Vec::new();if let Some(entries)=entries.as_object(){for k in entries.keys(){names.push(k.clone());}}self.ui.event("system",&names.join("\n"))},
            "/yes"=>{self.yes=!self.yes;self.ui.event("system",&format!("auto apply: {}",self.yes))},
            "/run"=>{self.allow_run=!self.allow_run;self.ui.event("system",&format!("run commands: {}",self.allow_run))},
            "/clear"=>self.ui.event("clear",""),
            "/new"=>{self.browser.call(json!({"op":"new_chat"}),None)?;self.thread=None;self.state=fresh_state();self.ui.event("system","New chat started.")},
            "/undo"=>{
                let result=self.worker("backup_undo",json!({"root":self.root.to_string_lossy()}))?;
                if result["nothing"]==true{self.ui.event("system","Nothing to undo.")}
                else{
                    for rel in strings(&result["restored"]){
                        if let Some(hashes)=self.state["sent_hash"].as_object_mut(){hashes.remove(&rel);}
                    }
                    self.note("The user UNDID your last set of changes; files were restored to their earlier state.".into());
                    self.persist()?;
                    self.ui.event("system","Undone.")
                }
            },
            _=>self.ui.event("error","Unknown command. Type /help."),
        }
        Ok(true)
    }
}
fn fresh_state()->Value{json!({"started":false,"sent_hash":{},"sent_paths":[],"not_sent":[],"notes":[]})}
fn tail(s:&str,n:usize)->String{if s.len()<=n{return s.into()}let mut begin=s.len()-n;while !s.is_char_boundary(begin){begin+=1}format!("...(truncated)...\n{}",&s[begin..])}
fn clean_reply(s:&str)->String{
    let mut visible=Vec::new();
    let mut fence:Option<(char,usize)>=None;
    let mut edit_block=false;
    let mut html_pre=false;
    for line in s.lines(){
        let t=line.trim_start();
        let ch=t.chars().next().unwrap_or(' ');
        let count=if ch=='`'||ch=='~'{t.chars().take_while(|&c|c==ch).count()}else{0};
        if let Some((opened,len))=fence {
            if ch==opened && count>=len && t[count..].trim().is_empty(){fence=None;}
            continue;
        }
        if count>=3 {fence=Some((ch,count));continue;}
        if html_pre {if t.contains("</pre>"){html_pre=false;}continue;}
        if t.starts_with("<pre"){html_pre=!t.contains("</pre>");continue;}
        if t.starts_with("FILE:"){edit_block=true;continue;}
        if edit_block {if t.starts_with(">>>>>>> REPLACE"){edit_block=false;}continue;}
        if t.starts_with("<<<<<<< SEARCH")||t=="======="||t.starts_with(">>>>>>> REPLACE"){continue;}
        if t.starts_with("RUN:")||t.starts_with("NEED:")||t.starts_with("DELETE:"){continue;}
        visible.push(line);
    }
    visible.join("\n").trim().to_string()
}

fn run()->Result<(),String>{
    let o=parse_options()?;let root=fs::canonicalize(&o.root).map_err(err)?;
    if !root.is_dir(){return Err("project directory does not exist".into())}
    if !o.force && !o.login && !o.list {
        let drive_root=root.parent().is_none();
        let user_home=home().parent().map(Path::to_path_buf);
        if drive_root||user_home.as_ref()==Some(&root){return Err("refusing to run in home or drive root without --force".into())}
    }
    let db=home().join("workspace.sqlite3");
    let mut worker=JsonChild::spawn(Command::new(core_executable()))?;
    // List/import local Codex sessions without opening a browser.
    // Existing Codex sessions are read-only. Import writes only to WebCodex SQLite.
    let mut imported_thread:Option<String>=None;
    if o.import_codex {
        let folder=codex_import::codex_home(o.codex_home.as_deref());
        let sessions=codex_import::discover(&folder);
        if o.import_list {
            for (i,s) in sessions.iter().enumerate(){println!("{:>3}. {}  {}  [{}]",i+1,s.id,s.title,s.cwd);}
            if sessions.is_empty(){println!("No Codex CLI sessions found under {}",folder.display());}
            return Ok(())
        }
        let chosen=if let Some(id)=o.import_session.as_deref() {
            codex_import::find(&sessions,id)?.clone()
        }else{
            let rows=sessions.iter().map(|s|(s.id.clone(),format!("{} · {} · {}",s.title,s.id,s.cwd))).collect::<Vec<_>>();
            let Some(index)=picker::pick("Import a Codex CLI session",&rows,false)? else{return Ok(())};
            sessions[index].clone()
        };
        // Guard against ambiguous or unreadable source before creating a thread.
        let imported=codex_import::import(&chosen)?;
        let title=format!("Codex import · {}",chosen.title.chars().take(90).collect::<String>());
        let created=worker.call(json!({"op":"store_create","db_path":db.to_string_lossy(),
            "project_root":root.to_string_lossy(),"title":title}))?;
        let id=created["id"].as_str().ok_or("missing imported thread ID")?.to_string();
        for (role,text) in &imported.messages {
            worker.call(json!({"op":"store_add_message","db_path":db.to_string_lossy(),
                "thread_id":id,"role":role,"content":text}))?;
        }
        worker.call(json!({"op":"store_save_state","db_path":db.to_string_lossy(),
            "thread_id":id,"state":codex_import::state(&imported.handoff,&chosen)}))?;
        println!("Imported {} messages ({} older turns omitted) into WebCodex thread {}.\nOriginal Codex session unchanged.",imported.messages.len(),imported.omitted,id);
        imported_thread=Some(id);
    }
    if o.list{
        let result=worker.call(json!({"op":"store_list","db_path":db.to_string_lossy(),"limit":30}))?;
        for item in result["threads"].as_array().into_iter().flatten(){println!("{}  {}  {}  [{}]",item["id"].as_str().unwrap_or(""),item["updated_at"].as_str().unwrap_or(""),item["title"].as_str().unwrap_or(""),item["project_root"].as_str().unwrap_or(""));}
        return Ok(())
    }
    let mut config=serde_json::Map::new();
    if let Some(v)=o.model.clone(){config.insert("model".into(),json!(v));}
    if let Some(v)=o.browser.clone(){config.insert("browser".into(),json!(v));}
    if let Some(v)=o.cdp.clone(){config.insert("cdp".into(),json!(v));}
    if let Some(v)=o.profile.clone(){config.insert("profile_dir".into(),json!(v));}
    if o.headless{config.insert("headless".into(),json!(true));}
    if o.temporary{config.insert("temporary".into(),json!(true));}
    // Resolve resume selection before starting Playwright. Esc starts a new chat.
    let mut selected_thread=o.thread_id.clone().or(imported_thread);
    if selected_thread.is_none() && o.resume && !o.last && !o.import_codex && o.chatgpt_url.is_none() {
        let listing=worker.call(json!({"op":"store_list","db_path":db.to_string_lossy(),
            "project_root":root.to_string_lossy(),"limit":100}))?;
        let threads=listing["threads"].as_array().cloned().unwrap_or_default();
        let rows=threads.iter().map(|v|{
            let id=v["id"].as_str().unwrap_or("").to_string();
            let label=format!("{}  ·  {}  ·  {}",v["title"].as_str().unwrap_or("Untitled"),
                v["updated_at"].as_str().unwrap_or(""),id);
            (id,label)
        }).collect::<Vec<_>>();
        if let Some(idx)=picker::pick("Resume a conversation",&rows,true)?{
            selected_thread=Some(rows[idx].0.clone());
        }
    }
    // A direct ChatGPT URL may already be associated with a WebCodex thread.
    // Use that thread when possible to preserve local work and prevent duplicates.
    if let Some(url)=o.chatgpt_url.as_ref(){
        let found=worker.call(json!({"op":"store_find_url","db_path":db.to_string_lossy(),
            "project_root":root.to_string_lossy(),"url":url}))?;
        selected_thread=found["thread"]["id"].as_str().map(str::to_string);
    }
    // Launch the native TUI before opening the browser so startup never looks frozen.
    let mut ui=Ui::new(&root,o.plain||o.login);
    ui.event("status","Starting browser transport…");
    let mut browser=Browser::spawn(Value::Object(config),&mut ui)?;
    if o.login{
        let result=browser.call(json!({"op":"login"}),None)?;
        if result["ok"]==true{println!("WebCodex browser login successful.");return Ok(())}
        return Err("browser login could not be confirmed".into())
    }
    let mut thread=selected_thread;
    if thread.is_none()&&o.resume&&o.last{
        let latest=worker.call(json!({"op":"store_latest","db_path":db.to_string_lossy(),"project_root":root.to_string_lossy()}))?;
        thread=latest["thread"]["id"].as_str().map(str::to_string);
    }
    let mut state=fresh_state();let mut opened=false;
    if let Some(url)=o.chatgpt_url.as_ref(){
        ui.event("status","Opening requested ChatGPT conversation…");
        opened=browser.call(json!({"op":"open_chat","url":url}),Some(&mut ui))?["ok"]==true;
        if !opened {
            return Err(format!("ChatGPT conversation could not be opened: {url}. Check browser login and access permissions."));
        }
        if thread.is_none(){
            let created=worker.call(json!({"op":"store_create","db_path":db.to_string_lossy(),
                "project_root":root.to_string_lossy(),"chat_url":url,
                "title":format!("ChatGPT · {}",url.rsplit('/').next().unwrap_or("conversation"))}))?;
            thread=created["id"].as_str().map(str::to_string);
            if thread.is_none(){return Err("could not save ChatGPT conversation link".into())}
        }
    }
    if let Some(tid)=thread.as_ref(){
        let found=worker.call(json!({"op":"store_get","db_path":db.to_string_lossy(),"thread_id":tid}))?;
        if found["thread"].is_null() {
            if o.thread_id.is_some(){return Err(format!("No saved thread with id {tid}"))}
            thread=None;
        } else if o.chatgpt_url.is_some() {
            // Browser navigation already succeeded. Load only local state.
            let saved=worker.call(json!({"op":"store_load_state","db_path":db.to_string_lossy(),"thread_id":tid}))?;
            if saved["state"]["sent_hash"].is_object(){state=saved["state"].clone();}
        } else if found["thread"]["chat_url"].is_null() {
            let saved=worker.call(json!({"op":"store_load_state","db_path":db.to_string_lossy(),"thread_id":tid}))?;
            if saved["state"]["codex_import"].is_object() {
                state=saved["state"].clone();
                ui.event("status","Starting ChatGPT for imported Codex session…");
                browser.call(json!({"op":"new_chat"}),Some(&mut ui))?;
                opened=true;
            }
        } else if let Some(url)=found["thread"]["chat_url"].as_str(){
            ui.event("status","Restoring ChatGPT conversation…");
            opened=browser.call(json!({"op":"open_chat","url":url}),Some(&mut ui)).map(|v|v["ok"]==true).unwrap_or(false);
            if opened {let saved=worker.call(json!({"op":"store_load_state","db_path":db.to_string_lossy(),"thread_id":tid}))?;
                if saved["state"].is_object(){state=saved["state"].clone();}
                let had_context=state["sent_hash"].as_object().is_some_and(|h|!h.is_empty())
                    ||state["sent_paths"].as_array().is_some_and(|p|!p.is_empty());
                for (k,v) in [("started",json!(had_context)),("sent_hash",json!({})),("sent_paths",json!([])),("not_sent",json!([])),("notes",json!([]))]{
                    if state.get(k).is_none(){state[k]=v;}
                }
            }
        }
    }
    if !opened {
        // Do not overwrite an old thread when its browser conversation is
        // unavailable. A fresh browser chat must also receive a fresh thread.
        thread=None;
        ui.event("status","Preparing a new ChatGPT conversation…");
        browser.call(json!({"op":"new_chat"}),Some(&mut ui))?;
        state=fresh_state();
    }
    ui.event("ready","");
    ui.event("system",if state["codex_import"].is_object(){"Codex history imported. Your next message starts the ChatGPT handoff."}else if opened{"Resumed your earlier ChatGPT chat."}else{"WebCodex · new session"});
    // Restore a recent SQLite window in a single TUI event. The previous
    // implementation sent 100 individual messages, triggering repeated full
    // transcript formatting and redraws while resuming long chats.
    // Older conversation turns remain available in the session database.
    if opened {
        if let Some(tid)=thread.as_deref() {
            if let Ok(history)=worker.call(json!({"op":"store_messages", "db_path":db.to_string_lossy(),
                "thread_id":tid, "limit":50})) {
                let mut restored=Vec::<(String,String)>::new();
                for item in history["messages"].as_array().into_iter().flatten() {
                    let role=item["role"].as_str().unwrap_or("");
                    let content=item["content"].as_str().unwrap_or("");
                    match role {
                        "user"=>restored.push(("user".into(),content.into())),
                        "assistant"=>{
                            let display=clean_reply(content);
                            if !display.is_empty(){restored.push(("assistant".into(),display));}
                        },
                        _=>{}
                    }
                }
                if !restored.is_empty() {
                    if let Ok(batch)=serde_json::to_string(&restored) {ui.event("history",&batch);}
                }
            }
        }
    }
    let cfg:Value=fs::read(home().join("config.json")).ok().and_then(|v|serde_json::from_slice(&v).ok()).unwrap_or(Value::Null);
    let max_turns=o.max_turns.unwrap_or(cfg["max_turns"].as_u64().unwrap_or(6) as usize);
    let max_chars=o.max_chars.unwrap_or(cfg["max_chars"].as_u64().unwrap_or(60000) as usize);
    let mut agent=Agent{root,db,worker,browser,ui,thread,state,yes:o.yes||o.approve_never,
        allow_run:o.allow_run,approve_never:o.approve_never,max_turns,max_chars,
        max_file_chars:cfg["max_file_chars"].as_u64().unwrap_or(20000) as usize,
        max_files:cfg["max_files"].as_u64().unwrap_or(4000) as usize};
    if !o.tasks.is_empty(){agent.task(&o.tasks.join(" "))?;if !o.interactive{return Ok(())}}
    loop {
        let Some(input)=agent.ui.receive()? else{break};
        if input.trim().is_empty(){continue}
        agent.ui.event("busy","true");
        let result=if input.starts_with('/') {agent.command(input.trim()).map(|continue_|(continue_,()))}
            else{agent.task(input.trim()).map(|_|(true,()))};
        agent.ui.event("busy","false");
        match result{Ok((false,_))=>break,Err(e)=>agent.ui.event("error",&e),_=>{}}
    }
    Ok(())
}
fn main(){if let Err(e)=run(){eprintln!("WebCodex Rust agent error: {e}");std::process::exit(1)}}

#[cfg(test)]mod tests {
    use super::*;
    #[test]fn markdown_blocks_are_hidden(){
        let s="hello\n````\nFILE: a\n<<<<<<< SEARCH\nold\n=======\nnew\n>>>>>>> REPLACE\n````\nRUN: echo hello";
        assert_eq!(clean_reply(s),"hello");
    }
    #[test]fn all_fenced_blocks_stay_hidden(){
        let input="Intro\n```rust\nfn main() {}\n```\nAfter\n~~~~\nsecret\n~~~~\nDone";
        assert_eq!(clean_reply(input),"Intro\nAfter\nDone");
        assert_eq!(clean_reply("Hi\n````\nFILE: x\n"),"Hi");
    }
    #[test]fn streamed_partial_fences_never_show_code(){
        assert_eq!(clean_reply("Hello\n```python\nprint('secret')"),"Hello");
        assert_eq!(clean_reply("Hello\n```python\nprint('secret')\n```\nFinished"),"Hello\nFinished");
    }
    #[test]fn unicode_tail_is_valid(){assert!(tail("é⚡abc",4).ends_with("abc"));}
    #[test]fn chatgpt_ids_accept_uuid_and_url(){
        let id="6aebe9ae-0a9c-83ea-8e91-da222befc0c1";
        let url=format!("https://chatgpt.com/c/{id}");
        assert_eq!(chatgpt_resume_url(id).unwrap(),url);
        assert_eq!(chatgpt_resume_url(&url).unwrap(),url);
        assert_eq!(chatgpt_resume_url(&format!("{url}/")).unwrap(),url);
        assert_eq!(chatgpt_resume_url(&id.to_ascii_uppercase()).unwrap(),url);
    }
    #[test]fn chatgpt_ids_reject_other_websites_and_invalid_ids(){
        for value in ["https://evil.example/c/6aebe9ae-0a9c-83ea-8e91-da222befc0c1",
            "https://chatgpt.com/c/6aebe9ae-0a9c-83ea-8e91-da222befc0c1?x=1",
            "short-id", "../../oops", "https://chatgpt.com/c/abc"] {
            assert!(chatgpt_resume_url(value).is_err(),"{value}");
        }
    }
    #[test]fn initial_state_is_compatible(){let v=fresh_state();assert!(v["sent_hash"].is_object());assert_eq!(v["started"],false);}

}
